import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/notifications/background_sync_worker.dart';
import 'package:hellodoctor_mobile/core/notifications/notification_ids.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';

NotificationDto _dto({
  required String id,
  required DateTime createdAt,
  String type = 'NEW_CHAT_MESSAGE',
}) =>
    NotificationDto(
      id: id,
      userId: 'doctor-user-1',
      type: type,
      title: 'Title $id',
      body: 'Body $id',
      createdAt: createdAt,
      deliveryStatus: 'DELIVERED',
    );

void main() {
  group('computeNewNotifications', () {
    test(
        '3 notifications newer than the cursor and 2 older -> exactly 3 new '
        'decisions, cursor advances to the newest', () {
      final cursor = DateTime.utc(2026, 1, 1, 12);
      final older1 = _dto(
          id: 'older-1',
          createdAt: cursor.subtract(const Duration(minutes: 10)));
      final older2 = _dto(
          id: 'older-2',
          createdAt: cursor.subtract(const Duration(minutes: 5)));
      final newer1 = _dto(
          id: 'newer-1', createdAt: cursor.add(const Duration(minutes: 1)));
      final newer2 = _dto(
          id: 'newer-2', createdAt: cursor.add(const Duration(minutes: 2)));
      final newer3 = _dto(
          id: 'newer-3', createdAt: cursor.add(const Duration(minutes: 3)));
      final fetched = [newer3, newer2, newer1, older2, older1];

      final decision = computeNewNotifications(
        fetched: fetched,
        cursorIso: cursor.toIso8601String(),
      );

      expect(decision.newItems, hasLength(3));
      expect(
        decision.newItems.map((d) => d.source.id).toSet(),
        {'newer-1', 'newer-2', 'newer-3'},
      );
      expect(decision.nextCursor, newer3.createdAt.toIso8601String());
      // Each new item must carry the exact dedup id/payload the dedup
      // mechanism (notification_ids.dart) requires, derived the same way
      // every other call site in this app derives it.
      for (final item in decision.newItems) {
        expect(item.payload, remoteEventKey(item.source.id));
        expect(item.id, stableNotificationId(remoteEventKey(item.source.id)));
      }
    });

    test('nothing newer than the cursor -> zero decisions, cursor unchanged',
        () {
      final cursor = DateTime.utc(2026, 1, 1, 12);
      final older1 = _dto(
          id: 'older-1',
          createdAt: cursor.subtract(const Duration(minutes: 10)));
      final older2 = _dto(
          id: 'older-2',
          createdAt: cursor.subtract(const Duration(minutes: 1)));
      final sameAsCursor = _dto(id: 'same', createdAt: cursor);

      final decision = computeNewNotifications(
        fetched: [sameAsCursor, older2, older1],
        cursorIso: cursor.toIso8601String(),
      );

      expect(decision.newItems, isEmpty);
      expect(decision.nextCursor, cursor.toIso8601String());
    });

    test(
        'the same notification list run twice in a row -> the second run '
        'produces zero new decisions (no duplicate)', () {
      final base = DateTime.utc(2026, 2, 1, 8);
      final fetched = [
        _dto(id: 'a', createdAt: base),
        _dto(id: 'b', createdAt: base.add(const Duration(minutes: 1))),
        _dto(id: 'c', createdAt: base.add(const Duration(minutes: 2))),
      ];

      // First run: no cursor yet.
      final firstRun =
          computeNewNotifications(fetched: fetched, cursorIso: null);
      expect(firstRun.newItems, hasLength(3));
      expect(firstRun.nextCursor, isNotNull);

      // Second run: same fetched list, cursor now persisted from the first
      // run's result — exactly what a repeated background wake looks like.
      final secondRun = computeNewNotifications(
        fetched: fetched,
        cursorIso: firstRun.nextCursor,
      );
      expect(secondRun.newItems, isEmpty);
      expect(secondRun.nextCursor, firstRun.nextCursor);
    });

    test('an empty fetch with an existing cursor leaves the cursor unchanged',
        () {
      final cursor = DateTime.utc(2026, 1, 1, 12);
      final decision = computeNewNotifications(
        fetched: const [],
        cursorIso: cursor.toIso8601String(),
      );
      expect(decision.newItems, isEmpty);
      expect(decision.nextCursor, cursor.toIso8601String());
    });
  });

  group('runGracefully', () {
    test(
        'an ApiException (e.g. an expired/revoked session) is swallowed '
        'and the run still reports success', () async {
      final result = await runGracefully(() async {
        throw const ApiException(401, 'Your session expired.');
      });
      expect(result, isTrue);
    });

    test('any other unexpected error is also swallowed, never propagated',
        () async {
      final result = await runGracefully(() async {
        throw StateError('unexpected');
      });
      expect(result, isTrue);
    });

    test('a successful body still reports success', () async {
      final result = await runGracefully(() async {});
      expect(result, isTrue);
    });
  });

  group('doctorIdentifierFromAccessToken', () {
    test(
        'extracts user_id/sub (the JWT subject) when no doctor_id claim '
        'exists, matching the real backend Claims shape', () {
      final jwt = fakeJwt({'sub': 'user-123', 'user_id': 'user-123'});
      expect(doctorIdentifierFromAccessToken(jwt), 'user-123');
    });

    test('prefers an explicit doctor_id claim if one is ever present', () {
      final jwt = fakeJwt(
          {'doctor_id': 'doc-9', 'user_id': 'user-123', 'sub': 'user-123'});
      expect(doctorIdentifierFromAccessToken(jwt), 'doc-9');
    });

    test('returns null for a malformed token rather than throwing', () {
      expect(doctorIdentifierFromAccessToken('not-a-jwt'), isNull);
      expect(doctorIdentifierFromAccessToken(null), isNull);
      expect(doctorIdentifierFromAccessToken(''), isNull);
    });
  });
}

/// Builds a real (unsigned, test-only) JWT string: three dot-separated
/// base64url segments, matching exactly what
/// [doctorIdentifierFromAccessToken] decodes (`base64Url.normalize` +
/// `base64Url.decode` + `utf8.decode` + `jsonDecode` on the middle segment).
String fakeJwt(Map<String, dynamic> claims) {
  String segment(Map<String, dynamic> value) =>
      base64Url.encode(utf8.encode(jsonEncode(value)));
  return '${segment({'alg': 'none', 'typ': 'JWT'})}.${segment(claims)}.';
}
