import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/video/agora_engine.dart';
import 'package:hellodoctor_mobile/core/video/call_controller.dart';

// ============================================================================
// IMPORTANT — scope of what this file proves, read before trusting a green
// run: every test below runs against [FakeAgoraEngine], a plain in-memory
// Dart object with NO platform channel and NO real Agora SDK involved at
// all. A pass here proves [CallController]'s own state-machine logic is
// correct (it reacts to exactly the events a provider would report, and
// never claims "connected" before being told so) — it proves NOTHING about
// whether the real `agora_rtc_engine` plugin, a real App ID/Certificate, or
// Agora's live servers actually work. That can only be confirmed by an
// actual two-device call against real Agora credentials (see the Stage B3
// integration-readiness report for the minimum test procedure). Do not cite
// "`flutter test` passed" as evidence of a verified Agora connection.
// ============================================================================

/// A fully in-memory stand-in for the real Agora SDK — no platform channel,
/// no network. Records every call so tests can assert on exactly what the
/// controller asked the engine to do, and lets tests fire the engine's own
/// callbacks (join success, remote join/leave, connection-state changes,
/// token-will-expire) to simulate real provider-reported events without
/// ever fabricating one the controller didn't explicitly request or the
/// "provider" didn't explicitly report.
class FakeAgoraEngine implements AgoraEngineAdapter {
  bool initialized = false;
  String? initializedAppId;
  bool previewStarted = false;
  bool joined = false;
  String? joinedToken;
  String? joinedChannelId;
  int? joinedUid;
  bool released = false;
  bool? lastAudioMuted;
  bool? lastVideoMuted;
  int switchCameraCalls = 0;
  String? lastRenewedToken;

  VoidCallback? _onJoinSuccess;
  void Function(int)? _onRemoteUserJoined;
  void Function(int)? _onRemoteUserLeft;
  void Function(CallConnectionState)? _onConnectionStateChanged;
  VoidCallback? _onTokenWillExpire;
  void Function(String)? _onError;

  @override
  Future<void> initialize(String appId) async {
    initialized = true;
    initializedAppId = appId;
  }

  @override
  void setEventHandlers({
    required VoidCallback onJoinSuccess,
    required void Function(int remoteUid) onRemoteUserJoined,
    required void Function(int remoteUid) onRemoteUserLeft,
    required void Function(CallConnectionState state) onConnectionStateChanged,
    required VoidCallback onTokenWillExpire,
    required void Function(String reason) onError,
  }) {
    _onJoinSuccess = onJoinSuccess;
    _onRemoteUserJoined = onRemoteUserJoined;
    _onRemoteUserLeft = onRemoteUserLeft;
    _onConnectionStateChanged = onConnectionStateChanged;
    _onTokenWillExpire = onTokenWillExpire;
    _onError = onError;
  }

  @override
  Future<void> enableVideoAndStartPreview() async {
    previewStarted = true;
  }

  @override
  Future<void> joinChannel({
    required String token,
    required String channelId,
    required int uid,
  }) async {
    joined = true;
    joinedToken = token;
    joinedChannelId = channelId;
    joinedUid = uid;
  }

  @override
  Future<void> leaveChannel() async {
    joined = false;
  }

  @override
  Future<void> muteLocalAudio(bool muted) async {
    lastAudioMuted = muted;
  }

  @override
  Future<void> muteLocalVideo(bool muted) async {
    lastVideoMuted = muted;
  }

  @override
  Future<void> switchCamera() async {
    switchCameraCalls++;
  }

  @override
  Future<void> renewToken(String token) async {
    lastRenewedToken = token;
  }

  @override
  Future<void> release() async {
    released = true;
    joined = false;
  }

  @override
  Widget buildLocalPreview() => const SizedBox.shrink();

  @override
  Widget buildRemoteView({required int remoteUid, required String channelId}) =>
      const SizedBox.shrink();

  // ---- test-only simulation hooks: fire exactly what a real provider
  // callback would fire, nothing more ----
  void simulateJoinSuccess() => _onJoinSuccess?.call();
  void simulateRemoteUserJoined(int uid) => _onRemoteUserJoined?.call(uid);
  void simulateRemoteUserLeft(int uid) => _onRemoteUserLeft?.call(uid);
  void simulateConnectionState(CallConnectionState state) =>
      _onConnectionStateChanged?.call(state);
  void simulateTokenWillExpire() => _onTokenWillExpire?.call();
  void simulateError(String reason) => _onError?.call(reason);
}

RtcTokenData _token({String token = 't1', int uid = 42}) => RtcTokenData(
      appId: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      channelId: 'rtc_channel',
      token: token,
      uid: uid,
      expiresInSeconds: 3600,
    );

void main() {
  group('RtcTokenData.fromJson', () {
    test('parses the full real rtc-token response', () {
      final data = RtcTokenData.fromJson({
        'channel_name': 'rtc_abc123',
        'token': '007eAwARealToken',
        'uid': 123456789,
        'app_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        'expires_in': 1800,
      });
      expect(data.channelId, 'rtc_abc123');
      expect(data.token, '007eAwARealToken');
      expect(data.uid, 123456789);
      expect(data.appId, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
      expect(data.expiresInSeconds, 1800);
    });

    test('tolerates numeric fields arriving as strings', () {
      final data = RtcTokenData.fromJson({
        'channel_name': 'rtc_abc123',
        'token': 'tok',
        'uid': '42',
        'app_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        'expires_in': '900',
      });
      expect(data.uid, 42);
      expect(data.expiresInSeconds, 900);
    });

    test('rejects a response missing any required field', () {
      expect(
        () => RtcTokenData.fromJson({
          'channel_name': 'rtc_abc123',
          'app_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          // 'token' and 'uid' deliberately missing
        }),
        throwsA(isA<FormatException>()),
      );
    });
  });

  group('CallController', () {
    test('requests permissions before anything else', () async {
      final engine = FakeAgoraEngine();
      var permissionsRequested = false;
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async {
          permissionsRequested = true;
          return true;
        },
      );
      expect(controller.state, CallScreenState.requestingPermissions);
      await controller.start();
      expect(permissionsRequested, isTrue);
    });

    test('denied permissions stop before ever touching the engine', () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => false,
      );
      await controller.start();
      expect(controller.state, CallScreenState.permissionDenied);
      expect(engine.initialized, isFalse,
          reason: 'must never touch the engine without granted permissions');
    });

    test(
        'never reports connected until the provider itself reports join success',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      // Joined the channel, but the provider hasn't confirmed success yet —
      // must not be reported as connected.
      expect(controller.state, CallScreenState.connecting);
      expect(engine.joined, isTrue);

      engine.simulateJoinSuccess();
      expect(controller.state, CallScreenState.connected);
    });

    test('joins with exactly the minted token, channel and uid', () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(token: 'real-token', uid: 777),
        requestPermissions: () async => true,
      );
      await controller.start();
      expect(engine.initializedAppId, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
      expect(engine.joinedToken, 'real-token');
      expect(engine.joinedChannelId, 'rtc_channel');
      expect(engine.joinedUid, 777);
      expect(controller.channelId, 'rtc_channel');
    });

    test('a token-mint failure surfaces as the failed state, not a crash',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => throw Exception('network down'),
        requestPermissions: () async => true,
      );
      await controller.start();
      expect(controller.state, CallScreenState.failed);
      expect(controller.errorMessage, isNotNull);
      expect(engine.joined, isFalse);
    });

    test('remote join/leave are reflected exactly as the provider reports',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      engine.simulateJoinSuccess();
      expect(controller.remoteUid, isNull);

      engine.simulateRemoteUserJoined(555);
      expect(controller.remoteUid, 555);

      engine.simulateRemoteUserLeft(555);
      expect(controller.remoteUid, isNull);
    });

    test('provider-reported reconnecting/failed states are surfaced honestly',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      engine.simulateJoinSuccess();
      expect(controller.state, CallScreenState.connected);

      engine.simulateConnectionState(CallConnectionState.reconnecting);
      expect(controller.state, CallScreenState.reconnecting);

      engine.simulateConnectionState(CallConnectionState.failed);
      expect(controller.state, CallScreenState.failed);
      expect(controller.errorMessage, isNotNull);
    });

    test('an engine-reported error surfaces as failed with the real reason',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      engine.simulateError('ERR_TOKEN_EXPIRED');
      expect(controller.state, CallScreenState.failed);
      expect(controller.errorMessage, 'ERR_TOKEN_EXPIRED');
    });

    test('token-will-expire re-mints and renews without dropping the call',
        () async {
      final engine = FakeAgoraEngine();
      var mintCount = 0;
      final controller = CallController(
        engine: engine,
        mintToken: () async {
          mintCount++;
          return _token(token: 'token-$mintCount');
        },
        requestPermissions: () async => true,
      );
      await controller.start();
      engine.simulateJoinSuccess();
      expect(mintCount, 1);

      engine.simulateTokenWillExpire();
      await Future<void>.delayed(Duration.zero);
      expect(mintCount, 2);
      expect(engine.lastRenewedToken, 'token-2');
      expect(controller.state, CallScreenState.connected,
          reason: 'renewal must not interrupt an otherwise-healthy call');
    });

    test('toggleMute flips local state and calls the engine exactly once',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      expect(controller.audioMuted, isFalse);
      await controller.toggleMute();
      expect(controller.audioMuted, isTrue);
      expect(engine.lastAudioMuted, isTrue);
      await controller.toggleMute();
      expect(controller.audioMuted, isFalse);
      expect(engine.lastAudioMuted, isFalse);
    });

    test('toggleVideo flips local state and calls the engine inverted',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      expect(controller.videoEnabled, isTrue);
      await controller.toggleVideo();
      expect(controller.videoEnabled, isFalse);
      expect(engine.lastVideoMuted, isTrue);
    });

    test('switchCamera delegates straight to the engine', () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      await controller.switchCamera();
      expect(engine.switchCameraCalls, 1);
    });

    test('leave releases the engine and never claims completion', () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      engine.simulateJoinSuccess();
      await controller.leave();
      expect(engine.released, isTrue);
      expect(controller.state, CallScreenState.left);
    });

    test('maps provider connection changes to bounded client telemetry',
        () async {
      final engine = FakeAgoraEngine();
      final events = <Map<String, dynamic>>[];
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
        telemetrySubmit: (batch) async => events.addAll(batch),
      );
      await controller.start();
      engine.simulateJoinSuccess();
      engine.simulateConnectionState(CallConnectionState.reconnecting);
      await Future<void>.delayed(const Duration(milliseconds: 1100));
      expect(
          events.map((event) => event['event_type']),
          containsAll(<String>[
            'DOCTOR_JOINED',
            'DOCTOR_RECONNECTING',
          ]));
      await controller.leave();
      expect(events.map((event) => event['event_type']),
          contains('DOCTOR_DISCONNECTED'));
    });

    test('leave() is idempotent: calling it twice queues only one DISCONNECTED',
        () async {
      // Mirrors AgoraPage's real lifecycle: `_leaveAndPop()` awaits leave()
      // then pops the Navigator, which synchronously triggers the widget's
      // own `dispose()` — which also calls leave() again, with no way to
      // know the first call already happened.
      final engine = FakeAgoraEngine();
      final events = <Map<String, dynamic>>[];
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
        telemetrySubmit: (batch) async => events.addAll(batch),
      );
      await controller.start();
      engine.simulateJoinSuccess();
      await controller.leave();
      await controller.leave();
      await Future<void>.delayed(const Duration(milliseconds: 1100));
      expect(
        events.where((event) => event['event_type'] == 'DOCTOR_DISCONNECTED'),
        hasLength(1),
      );
    });

    test('an ordinary uninterrupted join never queues a false reconnect',
        () async {
      // Real Agora SDKs report onConnectionStateChanged(connected) as a
      // follow-up to the SDK's own onJoinChannelSuccess for the very same
      // join — not just onJoinChannelSuccess alone, which is all the test
      // above exercises. A controller that gates "was this a reconnect?" on
      // "have we ever joined" rather than "have we ever actually reached
      // connected" would misreport this ordinary, never-interrupted call.
      final engine = FakeAgoraEngine();
      final events = <Map<String, dynamic>>[];
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
        telemetrySubmit: (batch) async => events.addAll(batch),
      );
      await controller.start();
      engine.simulateJoinSuccess();
      engine.simulateConnectionState(CallConnectionState.connected);
      await Future<void>.delayed(const Duration(milliseconds: 1100));
      expect(
        events.map((event) => event['event_type']),
        isNot(contains('DOCTOR_RECONNECTED')),
      );

      // A genuine later drop-and-recover must still be reported.
      engine.simulateConnectionState(CallConnectionState.reconnecting);
      engine.simulateConnectionState(CallConnectionState.connected);
      await Future<void>.delayed(const Duration(milliseconds: 1100));
      expect(
        events.map((event) => event['event_type']),
        contains('DOCTOR_RECONNECTED'),
      );
    });

    test('retry after failure re-runs the full flow', () async {
      final engine = FakeAgoraEngine();
      var shouldFail = true;
      final controller = CallController(
        engine: engine,
        mintToken: () async {
          if (shouldFail) throw Exception('temporary outage');
          return _token();
        },
        requestPermissions: () async => true,
      );
      await controller.start();
      expect(controller.state, CallScreenState.failed);

      shouldFail = false;
      await controller.retry();
      expect(controller.state, CallScreenState.connecting);
      expect(engine.joined, isTrue);
    });

    test('backgrounding mutes local video only while connected, not before',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.handleAppBackgrounded();
      expect(engine.lastVideoMuted, isNull,
          reason: 'not connected yet — nothing to mute');

      await controller.start();
      engine.simulateJoinSuccess();
      await controller.handleAppBackgrounded();
      expect(engine.lastVideoMuted, isTrue);

      await controller.handleAppForegrounded();
      expect(engine.lastVideoMuted, isFalse);
    });

    test(
        'backgrounding never re-enables video on foreground if the user had muted it',
        () async {
      final engine = FakeAgoraEngine();
      final controller = CallController(
        engine: engine,
        mintToken: () async => _token(),
        requestPermissions: () async => true,
      );
      await controller.start();
      engine.simulateJoinSuccess();
      await controller.toggleVideo(); // user turns their own video off
      expect(controller.videoEnabled, isFalse);

      await controller.handleAppBackgrounded();
      await controller.handleAppForegrounded();
      expect(engine.lastVideoMuted, isTrue,
          reason:
              'foregrounding must respect the user\'s own choice, not override it');
    });
  });
}
