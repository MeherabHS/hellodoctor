import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../features/patient/booking/booking_models.dart';
import 'agora_engine.dart';

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
/// leaving a call. Consultation completion remains an entirely separate,
/// Doctor-initiated action elsewhere in the system.
class CallController extends ChangeNotifier {
  CallController({
    required this.engine,
    required this.mintToken,
    required this.requestPermissions,
  });

  final AgoraEngineAdapter engine;
  final Future<RtcSessionDto> Function() mintToken;

  /// Returns true only if BOTH camera and microphone are granted. Injected
  /// so tests can simulate grant/deny without touching the OS permission
  /// dialog.
  final Future<bool> Function() requestPermissions;

  CallScreenState state = CallScreenState.requestingPermissions;
  RtcSessionDto? _session;
  String? get channelId => _session?.channelName;
  int? remoteUid;
  bool audioMuted = false;
  bool videoEnabled = true;
  String? errorMessage;
  bool _disposed = false;
  bool _wasVideoEnabledBeforeBackground = true;
  Timer? _tokenRenewalGuard;

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
      final session = await mintToken();
      if (_disposed) return;
      _session = session;
      await engine.initialize(session.appId);
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
        token: session.token,
        channelId: session.channelName,
        uid: session.uid,
      );
    } catch (error) {
      if (_disposed) return;
      errorMessage =
          'The consultation call could not be connected. Please try again.';
      _setState(CallScreenState.failed);
    }
  }

  void _onJoinSuccess() {
    if (_disposed) return;
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
        _setState(CallScreenState.connected);
        break;
      case CallConnectionState.reconnecting:
        _setState(CallScreenState.reconnecting);
        break;
      case CallConnectionState.failed:
        errorMessage =
            'The connection was lost and could not be recovered. Please try again.';
        _setState(CallScreenState.failed);
        break;
      case CallConnectionState.left:
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
        _session = fresh;
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

  Future<void> leave() async {
    _tokenRenewalGuard?.cancel();
    await engine.release();
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
    super.dispose();
  }
}
