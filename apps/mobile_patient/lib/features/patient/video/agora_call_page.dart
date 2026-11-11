import 'package:flutter/material.dart';
import 'package:permission_handler/permission_handler.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/video/agora_engine.dart';
import '../../../core/video/call_controller.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';

/// Real Agora RTC consultation call screen. Reuses the existing server
/// contract (`POST /consultations/:id/rtc-token`) exclusively — this screen
/// never mints a token itself and never sees the Agora App Certificate
/// (CLAUDE.md §1.10: Agora RTC SDK only, no custom signaling, certificate
/// stays server-side). Joining, leaving, muting, camera switching and
/// connection-state handling are all delegated to [CallController], which is
/// the directly unit-tested piece (`test/core/video/call_controller_test.dart`);
/// this widget is responsible only for rendering whatever state it reports.
///
/// Supersedes the former `AgoraPlaceholderPage` — same navigation contract
/// (same constructor shape), real media instead of a placeholder.
class AgoraCallPage extends StatefulWidget {
  const AgoraCallPage({
    super.key,
    required this.appointmentId,
    required this.repo,
    this.scheduledStartTime,
  });
  final String appointmentId;
  final BookingRepository repo;
  final DateTime? scheduledStartTime;

  @override
  State<AgoraCallPage> createState() => _AgoraCallPageState();
}

class _AgoraCallPageState extends State<AgoraCallPage>
    with WidgetsBindingObserver {
  late final CallController _controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _controller = CallController(
      engine: RealAgoraEngine(),
      mintToken: () => widget.repo.mintRtcToken(widget.appointmentId),
      requestPermissions: _requestPermissions,
      telemetrySubmit: (events) =>
          widget.repo.submitTelemetryEvents(widget.appointmentId, events),
    )..addListener(_onControllerChanged);
    _controller.start();
  }

  Future<bool> _requestPermissions() async {
    final result = await [Permission.camera, Permission.microphone].request();
    return result.values.every((status) => status.isGranted);
  }

  void _onControllerChanged() {
    if (mounted) setState(() {});
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.inactive) {
      _controller.handleAppBackgrounded();
    } else if (state == AppLifecycleState.resumed) {
      _controller.handleAppForegrounded();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _controller.removeListener(_onControllerChanged);
    _controller.leave();
    _controller.dispose();
    super.dispose();
  }

  Future<void> _leaveAndPop() async {
    await _controller.leave();
    if (mounted) Navigator.of(context).pop();
  }

  bool get _isRunningLate {
    final scheduled = widget.scheduledStartTime;
    if (scheduled == null) return false;
    return DateTime.now().difference(scheduled) > const Duration(minutes: 5);
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final inCall = _controller.state == CallScreenState.connected ||
        _controller.state == CallScreenState.reconnecting;
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;
        await _leaveAndPop();
      },
      child: Scaffold(
        backgroundColor: inCall ? Colors.black : PatientTokens.bg,
        appBar: inCall
            ? null
            : AppBar(
                backgroundColor: PatientTokens.surface,
                foregroundColor: PatientTokens.text,
                title: Text(t.callTitle),
              ),
        body: _buildBody(context, t),
      ),
    );
  }

  Widget _buildBody(BuildContext context, AppLocalizations t) {
    switch (_controller.state) {
      case CallScreenState.requestingPermissions:
        return _CenteredStatus(
          icon: Icons.mic_none,
          message: t.callRequestingPermissions,
          showSpinner: true,
        );
      case CallScreenState.permissionDenied:
        return _CenteredStatus(
          icon: Icons.videocam_off_outlined,
          message: t.callPermissionDenied,
          actionLabel: t.callOpenSettings,
          onAction: openAppSettings,
        );
      case CallScreenState.authorizing:
        return Column(mainAxisSize: MainAxisSize.min, children: [
          const Spacer(),
          _CenteredStatus(
            icon: Icons.lock_outline,
            message: t.callPreparing,
            showSpinner: true,
          ),
          const Spacer(),
          if (_isRunningLate)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                decoration: BoxDecoration(
                  color: PatientTokens.warningLight,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(t.callDelayed,
                    style: const TextStyle(
                        fontSize: 11.5, color: PatientTokens.amberText)),
              ),
            ),
        ]);
      case CallScreenState.connecting:
        return _CenteredStatus(
          icon: Icons.sync,
          message: t.callConnecting,
          showSpinner: true,
        );
      case CallScreenState.connected:
      case CallScreenState.reconnecting:
        return _InCallView(
            controller: _controller, onLeave: _leaveAndPop, t: t);
      case CallScreenState.failed:
        return _CenteredStatus(
          icon: Icons.cloud_off,
          message: _controller.errorMessage ?? t.callConnectionLost,
          actionLabel: t.retryButton,
          onAction: _controller.retry,
        );
      case CallScreenState.left:
        return _CenteredStatus(icon: Icons.call_end, message: t.callEnded);
    }
  }
}

class _InCallView extends StatelessWidget {
  const _InCallView(
      {required this.controller, required this.onLeave, required this.t});
  final CallController controller;
  final Future<void> Function() onLeave;
  final AppLocalizations t;

  @override
  Widget build(BuildContext context) {
    final remoteUid = controller.remoteUid;
    return Stack(
      fit: StackFit.expand,
      children: [
        if (remoteUid != null)
          controller.engine.buildRemoteView(
            remoteUid: remoteUid,
            channelId: controller.channelId ?? '',
          )
        else
          Center(
            child: Text(t.callWaitingForOther,
                style: const TextStyle(color: Colors.white70)),
          ),
        Positioned(
          right: 16,
          top: 16,
          width: 110,
          height: 150,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(12),
            child: controller.videoEnabled
                ? controller.engine.buildLocalPreview()
                : Container(color: PatientTokens.surface),
          ),
        ),
        if (controller.state == CallScreenState.reconnecting)
          Positioned(
            top: 16,
            left: 16,
            right: 146,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: Colors.black54,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(t.callReconnecting,
                  style: const TextStyle(color: Colors.white)),
            ),
          ),
        Positioned(
          left: 0,
          right: 0,
          bottom: 24,
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              _CallControlButton(
                icon: controller.audioMuted ? Icons.mic_off : Icons.mic,
                onTap: controller.toggleMute,
              ),
              const SizedBox(width: 16),
              _CallControlButton(
                icon: controller.videoEnabled
                    ? Icons.videocam
                    : Icons.videocam_off,
                onTap: controller.toggleVideo,
              ),
              const SizedBox(width: 16),
              _CallControlButton(
                icon: Icons.cameraswitch_outlined,
                onTap: controller.switchCamera,
              ),
              const SizedBox(width: 16),
              _CallControlButton(
                icon: Icons.call_end,
                background: Colors.red,
                onTap: onLeave,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _CallControlButton extends StatelessWidget {
  const _CallControlButton({
    required this.icon,
    required this.onTap,
    this.background,
  });
  final IconData icon;
  final Future<void> Function() onTap;
  final Color? background;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      customBorder: const CircleBorder(),
      child: CircleAvatar(
        radius: 26,
        backgroundColor: background ?? Colors.white24,
        child: Icon(icon, color: Colors.white),
      ),
    );
  }
}

class _CenteredStatus extends StatelessWidget {
  const _CenteredStatus({
    required this.icon,
    required this.message,
    this.showSpinner = false,
    this.actionLabel,
    this.onAction,
  });
  final IconData icon;
  final String message;
  final bool showSpinner;
  final String? actionLabel;
  final Future<void> Function()? onAction;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          if (showSpinner)
            const CircularProgressIndicator()
          else
            Icon(icon, size: 52, color: PatientTokens.textSub),
          const SizedBox(height: 16),
          Text(message, textAlign: TextAlign.center),
          if (actionLabel != null && onAction != null) ...[
            const SizedBox(height: 16),
            TextButton(onPressed: onAction, child: Text(actionLabel!)),
          ],
        ]),
      ),
    );
  }
}
