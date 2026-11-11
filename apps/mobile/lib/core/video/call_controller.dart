import 'dart:async';

import 'package:flutter/foundation.dart';

import 'agora_engine.dart';

/// The server's own `POST /consultations/:id/rtc-token` response, fully
/// parsed (unlike the old placeholder's `AgoraSession`, which only kept
/// `channel`/`appId` — a real join also needs the minted `token` and the
/// server-derived `uid`).
class RtcTokenData {
  const RtcTokenData({
    required this.appId,
    required this.channelId,
    required this.token,
    required this.uid,
    required this.expiresInSeconds,
  });

  final String appId;
  final String channelId;
  final String token;
  final int uid;
  final int expiresInSeconds;

  factory RtcTokenData.fromJson(Map<String, dynamic> json) {
    final appId = json['app_id']?.toString();
    final channelId = json['channel_name']?.toString();
    final token = json['token']?.toString();
    final uid = json['uid'];
    if (appId == null ||
        appId.isEmpty ||
        channelId == null ||
        channelId.isEmpty ||
        token == null ||
        token.isEmpty ||
        uid == null) {
      throw const FormatException(
          'The RTC authorization response is incomplete.');
    }
    return RtcTokenData(
      appId: appId,
      channelId: channelId,
      token: token,
      uid: uid is int ? uid : int.parse(uid.toString()),
      expiresInSeconds: json['expires_in'] is int
          ? json['expires_in'] as int
          : int.tryParse(json['expires_in']?.toString() ?? '') ?? 3600,
    );
  }
}

/// What the call screen is currently showing — every state the user's own
/// requirements named (loading, connecting, connected, reconnecting, failed)
/// plus the two that genuinely precede them for a real SDK session
/// (permission request, and its denial).
enum CallScreenState {
  requestingPermissions,
  permissionDenied,
  authorizing,
  connecting,
  connected,
  reconnecting,
  failed,
  left,
}

/// Drives one consultation call end-to-end: permission request → token mint
/// → engine join → connected/reconnecting/failed, plus the in-call controls
/// (mute, camera toggle/switch, leave) and token renewal. Deliberately knows
/// nothing about Agora types directly (only [AgoraEngineAdapter]'s own
/// vocabulary) and nothing about HTTP (only the injected [mintToken]
/// callback) — both are real implementations in production and fakes in
/// `call_controller_test.dart`, which is what makes this state machine
/// directly unit-testable without a platform channel or a network call.
///
/// This controller never marks anything "attended" or "completed" and never
/// submits any clinical/financial side effect — leaving a call is just
/// leaving a call. That stays an explicit, separate, doctor-initiated action
/// elsewhere in the app (`DoctorCubit`'s own complete/complete-no-rx flow).
class CallController extends ChangeNotifier {
  CallController({
    required this.engine,
    required this.mintToken,
    required this.requestPermissions,
    this.telemetrySubmit,
    this.telemetryPerspective = 'DOCTOR',
  });

  final AgoraEngineAdapter engine;
  final Future<RtcTokenData> Function() mintToken;

  /// Returns true only if BOTH camera and microphone are granted. Injected
  /// so tests can simulate grant/deny without touching the OS permission
  /// dialog.
  final Future<bool> Function() requestPermissions;
  final Future<void> Function(List<Map<String, dynamic>> events)?
      telemetrySubmit;
  final String telemetryPerspective;

  CallScreenState state = CallScreenState.requestingPermissions;
  RtcTokenData? _tokenData;
  String? get channelId => _tokenData?.channelId;
  int? remoteUid;
  bool audioMuted = false;
  bool videoEnabled = true;
  String? errorMessage;
  bool _disposed = false;
  bool _wasVideoEnabledBeforeBackground = true;
  Timer? _tokenRenewalGuard;
  Timer? _telemetryFlushTimer;
  final List<Map<String, dynamic>> _telemetryQueue = <Map<String, dynamic>>[];
  bool _joinedAtLeastOnce = false;

  /// True only after the engine's own connection-state callback has reported
  /// `connected` at least once. Deliberately distinct from
  /// [_joinedAtLeastOnce]: the Agora SDK's documented event order for every
  /// successful join is `onConnectionStateChanged(connecting)` ->
  /// `onJoinChannelSuccess` -> `onConnectionStateChanged(connected)` — that
  /// last, SDK-driven `connected` event for the *original* join would
  /// otherwise be indistinguishable from a genuine reconnect if gated on
  /// [_joinedAtLeastOnce] alone (which [_onJoinSuccess] already set to true
  /// one callback earlier), misreporting every ordinary, never-interrupted
  /// call as a reconnect in clinical telemetry.
  bool _hasReachedConnectedBefore = false;

  void _queueTelemetry(String suffix) {
    if (telemetrySubmit == null || _disposed) return;
    if (_telemetryQueue.length >= 20) _telemetryQueue.removeAt(0);
    _telemetryQueue.add({
      'event_type': '${telemetryPerspective}_$suffix',
      'occurred_at': DateTime.now().toUtc().toIso8601String(),
    });
    _telemetryFlushTimer ??= Timer(const Duration(seconds: 1), _flushTelemetry);
  }

  void _flushTelemetry() {
    _telemetryFlushTimer = null;
    if (_telemetryQueue.isEmpty || telemetrySubmit == null || _disposed) return;
    final events = List<Map<String, dynamic>>.from(_telemetryQueue);
    _telemetryQueue.clear();
    unawaited(telemetrySubmit!(events));
  }

  Future<void> start() async {
    _setState(CallScreenState.requestingPermissions);
    final granted = await requestPermissions();
    if (_disposed) return;
    if (!granted) {
      _setState(CallScreenState.permissionDenied);
      return;
    }
    await _authorizeAndJoin();
  }

  Future<void> _authorizeAndJoin() async {
    _setState(CallScreenState.authorizing);
    try {
      final token = await mintToken();
      if (_disposed) return;
      _tokenData = token;
      await engine.initialize(token.appId);
      if (_disposed) return;
      engine.setEventHandlers(
        onJoinSuccess: _onJoinSuccess,
        onRemoteUserJoined: _onRemoteUserJoined,
        onRemoteUserLeft: _onRemoteUserLeft,
        onConnectionStateChanged: _onConnectionStateChanged,
        onTokenWillExpire: _onTokenWillExpire,
        onError: _onEngineError,
      );
      await engine.enableVideoAndStartPreview();
      if (_disposed) return;
      _setState(CallScreenState.connecting);
      await engine.joinChannel(
        token: token.token,
        channelId: token.channelId,
        uid: token.uid,
      );
    } catch (error) {
      if (_disposed) return;
      errorMessage = _describeError(error);
      _setState(CallScreenState.failed);
    }
  }

  String _describeError(Object error) {
    if (error is FormatException) return error.message;
    return 'The consultation call could not be connected. Please try again.';
  }

  void _onJoinSuccess() {
    if (_disposed) return;
    _joinedAtLeastOnce = true;
    _queueTelemetry('JOINED');
    _setState(CallScreenState.connected);
  }

  void _onRemoteUserJoined(int uid) {
    if (_disposed) return;
    remoteUid = uid;
    notifyListeners();
  }

  void _onRemoteUserLeft(int uid) {
    if (_disposed) return;
    if (remoteUid == uid) {
      remoteUid = null;
      notifyListeners();
    }
  }

  void _onConnectionStateChanged(CallConnectionState connectionState) {
    if (_disposed) return;
    switch (connectionState) {
      case CallConnectionState.connecting:
        if (state != CallScreenState.connected) {
          _setState(CallScreenState.connecting);
        }
        break;
      case CallConnectionState.connected:
        if (_hasReachedConnectedBefore) _queueTelemetry('RECONNECTED');
        _hasReachedConnectedBefore = true;
        _setState(CallScreenState.connected);
        break;
      case CallConnectionState.reconnecting:
        if (_joinedAtLeastOnce) _queueTelemetry('RECONNECTING');
        _setState(CallScreenState.reconnecting);
        break;
      case CallConnectionState.failed:
        if (_joinedAtLeastOnce) _queueTelemetry('DISCONNECTED');
        errorMessage =
            'The connection was lost and could not be recovered. Please try again.';
        _setState(CallScreenState.failed);
        break;
      case CallConnectionState.left:
        if (_joinedAtLeastOnce) _queueTelemetry('DISCONNECTED');
        if (state != CallScreenState.failed) {
          _setState(CallScreenState.left);
        }
        break;
    }
  }

  void _onTokenWillExpire() {
    // Fire-and-forget by design: a renewal failure must not interrupt an
    // otherwise-healthy call. If it genuinely fails, the token then expires
    // for real and the SDK's own connection-state callback (above) reports
    // the resulting disconnect/failure honestly — never papered over.
    _tokenRenewalGuard?.cancel();
    _tokenRenewalGuard = Timer(Duration.zero, () async {
      try {
        final fresh = await mintToken();
        if (_disposed) return;
        _tokenData = fresh;
        await engine.renewToken(fresh.token);
      } catch (_) {
        // Swallowed — see comment above.
      }
    });
  }

  void _onEngineError(String reason) {
    if (_disposed) return;
    errorMessage = reason;
    _setState(CallScreenState.failed);
  }

  Future<void> retry() async {
    errorMessage = null;
    remoteUid = null;
    await engine.release();
    await start();
  }

  Future<void> toggleMute() async {
    audioMuted = !audioMuted;
    notifyListeners();
    await engine.muteLocalAudio(audioMuted);
  }

  Future<void> toggleVideo() async {
    videoEnabled = !videoEnabled;
    notifyListeners();
    await engine.muteLocalVideo(!videoEnabled);
  }

  Future<void> switchCamera() => engine.switchCamera();

  bool _left = false;

  /// Idempotent: a widget's `dispose()` calling this again after its own
  /// explicit "leave and pop" action already called it (`Navigator.pop`
  /// synchronously triggers `State.dispose()`, which has no way to know the
  /// leave already happened) must never queue a second `DISCONNECTED`
  /// telemetry event or re-release an already-released engine for the same
  /// call.
  Future<void> leave() async {
    if (_left) return;
    _left = true;
    _tokenRenewalGuard?.cancel();
    await engine.release();
    if (_joinedAtLeastOnce) {
      _queueTelemetry('DISCONNECTED');
      _flushTelemetry();
    }
    if (_disposed) return;
    _setState(CallScreenState.left);
  }

  /// Backgrounding turns off the local camera preview (privacy + battery)
  /// without leaving the channel — a brief app-switch must not drop the
  /// call. Audio keeps flowing throughout. Foregrounding restores whatever
  /// the user's own video preference was before backgrounding.
  Future<void> handleAppBackgrounded() async {
    if (state != CallScreenState.connected &&
        state != CallScreenState.reconnecting) {
      return;
    }
    _wasVideoEnabledBeforeBackground = videoEnabled;
    if (videoEnabled) {
      await engine.muteLocalVideo(true);
    }
  }

  Future<void> handleAppForegrounded() async {
    if (state != CallScreenState.connected &&
        state != CallScreenState.reconnecting) {
      return;
    }
    if (_wasVideoEnabledBeforeBackground && videoEnabled) {
      await engine.muteLocalVideo(false);
    }
  }

  void _setState(CallScreenState next) {
    state = next;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _tokenRenewalGuard?.cancel();
    _telemetryFlushTimer?.cancel();
    _flushTelemetry();
    super.dispose();
  }
}
