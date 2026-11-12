import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:permission_handler/permission_handler.dart';

import '../../core/network/api_client.dart';
import '../../core/theme/tokens.dart';
import '../../core/video/agora_engine.dart';
import '../../core/video/call_controller.dart';
import '../../core/video/join_eligibility.dart';
import '../../core/video/mint_failure.dart';
import 'doctor_cubit.dart';
import 'doctor_repository.dart';

/// Real Agora RTC consultation call screen. Reuses the existing server
/// contract (`POST /consultations/:id/rtc-token`) exclusively — this screen
/// never mints a token itself and never sees the Agora App Certificate
/// (CLAUDE.md §1.10: Agora RTC SDK only, no custom signaling, certificate
/// stays server-side). Joining, leaving, muting, camera switching and
/// connection-state handling are all delegated to [CallController], which is
/// the directly unit-tested piece (`test/core/video/call_controller_test.dart`);
/// this widget is responsible only for rendering whatever state it reports.
class AgoraPage extends StatefulWidget {
  const AgoraPage({super.key, required this.appointmentId});
  final String appointmentId;
  @override
  State<AgoraPage> createState() => _AgoraPageState();
}

class _AgoraPageState extends State<AgoraPage> with WidgetsBindingObserver {
  late final CallController _controller;

  /// Server-derived join eligibility. The call is only STARTED (permissions,
  /// token request, Agora engine) once the server says the window is open; until
  /// then this screen shows when the room opens, with a countdown. The token
  /// mint remains the authoritative check at the real join boundary.
  JoinEligibility? _join;
  Timer? _ticker;
  bool _started = false;
  bool _checking = true;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _controller = CallController(
      engine: RealAgoraEngine(),
      mintToken: _mintToken,
      requestPermissions: _requestPermissions,
      telemetrySubmit: (events) => context
          .read<DoctorCubit>()
          .repo
          .telemetry(widget.appointmentId, events),
    )..addListener(_onControllerChanged);
    unawaited(_bootstrap());
  }

  DoctorRepository get _repo => context.read<DoctorCubit>().repo;

  Future<void> _bootstrap() async {
    JoinEligibility? join;
    try {
      join = await _repo.joinEligibility(widget.appointmentId);
    } catch (_) {
      // Eligibility unknown (offline / older backend): do not invent a lock;
      // the token request below is still authoritative and its refusal is
      // mapped to an actionable message.
    }
    if (!mounted) return;
    setState(() {
      _join = join;
      _checking = false;
    });
    if (joinIsOpen(join)) {
      _startCall();
      return;
    }
    _ticker = Timer.periodic(const Duration(seconds: 1), (_) async {
      if (!mounted || _started) return;
      final current = _join;
      if (current != null && current.isOpen()) {
        // Local clock says open: revalidate with the server before starting.
        _ticker?.cancel();
        JoinEligibility? fresh;
        try {
          fresh = await _repo.joinEligibility(widget.appointmentId);
        } catch (_) {}
        if (!mounted) return;
        if (joinIsOpen(fresh)) {
          _startCall();
        } else {
          setState(() => _join = fresh);
          _ticker = Timer.periodic(const Duration(seconds: 5), (_) async {
            try {
              final again = await _repo.joinEligibility(widget.appointmentId);
              if (!mounted || _started) return;
              if (again.isOpen()) {
                _ticker?.cancel();
                _startCall();
              } else {
                setState(() => _join = again);
              }
            } catch (_) {}
          });
        }
      } else {
        setState(() {});
      }
    });
  }

  void _startCall() {
    if (_started || !mounted) return;
    _ticker?.cancel();
    setState(() => _started = true);
    _controller.start();
  }

  /// Mints the RTC token and translates a backend refusal into a typed,
  /// actionable failure using the backend's stable error `code` (not its text).
  Future<RtcTokenData> _mintToken() async {
    try {
      final data = await _repo.rtcToken(widget.appointmentId);
      return RtcTokenData.fromJson(Map<String, dynamic>.from(data));
    } on ApiException catch (e) {
      DateTime? opensAt;
      if (e.code == 'NOT_YET_OPEN') {
        try {
          opensAt =
              (await _repo.joinEligibility(widget.appointmentId)).joinOpenAt;
        } catch (_) {}
      }
      throw mapMintFailure(e, opensAt: opensAt);
    }
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
    if (!_started) return;
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
    _ticker?.cancel();
    _controller.removeListener(_onControllerChanged);
    if (_started) _controller.leave();
    _controller.dispose();
    super.dispose();
  }

  Future<void> _leaveAndPop() async {
    if (_started) await _controller.leave();
    if (mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;
        await _leaveAndPop();
      },
      child: Scaffold(
        backgroundColor: Colors.black,
        appBar: _controller.state == CallScreenState.connected ||
                _controller.state == CallScreenState.reconnecting
            ? null
            : AppBar(title: const Text('Consultation call')),
        body: _buildBody(context),
      ),
    );
  }

  Widget _preJoinBody() {
    final join = _join;
    if (_checking) {
      return const _CenteredStatus(
        icon: Icons.lock_clock,
        message: 'Checking the consultation room…',
        showSpinner: true,
      );
    }
    if (join == null) {
      return const _CenteredStatus(
          icon: Icons.lock_clock,
          message: 'Preparing your call…',
          showSpinner: true);
    }
    final gate = join.gate();
    final String message;
    if (gate == JoinGateState.locked) {
      message =
          '${joinDenialText('NOT_YET_OPEN', opensAt: join.joinOpenAt)}\n\nOpens in ${formatCountdown(join.untilOpen())}';
    } else if (gate == JoinGateState.closed) {
      message = joinDenialText('WINDOW_CLOSED');
    } else {
      message = joinDenialText(join.reason);
    }
    return _CenteredStatus(
      icon: Icons.lock_clock,
      message: message,
      actionLabel: gate == JoinGateState.locked ? null : 'Back',
      onAction: gate == JoinGateState.locked
          ? null
          : () async {
              if (mounted) Navigator.of(context).pop();
            },
    );
  }

  Widget _buildBody(BuildContext context) {
    if (!_started) return _preJoinBody();
    switch (_controller.state) {
      case CallScreenState.requestingPermissions:
        return const _CenteredStatus(
          icon: Icons.mic_none,
          message: 'Requesting camera and microphone access…',
          showSpinner: true,
        );
      case CallScreenState.permissionDenied:
        return const _CenteredStatus(
          icon: Icons.videocam_off_outlined,
          message:
              'Camera and microphone access is required to join this call.',
          actionLabel: 'Open Settings',
          onAction: openAppSettings,
        );
      case CallScreenState.authorizing:
        return const _CenteredStatus(
          icon: Icons.lock_outline,
          message: 'Preparing your call…',
          showSpinner: true,
        );
      case CallScreenState.connecting:
        return const _CenteredStatus(
          icon: Icons.sync,
          message: 'Connecting…',
          showSpinner: true,
        );
      case CallScreenState.connected:
      case CallScreenState.reconnecting:
        return _InCallView(controller: _controller, onLeave: _leaveAndPop);
      case CallScreenState.failed:
        return _CenteredStatus(
          icon: Icons.cloud_off,
          message:
              '${_controller.errorMessage ?? 'The consultation call could not be connected.'}${_controller.diagnostic == null ? '' : '\n\n[${_controller.diagnostic}]'}',
          actionLabel: _controller.retryable ? 'Retry' : null,
          onAction: _controller.retryable ? _controller.retry : null,
        );
      case CallScreenState.left:
        return const _CenteredStatus(
          icon: Icons.call_end,
          message: 'Call ended.',
        );
    }
  }
}

class _InCallView extends StatelessWidget {
  const _InCallView({required this.controller, required this.onLeave});
  final CallController controller;
  final Future<void> Function() onLeave;

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
          const Center(
            child: Text('Waiting for the other participant to join…',
                style: TextStyle(color: Colors.white70)),
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
                : Container(color: DoctorTokens.surfaceSubtle),
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
              child: const Text('Reconnecting…',
                  style: TextStyle(color: Colors.white)),
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
            Icon(icon, size: 52, color: Colors.white70),
          const SizedBox(height: 16),
          Text(message,
              textAlign: TextAlign.center,
              style: const TextStyle(color: Colors.white)),
          if (actionLabel != null && onAction != null) ...[
            const SizedBox(height: 16),
            TextButton(onPressed: onAction, child: Text(actionLabel!)),
          ],
        ]),
      ),
    );
  }
}
