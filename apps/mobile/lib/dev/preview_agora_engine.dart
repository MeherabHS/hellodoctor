// Debug-only stand-in for the Agora engine, used by the design previews so
// the call screen can be shown without a device camera, a token or a
// network. It "connects" at once and draws labelled placeholder video.

import 'package:flutter/material.dart';

import '../core/video/agora_engine.dart';

class PreviewAgoraEngine implements AgoraEngineAdapter {
  VoidCallback? _joined;
  void Function(int)? _remoteJoined;
  void Function(CallConnectionState)? _state;

  @override
  Future<void> initialize(String appId) async {}

  @override
  void setEventHandlers({
    required VoidCallback onJoinSuccess,
    required void Function(int remoteUid) onRemoteUserJoined,
    required void Function(int remoteUid) onRemoteUserLeft,
    required void Function(CallConnectionState state) onConnectionStateChanged,
    required VoidCallback onTokenWillExpire,
    required void Function(String reason) onError,
  }) {
    _joined = onJoinSuccess;
    _remoteJoined = onRemoteUserJoined;
    _state = onConnectionStateChanged;
  }

  @override
  Future<void> enableVideoAndStartPreview() async {}

  @override
  Future<void> joinChannel(
      {required String token,
      required String channelId,
      required int uid}) async {
    _joined?.call();
    _state?.call(CallConnectionState.connected);
    _remoteJoined?.call(7);
  }

  @override
  Future<void> leaveChannel() async {}
  @override
  Future<void> muteLocalAudio(bool muted) async {}
  @override
  Future<void> muteLocalVideo(bool muted) async {}
  @override
  Future<void> switchCamera() async {}
  @override
  Future<void> renewToken(String token) async {}
  @override
  Future<void> release() async {}

  @override
  Widget buildLocalPreview() => const _PlaceholderVideo(
      colors: [Color(0xFF4B5563), Color(0xFF1F2937)],
      label: 'You',
      small: true);

  @override
  Widget buildRemoteView({required int remoteUid, required String channelId}) =>
      const _PlaceholderVideo(
          colors: [Color(0xFF1E3A4C), Color(0xFF0B1A24)],
          label: 'Sample video (preview)');
}

class _PlaceholderVideo extends StatelessWidget {
  const _PlaceholderVideo(
      {required this.colors, required this.label, this.small = false});
  final List<Color> colors;
  final String label;
  final bool small;
  @override
  Widget build(BuildContext context) => DecoratedBox(
        decoration: BoxDecoration(
            gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: colors)),
        child: Center(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Icon(Icons.person_rounded,
                size: small ? 56 : 220, color: Colors.white24),
            Text(label,
                style: TextStyle(
                    color: Colors.white54, fontSize: small ? 11 : 14)),
          ]),
        ),
      );
}
