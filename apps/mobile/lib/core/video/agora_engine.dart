import 'package:agora_rtc_engine/agora_rtc_engine.dart';
import 'package:flutter/widgets.dart';

/// This app's own connection-state vocabulary, decoupled from Agora's
/// [ConnectionStateType] — [CallController] depends only on this, never on
/// the Agora SDK's own types, so it (and its tests) never need a real engine.
enum CallConnectionState { connecting, connected, reconnecting, failed, left }

/// The minimal surface [CallController] needs from a real-time engine.
/// [RealAgoraEngine] implements this against the actual Agora SDK;
/// `FakeAgoraEngine` (test-only, see `test/core/video/call_controller_test.dart`)
/// implements it against nothing at all, so the controller's state-machine
/// logic — join/leave/mute/camera-switch/reconnect/token-renewal — is
/// exercised by real unit tests without ever touching a platform channel.
abstract class AgoraEngineAdapter {
  Future<void> initialize(String appId);

  /// Registers callbacks. [onError] receives Agora's own human-readable
  /// error description — never fabricated, always exactly what the SDK
  /// reported.
  void setEventHandlers({
    required VoidCallback onJoinSuccess,
    required void Function(int remoteUid) onRemoteUserJoined,
    required void Function(int remoteUid) onRemoteUserLeft,
    required void Function(CallConnectionState state) onConnectionStateChanged,
    required VoidCallback onTokenWillExpire,
    required void Function(String reason) onError,
  });

  Future<void> enableVideoAndStartPreview();
  Future<void> joinChannel({
    required String token,
    required String channelId,
    required int uid,
  });
  Future<void> leaveChannel();
  Future<void> muteLocalAudio(bool muted);
  Future<void> muteLocalVideo(bool muted);
  Future<void> switchCamera();
  Future<void> renewToken(String token);
  Future<void> release();

  /// This device's own camera preview. Only valid to call after
  /// [enableVideoAndStartPreview].
  Widget buildLocalPreview();

  /// The remote participant's video. Only valid once that `uid` has actually
  /// joined (reported via [onRemoteUserJoined] above) — this is never shown
  /// speculatively.
  Widget buildRemoteView({required int remoteUid, required String channelId});
}

/// Real implementation backed by the Agora Flutter SDK
/// (`package:agora_rtc_engine`). The App Certificate never appears here or
/// anywhere else in this app — only the App ID (not secret) and a
/// server-minted token are ever used, both obtained from the backend's own
/// `POST /consultations/:id/rtc-token` endpoint (CLAUDE.md §1.10 /
/// Agora-architecture rule: no custom signaling, no client-side token
/// minting, Agora RTC SDK exclusively).
class RealAgoraEngine implements AgoraEngineAdapter {
  RtcEngine? _engine;

  RtcEngine get _requireEngine {
    final engine = _engine;
    if (engine == null) {
      throw StateError(
          'AgoraEngineAdapter.initialize() must be awaited before use.');
    }
    return engine;
  }

  @override
  Future<void> initialize(String appId) async {
    final engine = createAgoraRtcEngine();
    await engine.initialize(RtcEngineContext(
      appId: appId,
      channelProfile: ChannelProfileType.channelProfileCommunication,
    ));
    _engine = engine;
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
    _requireEngine.registerEventHandler(RtcEngineEventHandler(
      onJoinChannelSuccess: (connection, elapsed) => onJoinSuccess(),
      onUserJoined: (connection, remoteUid, elapsed) =>
          onRemoteUserJoined(remoteUid),
      onUserOffline: (connection, remoteUid, reason) =>
          onRemoteUserLeft(remoteUid),
      onConnectionStateChanged: (connection, state, reason) =>
          onConnectionStateChanged(_mapConnectionState(state)),
      onTokenPrivilegeWillExpire: (connection, token) => onTokenWillExpire(),
      onError: (err, msg) => onError(msg),
      onLeaveChannel: (connection, stats) {},
    ));
  }

  CallConnectionState _mapConnectionState(ConnectionStateType state) {
    switch (state) {
      case ConnectionStateType.connectionStateConnecting:
        return CallConnectionState.connecting;
      case ConnectionStateType.connectionStateConnected:
        return CallConnectionState.connected;
      case ConnectionStateType.connectionStateReconnecting:
        return CallConnectionState.reconnecting;
      case ConnectionStateType.connectionStateFailed:
        return CallConnectionState.failed;
      case ConnectionStateType.connectionStateDisconnected:
        return CallConnectionState.left;
    }
  }

  @override
  Future<void> enableVideoAndStartPreview() async {
    await _requireEngine.enableVideo();
    await _requireEngine.startPreview();
  }

  @override
  Future<void> joinChannel({
    required String token,
    required String channelId,
    required int uid,
  }) async {
    await _requireEngine.joinChannel(
      token: token,
      channelId: channelId,
      uid: uid,
      options: const ChannelMediaOptions(
        clientRoleType: ClientRoleType.clientRoleBroadcaster,
        channelProfile: ChannelProfileType.channelProfileCommunication,
      ),
    );
  }

  @override
  Future<void> leaveChannel() => _requireEngine.leaveChannel();

  @override
  Future<void> muteLocalAudio(bool muted) =>
      _requireEngine.muteLocalAudioStream(muted);

  @override
  Future<void> muteLocalVideo(bool muted) =>
      _requireEngine.muteLocalVideoStream(muted);

  @override
  Future<void> switchCamera() => _requireEngine.switchCamera();

  @override
  Future<void> renewToken(String token) => _requireEngine.renewToken(token);

  @override
  Future<void> release() async {
    final engine = _engine;
    _engine = null;
    if (engine != null) {
      await engine.leaveChannel();
      await engine.release();
    }
  }

  @override
  Widget buildLocalPreview() {
    return AgoraVideoView(
      controller: VideoViewController(
        rtcEngine: _requireEngine,
        canvas: const VideoCanvas(uid: 0),
      ),
    );
  }

  @override
  Widget buildRemoteView({required int remoteUid, required String channelId}) {
    return AgoraVideoView(
      controller: VideoViewController.remote(
        rtcEngine: _requireEngine,
        canvas: VideoCanvas(uid: remoteUid),
        connection: RtcConnection(channelId: channelId),
      ),
    );
  }
}
