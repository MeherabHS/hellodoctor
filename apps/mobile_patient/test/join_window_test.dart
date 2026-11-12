import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/connectivity_service.dart';
import 'package:hellodoctor_mobile_patient/core/video/call_controller.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/join_eligibility.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/waiting_room_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/video/mint_failure.dart';
import 'core/video/call_controller_test.dart' show FakeAgoraEngine;
import 'patient_discovery_qna_correction_test.dart' show MemoryTokens, host;

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

void main() {
  group('join gate boundaries (server-derived instants)', () {
    test('16 minutes before start the room is locked, countdown is one minute',
        () {
      final now = _start.subtract(const Duration(minutes: 16));
      final j = _join(now);
      expect(j.gate(now), JoinGateState.locked);
      expect(j.untilOpen(now), const Duration(minutes: 1));
    });
    test('exactly 15 minutes before start the room opens', () {
      final now = _start.subtract(const Duration(minutes: 15));
      expect(_join(now).gate(now), JoinGateState.open);
      final early = now.subtract(const Duration(seconds: 1));
      expect(_join(early).gate(early), JoinGateState.locked);
    });
    test(
        'start, end and the post-end grace are open; one second later it closes',
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
      expect(JoinEligibility.tryParse({'eligible': true}), isNull);
      expect(joinIsOpen(null), isTrue);
    });
  });

  group('wrong device clock / timezone', () {
    test('a device clock hours off is corrected with the server time', () {
      for (final drift in [3, 6, -6, 1]) {
        final serverNow = _start.subtract(const Duration(minutes: 16));
        final deviceNow = serverNow.add(Duration(hours: drift));
        final j = _join(serverNow, received: deviceNow);
        expect(j.gate(deviceNow), JoinGateState.locked, reason: 'drift $drift');
        // 61 s later on the (still wrong) device clock the window has opened.
        expect(j.gate(deviceNow.add(const Duration(seconds: 61))),
            JoinGateState.open,
            reason: 'drift $drift');
      }
    });
    test('local and UTC DateTime instants evaluate identically', () {
      final now = _start.subtract(const Duration(minutes: 16));
      final j = _join(now);
      expect(j.gate(now.toLocal()), j.gate(now.toUtc()));
    });
  });

  group('wording: EAT, with the date when not today', () {
    test('too early names the room-open time in EAT', () {
      final today = DateTime.utc(2026, 11, 11, 9);
      expect(joinDenialText('NOT_YET_OPEN', opensAt: _open, now: today),
          'Your consultation room opens at 2:45 PM EAT.');
      expect(
          joinDenialText('NOT_YET_OPEN',
              opensAt: _open, now: DateTime.utc(2026, 11, 9, 9)),
          'Your consultation room opens at 11 Nov, 2:45 PM EAT.');
      expect(joinDenialText('NOT_YET_OPEN'),
          'Your consultation room is not open yet.');
    });
    test('distinct text per denial reason', () {
      expect(joinDenialText('WINDOW_CLOSED'), contains('window'));
      expect(
          joinDenialText('INVALID_STATE'), contains('not in an active state'));
      expect(joinDenialText('NEEDS_REVIEW'), contains('needs review'));
    });
    test('countdown formatting', () {
      expect(formatCountdown(Duration.zero), 'now');
      expect(formatCountdown(const Duration(seconds: 65)), '1m 05s');
      expect(formatCountdown(const Duration(minutes: 62)), '1h 02m');
    });
  });

  group('RTC-token refusals map by stable code, not by text', () {
    ApiException api(int? status, String? code) =>
        ApiException(status, 'ignored human text', code: code);
    test('too early / closed / invalid state / needs review are not retryable',
        () {
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
      expect(
          mapMintFailure(api(403, 'NOT_YET_OPEN'), opensAt: _open).userMessage,
          contains('EAT'));
    });
    test('offline and 5xx are retryable and distinct from a denial', () {
      final offline = mapMintFailure(api(null, null));
      expect(offline.reason, 'NETWORK');
      expect(offline.retryable, isTrue);
      final svc = mapMintFailure(api(503, 'SERVICE_UNAVAILABLE'));
      expect(svc.reason, 'SERVICE');
      expect(svc.retryable, isTrue);
    });
    test('auth, forbidden and already-live are separate outcomes', () {
      expect(
          mapMintFailure(api(401, 'UNAUTHORIZED')).reason, 'UNAUTHENTICATED');
      expect(mapMintFailure(api(403, 'FORBIDDEN')).reason, 'FORBIDDEN');
      expect(
          mapMintFailure(api(409, 'RESOURCE_CONFLICT')).reason, 'ALREADY_LIVE');
    });
  });

  group('CallController failure handling', () {
    RtcSessionDto session() => RtcSessionDto.fromJson({
          'channel_name': 'rtc_x',
          'token': 'SECRET-TOKEN-VALUE',
          'uid': 1,
          'app_id': 'a',
          'expires_in': 60
        });
    test('a denied room never initialises the engine and offers no retry',
        () async {
      final engine = FakeAgoraEngine();
      final c = CallController(
        engine: engine,
        mintToken: () async => throw const CallAuthorizationException(
            'NOT_YET_OPEN', 'Your consultation room opens at 2:45 PM EAT.'),
        requestPermissions: () async => true,
      );
      await c.start();
      expect(c.state, CallScreenState.failed);
      expect(c.errorMessage, 'Your consultation room opens at 2:45 PM EAT.');
      expect(c.retryable, isFalse);
      expect(c.diagnostic, 'stage=token reason=NOT_YET_OPEN');
      expect(engine.initialized, isFalse,
          reason: 'no Agora initialisation before the server authorises');
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
      expect(c.diagnostic, 'stage=token type=StateError');
    });
    test(
        'an engine failure AFTER authorization is reported as a video-service error',
        () async {
      final c = CallController(
        engine: _FailingEngine(),
        mintToken: () async => session(),
        requestPermissions: () async => true,
      );
      await c.start();
      expect(c.errorMessage, contains('video service'));
      expect(c.diagnostic, 'stage=engine_init type=StateError');
      expect(c.diagnostic, isNot(contains('SECRET-TOKEN-VALUE')),
          reason: 'never includes the token or any message text');
    });
  });

  group('waiting room Join gating', () {
    setUpAll(() async {
      await (FontLoader('Plus Jakarta Sans')
            ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf')))
          .load();
    });

    Future<void> pumpRoom(WidgetTester tester,
        {required Map<String, dynamic> join}) async {
      tester.view.physicalSize = const Size(390, 1400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final dio = Dio();
      final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
      addTearDown(api.dispose);
      final connectivity = ConnectivityService();
      addTearDown(connectivity.dispose);
      dio.interceptors.add(InterceptorsWrapper(onRequest: (o, h) {
        final dynamic data = o.path.endsWith('/join-eligibility')
            ? join
            : {'id': 'apt-1', 'status': 'CONFIRMED'};
        h.resolve(Response(
            requestOptions: o,
            statusCode: 200,
            data: {'success': true, 'data': data}));
      }));
      final family = FamilyProfilesCubit(FamilyProfilesRepository(api));
      addTearDown(family.close);
      final appointment = AppointmentSummaryDto.fromJson({
        'id': 'apt-1',
        'appointment_number': 'APT-1',
        'doctor_id': 'd1',
        'doctor_display_name': 'Dr Test',
        'primary_specialty': 'General',
        'modality': 'VIDEO',
        'status': 'CONFIRMED',
        'start_time': _start.toIso8601String(),
        'end_time': _end.toIso8601String(),
        'join': join,
      });
      await tester.pumpWidget(host(WaitingRoomPage(
          appointment: appointment,
          repo: BookingRepository(api, connectivity: connectivity),
          familyProfilesCubit: family)));
      await tester.pump(const Duration(milliseconds: 50));
    }

    bool joinEnabled(WidgetTester tester) {
      final button = tester.widget<FilledButton>(find.ancestor(
          of: find.text('Join Video Call'),
          matching: find.byWidgetPredicate((w) => w is FilledButton)));
      return button.onPressed != null;
    }

    Future<void> unmount(WidgetTester tester) async {
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
    }

    testWidgets('locked: Join is disabled and says when the room opens in EAT',
        (tester) async {
      final now = DateTime.now().toUtc();
      final start = now.add(const Duration(minutes: 30));
      final j = {
        'eligible': false,
        'reason': 'NOT_YET_OPEN',
        'join_open_at':
            start.subtract(const Duration(minutes: 15)).toIso8601String(),
        'join_close_at':
            start.add(const Duration(minutes: 70)).toIso8601String(),
        'server_time': now.toIso8601String(),
      };
      await pumpRoom(tester, join: j);
      expect(find.byKey(const Key('join-gate-notice')), findsOneWidget);
      expect(find.textContaining('opens at'), findsOneWidget);
      expect(find.textContaining('EAT'), findsWidgets);
      expect(joinEnabled(tester), isFalse);
      await unmount(tester);
    });

    testWidgets('open: Join is enabled and no lock notice is shown',
        (tester) async {
      final now = DateTime.now().toUtc();
      final j = {
        'eligible': true,
        'reason': 'OK',
        'join_open_at':
            now.subtract(const Duration(minutes: 5)).toIso8601String(),
        'join_close_at': now.add(const Duration(minutes: 60)).toIso8601String(),
        'server_time': now.toIso8601String(),
      };
      await pumpRoom(tester, join: j);
      expect(find.byKey(const Key('join-gate-notice')), findsNothing);
      expect(joinEnabled(tester), isTrue);
      await unmount(tester);
    });

    testWidgets(
        'a device clock 3 hours AHEAD of the server cannot unlock the room',
        (tester) async {
      // Server says "now" is 3h behind the device; opens in 30 min SERVER time.
      // A naive device-vs-open_at comparison would call this open.
      final deviceNow = DateTime.now().toUtc();
      final serverNow = deviceNow.subtract(const Duration(hours: 3));
      final j = {
        'eligible': false,
        'reason': 'NOT_YET_OPEN',
        'join_open_at':
            serverNow.add(const Duration(minutes: 30)).toIso8601String(),
        'join_close_at':
            serverNow.add(const Duration(minutes: 120)).toIso8601String(),
        'server_time': serverNow.toIso8601String(),
      };
      await pumpRoom(tester, join: j);
      expect(find.byKey(const Key('join-gate-notice')), findsOneWidget);
      expect(joinEnabled(tester), isFalse);
      await unmount(tester);
    });

    testWidgets(
        'closed and needs-review show their own wording, not a generic error',
        (tester) async {
      final now = DateTime.now().toUtc();
      final closed = {
        'eligible': false,
        'reason': 'WINDOW_CLOSED',
        'join_open_at':
            now.subtract(const Duration(hours: 3)).toIso8601String(),
        'join_close_at':
            now.subtract(const Duration(minutes: 1)).toIso8601String(),
        'server_time': now.toIso8601String(),
      };
      await pumpRoom(tester, join: closed);
      expect(find.textContaining('joining window'), findsOneWidget);
      expect(joinEnabled(tester), isFalse);
      await unmount(tester);

      final review = {...closed, 'reason': 'NEEDS_REVIEW'};
      await pumpRoom(tester, join: review);
      expect(find.textContaining('needs review'), findsOneWidget);
      expect(joinEnabled(tester), isFalse);
      await unmount(tester);
    });
  });
}

class _FailingEngine extends FakeAgoraEngine {
  @override
  Future<void> initialize(String appId) async => throw StateError('boom');
}
