import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_center_page.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

NotificationDto _n(String id, {bool read = false, String? relatedEntityType}) =>
    NotificationDto(
        id: id,
        userId: 'u1',
        type: 'EARNING_CREDITED',
        title: 'Consultation earning credited ($id)',
        body: 'Consultation earning credited: KES 80.',
        deliveryStatus: 'DELIVERED',
        relatedEntityType: relatedEntityType,
        createdAt: DateTime.utc(2026, 10, 5, 10),
        readAt: read ? DateTime.utc(2026, 10, 5, 11) : null);

/// `NotificationCenterPage.initState` fires a real `load()` on mount (same
/// `DoctorShell`-style convention as the rest of the Doctor app), so every
/// harness here must serve that call consistently with whatever fixture the
/// test wants rendered — a repository that just hangs forever would be
/// overwritten the instant `load()`'s own transient "loading" state is
/// emitted for any fixture with no cached items yet (`empty`/`error`).
/// [hang] is for the one case that deliberately wants to freeze mid-load, to
/// capture the loading spinner before it resolves.
class _StubRepository extends NotificationRepository {
  _StubRepository(
      {this.items = const [], this.unread = 0, this.error, this.hang = false})
      : super(ApiClient(dio: Dio()));
  final List<NotificationDto> items;
  final int unread;
  final ApiException? error;
  final bool hang;

  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async {
    if (hang) return Completer<List<NotificationDto>>().future;
    if (error != null) throw error!;
    return items;
  }

  @override
  Future<int> unreadCount() async {
    if (hang) return Completer<int>().future;
    if (error != null) throw error!;
    return unread;
  }

  // Overridden (rather than left to hit a real Dio socket) so a tapped row's
  // markRead()/markAllRead() confirmation resolves immediately instead of
  // attempting a genuine network call in a widget test.
  @override
  Future<void> markRead(String id) async {}
  @override
  Future<NotificationDto> detail(String id) async =>
      items.firstWhere((n) => n.id == id);
  @override
  Future<int> markAllRead() async => 0;
}

Widget harness(NotificationCubit cubit) => BlocProvider.value(
    value: cubit,
    child: MaterialApp(
        theme: DoctorTokens.theme(), home: const NotificationCenterPage()));

void main() {
  // Render with the app's already-bundled font and Flutter's existing icons
  // (same setup as qna_rendering_test.dart).
  setUpAll(() async {
    final text = FontLoader('Plus Jakarta Sans')
      ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf'));
    await text.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });

  final repos = <String, _StubRepository>{
    'loading': _StubRepository(hang: true),
    'empty': _StubRepository(),
    'error': _StubRepository(error: const ApiException(503, 'Unavailable')),
    'populated': _StubRepository(unread: 1, items: [
      _n('n-unread', relatedEntityType: 'APPOINTMENT'),
      _n('n-read', read: true),
    ]),
  };

  for (final entry in repos.entries) {
    testWidgets('notification center ${entry.key} rendered state',
        (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 844));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      // Freeze only the displayed refresh time. The original golden images
      // contain 14:45; wall-clock time made their exact pixel checks flaky.
      final cubit = NotificationCubit(entry.value,
          now: () => DateTime(2026, 10, 5, 14, 45));
      addTearDown(cubit.close);
      await tester.pumpWidget(harness(cubit));
      await tester.pump(const Duration(milliseconds: 200));
      expect(tester.takeException(), isNull);
      switch (entry.key) {
        case 'loading':
          expect(find.byType(CircularProgressIndicator), findsOneWidget);
          break;
        case 'empty':
          expect(find.text('No notifications yet'), findsOneWidget);
          break;
        case 'error':
          expect(find.text('Notifications could not be loaded. Please retry.'),
              findsOneWidget);
          break;
        case 'populated':
          expect(find.textContaining('n-unread'), findsOneWidget);
          expect(find.textContaining('n-read'), findsOneWidget);
          expect(find.text('Mark all read'), findsOneWidget);
          break;
      }
      await expectLater(find.byType(NotificationCenterPage),
          matchesGoldenFile('goldens/notifications-${entry.key}.png'));
    });
  }

  testWidgets('tapping an unread row marks it read and clears its dot',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final cubit = NotificationCubit(_StubRepository(
        unread: 1, items: [_n('00000000-0000-4000-8000-000000000001')]));
    addTearDown(cubit.close);
    await tester.pumpWidget(harness(cubit));
    await tester.pump(const Duration(milliseconds: 200));
    await tester
        .tap(find.textContaining('00000000-0000-4000-8000-000000000001').first);
    await tester.pump();
    expect(cubit.state.unreadCount, 0);
    expect(cubit.state.items.single.isRead, isTrue);
    expect(tester.takeException(), isNull);
  });

  testWidgets('"Mark all read" is hidden once there is nothing unread',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final cubit = NotificationCubit(
        _StubRepository(unread: 0, items: [_n('n1', read: true)]));
    addTearDown(cubit.close);
    await tester.pumpWidget(harness(cubit));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Mark all read'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
