import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/video/call_controller.dart';
import 'package:hellodoctor_mobile/core/video/join_eligibility.dart';
import 'package:hellodoctor_mobile/core/video/mint_failure.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'core/video/call_controller_test.dart' show FakeAgoraEngine;

// Appointment 12:00–12:10 UTC == 3:00–3:10 PM EAT. The server's policy (mirrored
// from the Rust evaluator's own unit tests): open 11:45:00Z, close 13:10:00Z.
final _start = DateTime.utc(2026, 11, 11, 12);
final _end = DateTime.utc(2026, 11, 11, 12, 10);
final _open = _start.subtract(const Duration(minutes: 15));
final _close = _end.add(const Duration(minutes: 60));

Map<String, dynamic> _joinJson(DateTime serverNow, {String reason = 'OK'}) => {
      'eligible': reason == 'OK',
      'reason': reason,
      'join_open_at': _open.toIso8601String(),
      'join_close_at': _close.toIso8601String(),
      'server_time': serverNow.toIso8601String(),
    };

/// `received` is the DEVICE clock at the moment the response arrived.
JoinEligibility _join(DateTime serverNow,
        {String reason = 'OK', DateTime? received}) =>
    JoinEligibility.tryParse(_joinJson(serverNow, reason: reason),
        receivedAt: received ?? serverNow)!;

class _FailingEngine extends FakeAgoraEngine {
  @override
  Future<void> initialize(String appId) async => throw StateError('boom');
}

void main() {
  group('doctor join gate boundaries (server-derived instants)', () {
    test('16 minutes before start the room is locked; countdown is one minute',
        () {
      final now = _start.subtract(const Duration(minutes: 16));
      expect(_join(now).gate(now), JoinGateState.locked);
      expect(_join(now).untilOpen(now), const Duration(minutes: 1));
    });
    test('exactly 15 minutes before start the room opens', () {
      final now = _start.subtract(const Duration(minutes: 15));
      expect(_join(now).gate(now), JoinGateState.open);
      final early = now.subtract(const Duration(seconds: 1));
      expect(_join(early).gate(early), JoinGateState.locked);
    });
    test('start, end and grace are open; one second after the grace it closes',
        () {
      for (final t in [
        _start,
        _end,
        _end.add(const Duration(minutes: 30)),
        _close
      ]) {
        expect(_join(t).gate(t), JoinGateState.open, reason: '$t');
      }
      final late = _close.add(const Duration(seconds: 1));
      expect(_join(late).gate(late), JoinGateState.closed);
    });
    test('completed, cancelled and interrupted sessions are unavailable', () {
      final now = _start.add(const Duration(minutes: 5));
      for (final reason in ['INVALID_STATE', 'NOT_VIDEO', 'NEEDS_REVIEW']) {
        expect(_join(now, reason: reason).gate(now), JoinGateState.unavailable,
            reason: reason);
      }
    });
    test('an older backend without a join object never invents a lock', () {
      expect(JoinEligibility.tryParse(null), isNull);
      expect(joinIsOpen(null), isTrue);
    });
    test('a device clock hours off (any timezone) is corrected by the server',
        () {
      for (final drift in [3, 6, -6, 1]) {
        final serverNow = _start.subtract(const Duration(minutes: 16));
        final deviceNow = serverNow.add(Duration(hours: drift));
        final j = _join(serverNow, received: deviceNow);
        expect(j.gate(deviceNow), JoinGateState.locked, reason: 'drift $drift');
        expect(j.gate(deviceNow.add(const Duration(seconds: 61))),
            JoinGateState.open,
            reason: 'drift $drift');
      }
    });
  });

  group('appointment rows disable Start/Join outside the window', () {
    Map<String, dynamic> row(DateTime serverNow,
            {String modality = 'VIDEO', String reason = 'OK'}) =>
        {
          'id': 'a1',
          'status': 'CONFIRMED',
          'modality': modality,
          'start_time': _start.toIso8601String(),
          'end_time': _end.toIso8601String(),
          'queue_actionable': true,
          'join': _joinJson(serverNow, reason: reason),
        };

    test('locked video row is blocked and labelled with the EAT open time', () {
      final now = DateTime.now().toUtc();
      final a = AppointmentDto.fromJson({
        'id': 'a1',
        'status': 'CONFIRMED',
        'modality': 'VIDEO',
        'queue_actionable': true,
        'join': {
          'eligible': false,
          'reason': 'NOT_YET_OPEN',
          'join_open_at':
              now.add(const Duration(minutes: 30)).toIso8601String(),
          'join_close_at':
              now.add(const Duration(minutes: 120)).toIso8601String(),
          'server_time': now.toIso8601String(),
        },
      });
      expect(a.joinBlocked, isTrue);
      expect(a.joinLockedLabel, startsWith('Room opens at '));
      expect(a.joinLockedLabel, contains('EAT'));
    });
    test('a device clock 3h AHEAD of the server cannot unlock a row', () {
      final device = DateTime.now().toUtc();
      final server = device.subtract(const Duration(hours: 3));
      // open in 30 min SERVER time; a naive device-vs-open_at check says "open".
      final a = AppointmentDto.fromJson({
        'id': 'a1',
        'status': 'CONFIRMED',
        'modality': 'VIDEO',
        'join': {
          'eligible': false,
          'reason': 'NOT_YET_OPEN',
          'join_open_at':
              server.add(const Duration(minutes: 30)).toIso8601String(),
          'join_close_at':
              server.add(const Duration(minutes: 120)).toIso8601String(),
          'server_time': server.toIso8601String(),
        },
      });
      expect(a.joinBlocked, isTrue);
    });
    test('an open window leaves Start/Join enabled', () {
      final now = DateTime.now().toUtc();
      final open = {
        'id': 'a1',
        'status': 'CONFIRMED',
        'modality': 'VIDEO',
        'queue_actionable': true,
        'join': {
          'eligible': true,
          'reason': 'OK',
          'join_open_at':
              now.subtract(const Duration(minutes: 5)).toIso8601String(),
          'join_close_at':
              now.add(const Duration(minutes: 60)).toIso8601String(),
          'server_time': now.toIso8601String(),
        },
      };
      final a = AppointmentDto.fromJson(open);
      expect(a.joinBlocked, isFalse);
      expect(a.joinLockedLabel, isNull);
    });
    test('chat appointments and rows without join are never blocked', () {
      expect(
          AppointmentDto.fromJson({...row(_start), 'modality': 'CHAT'})
              .joinBlocked,
          isFalse);
      expect(
          AppointmentDto.fromJson({
            'id': 'a2',
            'status': 'CONFIRMED',
            'modality': 'VIDEO',
          }).joinBlocked,
          isFalse);
    });
    test('completed/needs-review video rows are blocked with their own wording',
        () {
      final a = AppointmentDto.fromJson(row(_start, reason: 'NEEDS_REVIEW'));
      expect(a.joinBlocked, isTrue);
      expect(a.joinLockedLabel, 'Consultation cannot be joined');
    });
  });

  group('wording and RTC-token refusal mapping (stable code, not text)', () {
    test('too early names the open time in EAT, with the date when not today',
        () {
      expect(
          joinDenialText('NOT_YET_OPEN',
              opensAt: _open, now: DateTime.utc(2026, 11, 11, 9)),
          'The consultation room opens at 2:45 PM EAT.');
      expect(
          joinDenialText('NOT_YET_OPEN',
              opensAt: _open, now: DateTime.utc(2026, 11, 9, 9)),
          'The consultation room opens at 11 Nov, 2:45 PM EAT.');
    });
    ApiException api(int? status, String? code) =>
        ApiException(status, 'ignored human text', code: code);
    test('deterministic denials are not retryable', () {
      for (final code in [
        'NOT_YET_OPEN',
        'WINDOW_CLOSED',
        'INVALID_STATE',
        'NEEDS_REVIEW'
      ]) {
        final f =
            mapMintFailure(api(code == 'INVALID_STATE' ? 409 : 403, code));
        expect(f.reason, code);
        expect(f.retryable, isFalse, reason: code);
      }
    });
    test('offline and 5xx are retryable and distinct', () {
      expect(mapMintFailure(api(null, null)).reason, 'NETWORK');
      expect(mapMintFailure(api(null, null)).retryable, isTrue);
      expect(mapMintFailure(api(503, 'SERVICE_UNAVAILABLE')).reason, 'SERVICE');
      expect(
          mapMintFailure(api(409, 'RESOURCE_CONFLICT')).reason, 'ALREADY_LIVE');
      expect(
          mapMintFailure(api(401, 'UNAUTHORIZED')).reason, 'UNAUTHENTICATED');
    });
  });

  group('CallController (doctor) failure handling', () {
    RtcTokenData token() => const RtcTokenData(
        appId: 'a',
        channelId: 'rtc_x',
        token: 'SECRET-TOKEN-VALUE',
        uid: 1,
        expiresInSeconds: 60);
    test('a denied room never initialises the engine and offers no retry',
        () async {
      final engine = FakeAgoraEngine();
      final c = CallController(
        engine: engine,
        mintToken: () async => throw const CallAuthorizationException(
            'NOT_YET_OPEN', 'The consultation room opens at 2:45 PM EAT.'),
        requestPermissions: () async => true,
      );
      await c.start();
      expect(c.state, CallScreenState.failed);
      expect(c.errorMessage, 'The consultation room opens at 2:45 PM EAT.');
      expect(c.retryable, isFalse);
      expect(c.diagnostic, 'stage=token reason=NOT_YET_OPEN');
      expect(engine.initialized, isFalse);
    });
    test('a network failure at authorization is reported as network', () async {
      final c = CallController(
        engine: FakeAgoraEngine(),
        mintToken: () async => throw StateError('socket'),
        requestPermissions: () async => true,
      );
      await c.start();
      expect(c.errorMessage, startsWith('Network unavailable'));
      expect(c.retryable, isTrue);
    });
    test('an engine failure AFTER authorization is a video-service error',
        () async {
      final c = CallController(
        engine: _FailingEngine(),
        mintToken: () async => token(),
        requestPermissions: () async => true,
      );
      await c.start();
      expect(c.errorMessage, contains('video service'));
      expect(c.diagnostic, 'stage=engine_init type=StateError');
      expect(c.diagnostic, isNot(contains('SECRET-TOKEN-VALUE')));
    });
  });
}
