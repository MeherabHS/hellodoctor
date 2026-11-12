import '../../../core/network/screen_refresh.dart';

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:permission_handler/permission_handler.dart';

import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/l10n/kenya_time.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../chat/chat_repository.dart';
import '../chat/patient_chat_page.dart';
import '../consultation/post_consultation_page.dart';
import '../grievance/grievance_modal.dart';
import '../grievance/grievance_repository.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import '../video/agora_call_page.dart';
import 'booking_repository.dart';
import 'join_eligibility.dart';
import 'waiting_room_cubit.dart';

/// `prototype/index.html` view-10 — "Scheduled Consultation" / Waiting Room.
///
/// Deliberately does NOT reproduce the prototype's "Attached Prescriptions &
/// Records" gallery card: no backend endpoint lists the documents already
/// attached to a given appointment (only upload and single-document-by-id
/// download exist today) — rendering it from stale, session-local state
/// would be misleading once the patient has navigated away and back.
/// Skipped rather than faked, per CLAUDE.md's "No Phantom Completions" rule.
///
/// Also deliberately has no Cancel action of any kind — explicit product
/// decision: a booked consultation is never patient-cancellable. Status
/// polling lives in [WaitingRoomCubit]; this page owns only the
/// permission-check UI and navigation, which don't need a cubit.
class WaitingRoomPage extends StatefulWidget {
  const WaitingRoomPage(
      {super.key,
      required this.appointment,
      required this.repo,
      required this.familyProfilesCubit,
      this.myUserId});
  final AppointmentSummaryDto appointment;
  final BookingRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;

  /// The signed-in account's own `users.id` — threaded through to
  /// [PatientChatPage] so it can tell which side of the thread a bubble
  /// belongs to. `null` only degrades which side messages render on, never
  /// access to the conversation itself (that's RLS-enforced server-side).
  final String? myUserId;

  @override
  State<WaitingRoomPage> createState() => _WaitingRoomPageState();
}

class _WaitingRoomPageState extends State<WaitingRoomPage> {
  late final WaitingRoomCubit _cubit;
  late final ChatRepository _chatRepo;
  late final GrievanceRepository _grievanceRepo;
  late final ScreenRefresh _screenRefresh;
  bool _checkingPermissions = false;
  Map<Permission, PermissionStatus>? _permissionResult;
  bool _openingChat = false;
  Timer? _ticker;
  bool _wasLocked = false;

  @override
  void initState() {
    super.initState();
    _cubit = WaitingRoomCubit(
      widget.repo,
      appointmentId: widget.appointment.id,
      initialStatus: widget.appointment.status,
      initialJoin: widget.appointment.join,
      trackJoin: widget.appointment.modality == 'VIDEO',
    );
    // Drives the countdown and, the moment the window opens, re-validates with
    // the server instead of trusting the local clock alone.
    _ticker = Timer.periodic(const Duration(seconds: 1), (_) {
      final join = _cubit.state.join;
      if (!mounted || join == null) return;
      final locked = join.gate() == JoinGateState.locked;
      if (locked) {
        setState(() {});
      } else if (_wasLocked) {
        setState(() {});
        unawaited(_cubit.poll());
      }
      _wasLocked = locked;
    });
    _chatRepo =
        ChatRepository(widget.repo.api, connectivity: widget.repo.connectivity);
    _grievanceRepo = GrievanceRepository(widget.repo.api);
    // Matches the 15-second cross-portal refresh cadence already used
    // elsewhere in this product (`docs/CHECKPOINT.md`'s scheduling
    // verification entry) rather than inventing a new interval.
    _screenRefresh = ScreenRefresh(_cubit.poll,
        interval: const Duration(seconds: 15),
        networkChanges: widget.repo.connectivity?.changes,
        enabled: () => !_cubit.state.terminal);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _screenRefresh.attach(context);
  }

  @override
  void dispose() {
    _ticker?.cancel();
    _screenRefresh.dispose();
    _cubit.close();
    super.dispose();
  }

  Future<void> _testPermissions() async {
    setState(() => _checkingPermissions = true);
    try {
      final result = await [Permission.camera, Permission.microphone].request();
      if (!mounted) return;
      setState(() => _permissionResult = result);
    } finally {
      if (mounted) setState(() => _checkingPermissions = false);
    }
  }

  void _openPostConsultation(WaitingRoomState state) {
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => PostConsultationPage(
        appointment: widget.appointment,
        clinicalOutcome: state.clinicalOutcome,
        completedAt: state.completedAt == null
            ? null
            : DateTime.tryParse(state.completedAt!),
        repo: widget.repo,
        familyProfilesCubit: widget.familyProfilesCubit,
      ),
    ));
  }

  void _joinCall() {
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => AgoraCallPage(
        appointmentId: widget.appointment.id,
        scheduledStartTime: widget.appointment.startTime,
        repo: widget.repo,
      ),
    ));
  }

  /// Resolves the appointment's conversation (gap #3) and opens it. A 404
  /// here is real and expected before `CONFIRMED` — the conversation is only
  /// created once payment is confirmed (`process_payment_webhook`) — shown
  /// as an honest message, not a generic error.
  Future<void> _openChat() async {
    if (_openingChat) return;
    setState(() => _openingChat = true);
    try {
      final conversation =
          await _chatRepo.conversationForAppointment(widget.appointment.id);
      if (!mounted) return;
      await Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => PatientChatPage(
          conversation: conversation,
          doctorDisplayName: widget.appointment.doctorDisplayName,
          repo: _chatRepo,
          myUserId: widget.myUserId,
        ),
      ));
    } on ApiException catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text(e.status == 404
              ? AppLocalizations.of(context).chatOpensOnceConfirmed
              : e.userMessage)));
    } catch (_) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text(AppLocalizations.of(context).chatUnableToOpen)));
    } finally {
      if (mounted) setState(() => _openingChat = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final appointment = widget.appointment;
    final isVideo = appointment.modality == 'VIDEO';
    final l10n = AppLocalizations.of(context);
    return BlocProvider.value(
      value: _cubit,
      child: Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
          child: Column(children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(8, 8, 8, 0),
              child: Row(children: [
                IconButton(
                  onPressed: () => Navigator.of(context).maybePop(),
                  icon: const Icon(Icons.arrow_back, size: 20),
                ),
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text(
                      isVideo
                          ? l10n.waitingRoomVideoTitle
                          : l10n.waitingRoomChatTitle,
                      style: const TextStyle(
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text)),
                  Text(l10n.waitingRoomDoctorInitiated,
                      style: const TextStyle(
                          fontSize: 11, color: PatientTokens.textSub)),
                ]),
              ]),
            ),
            Expanded(
              child: BlocBuilder<WaitingRoomCubit, WaitingRoomState>(
                builder: (context, state) {
                  return ListView(
                    padding: const EdgeInsets.fromLTRB(18, 10, 18, 24),
                    children: [
                      PatientCard(
                        child: Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              const CircleAvatar(
                                radius: 25,
                                backgroundColor: PatientTokens.primaryLight,
                                child: Icon(Icons.medical_services,
                                    color: PatientTokens.primaryDark),
                              ),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                      Text(appointment.doctorDisplayName,
                                          style: const TextStyle(
                                              fontWeight: FontWeight.w800,
                                              color: PatientTokens.text)),
                                      Text(appointment.primarySpecialty,
                                          style: const TextStyle(
                                              fontSize: 11.5,
                                              color: PatientTokens.textSub)),
                                      const SizedBox(height: 6),
                                      Container(
                                        padding: const EdgeInsets.symmetric(
                                            horizontal: 8, vertical: 3),
                                        decoration: BoxDecoration(
                                            color: PatientTokens.primaryLight,
                                            borderRadius:
                                                BorderRadius.circular(6)),
                                        child: Text(
                                            l10n.waitingRoomSlotLabel(
                                                _formatSlot(l10n,
                                                    appointment.startTime)),
                                            style: const TextStyle(
                                                fontSize: 10.5,
                                                fontWeight: FontWeight.w700,
                                                color:
                                                    PatientTokens.primaryDark)),
                                      ),
                                    ]),
                              ),
                              StatusChip(_statusLabel(l10n, state.status)),
                            ]),
                      ),
                      const SizedBox(height: 12),
                      if (state.status == 'COMPLETED')
                        Column(children: [
                          Container(
                            padding: const EdgeInsets.all(14),
                            decoration: BoxDecoration(
                                color: const Color(0xFFF0FDF4),
                                borderRadius: BorderRadius.circular(
                                    PatientTokens.cardRadius),
                                border:
                                    Border.all(color: const Color(0xFFA7F3D0))),
                            child: Text(l10n.waitingRoomCompletedBanner,
                                style:
                                    const TextStyle(color: Color(0xFF065F46))),
                          ),
                          const SizedBox(height: 12),
                          SizedBox(
                            width: double.infinity,
                            child: FilledButton(
                              onPressed: () => _openPostConsultation(state),
                              style: FilledButton.styleFrom(
                                  backgroundColor: PatientTokens.primary,
                                  padding:
                                      const EdgeInsets.symmetric(vertical: 13)),
                              child: Text(l10n.waitingRoomViewSummary),
                            ),
                          ),
                        ])
                      else if (state.terminal)
                        Container(
                          padding: const EdgeInsets.all(14),
                          decoration: BoxDecoration(
                              color: PatientTokens.surfaceAlt,
                              borderRadius: BorderRadius.circular(
                                  PatientTokens.cardRadius)),
                          child: Text(
                              l10n.waitingRoomTerminalBanner(
                                  _statusLabel(l10n, state.status)
                                      .toLowerCase()),
                              style: const TextStyle(
                                  color: PatientTokens.textSub)),
                        )
                      else if (!const [
                        'CONFIRMED',
                        'WAITING',
                        'IN_CONSULTATION'
                      ].contains(state.status))
                        const Padding(
                            padding: EdgeInsets.all(14),
                            child: Text(
                                'Your consultation opens after payment is confirmed.'))
                      else ...[
                        Container(
                          padding: const EdgeInsets.all(12),
                          decoration: BoxDecoration(
                              color: const Color(0xFFF0FDF4),
                              borderRadius: BorderRadius.circular(
                                  PatientTokens.cardRadius),
                              border:
                                  Border.all(color: const Color(0xFFA7F3D0))),
                          child: Row(children: [
                            Container(
                              width: 10,
                              height: 10,
                              decoration: const BoxDecoration(
                                  color: Color(0xFF10B981),
                                  shape: BoxShape.circle),
                            ),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                        isVideo
                                            ? l10n.waitingRoomVideoCallsYou
                                            : l10n
                                                .waitingRoomChatMessageAnytime,
                                        style: const TextStyle(
                                            fontSize: 11.5,
                                            fontWeight: FontWeight.w800,
                                            color: Color(0xFF065F46))),
                                    const SizedBox(height: 2),
                                    Text(
                                        isVideo
                                            ? l10n
                                                .waitingRoomVideoCallsYouDetail
                                            : l10n.waitingRoomChatWindowDetail,
                                        style: const TextStyle(
                                            fontSize: 10,
                                            color: Color(0xFF047857))),
                                  ]),
                            ),
                          ]),
                        ),
                        const SizedBox(height: 14),
                        if (isVideo && state.join != null)
                          _JoinGateNotice(join: state.join!, l10n: l10n),
                        SizedBox(
                          width: double.infinity,
                          child: FilledButton.icon(
                            onPressed: isVideo
                                ? (joinIsOpen(state.join) ? _joinCall : null)
                                : _openChat,
                            style: FilledButton.styleFrom(
                                backgroundColor: PatientTokens.primary,
                                padding:
                                    const EdgeInsets.symmetric(vertical: 13)),
                            icon: _openingChat && !isVideo
                                ? const SizedBox(
                                    width: 14,
                                    height: 14,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2, color: Colors.white))
                                : Icon(isVideo
                                    ? Icons.videocam
                                    : Icons.chat_bubble_outline),
                            label: Text(isVideo
                                ? l10n.waitingRoomJoinVideoCall
                                : l10n.waitingRoomOpenChat),
                          ),
                        ),
                        if (isVideo) ...[
                          const SizedBox(height: 10),
                          // 24-hour chat is included with every CONFIRMED
                          // appointment regardless of modality (confirmed
                          // directly in `process_payment_webhook`) — the
                          // prototype shows this entry point in the Video
                          // waiting room too, not only the Chat one.
                          SizedBox(
                            width: double.infinity,
                            child: OutlinedButton.icon(
                              onPressed: _openingChat ? null : _openChat,
                              icon: _openingChat
                                  ? const SizedBox(
                                      width: 14,
                                      height: 14,
                                      child: CircularProgressIndicator(
                                          strokeWidth: 2))
                                  : const Icon(Icons.chat_bubble_outline,
                                      size: 16),
                              label: Text(l10n.waitingRoomOpen24hChat),
                            ),
                          ),
                        ],
                        const SizedBox(height: 10),
                        SizedBox(
                          width: double.infinity,
                          child: OutlinedButton.icon(
                            onPressed:
                                _checkingPermissions ? null : _testPermissions,
                            icon: _checkingPermissions
                                ? const SizedBox(
                                    width: 14,
                                    height: 14,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2))
                                : const Icon(Icons.videocam_outlined, size: 16),
                            label: Text(l10n.waitingRoomTestCameraMic),
                          ),
                        ),
                        if (_permissionResult != null) ...[
                          const SizedBox(height: 8),
                          _PermissionSummary(result: _permissionResult!),
                        ],
                        const SizedBox(height: 18),
                        Center(
                          child: TextButton.icon(
                            onPressed: () => showGrievanceModal(
                              context,
                              appointmentId: widget.appointment.id,
                              doctorName: widget.appointment.doctorDisplayName,
                              repo: _grievanceRepo,
                            ),
                            icon: const Icon(Icons.report_problem_outlined,
                                size: 14, color: PatientTokens.danger),
                            label: Text(l10n.waitingRoomReportIssue,
                                style: const TextStyle(
                                    fontSize: 11, color: PatientTokens.danger)),
                          ),
                        ),
                      ],
                    ],
                  );
                },
              ),
            ),
          ]),
        ),
      ),
    );
  }

  String _statusLabel(AppLocalizations l10n, String status) => switch (status) {
        'PENDING_PAYMENT' => l10n.apptStatusPendingPayment,
        'CONFIRMED' => l10n.apptStatusConfirmed,
        'WAITING' => l10n.apptStatusWaiting,
        'IN_CONSULTATION' => l10n.apptStatusInConsultation,
        'COMPLETED' => l10n.apptStatusCompleted,
        'CANCELLED' => l10n.apptStatusCancelled,
        'EXPIRED' => l10n.apptStatusExpired,
        _ => status,
      };

  String _formatSlot(AppLocalizations l10n, DateTime start) {
    // Both sides of the "today" comparison are Nairobi wall-clock, never the
    // device's own zone.
    final eat = kenyaTime(start);
    final today = kenyaTime(DateTime.now());
    final isToday = eat.year == today.year &&
        eat.month == today.month &&
        eat.day == today.day;
    final time = formatTimeEat(start);
    return '${isToday ? l10n.todayLabel : '${eat.day}/${eat.month}'}, $time';
  }
}

class _PermissionSummary extends StatelessWidget {
  const _PermissionSummary({required this.result});
  final Map<Permission, PermissionStatus> result;

  @override
  Widget build(BuildContext context) {
    final allGranted = result.values.every((s) => s.isGranted);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color:
            allGranted ? const Color(0xFFF0FDF4) : PatientTokens.warningLight,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(children: [
        Icon(allGranted ? Icons.check_circle : Icons.warning_amber_rounded,
            size: 18,
            color:
                allGranted ? const Color(0xFF059669) : PatientTokens.amberText),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            allGranted
                ? AppLocalizations.of(context).waitingRoomPermissionsGranted
                : AppLocalizations.of(context).waitingRoomPermissionsDenied,
            style: TextStyle(
                fontSize: 11.5,
                color: allGranted
                    ? const Color(0xFF065F46)
                    : PatientTokens.amberText),
          ),
        ),
      ]),
    );
  }
}

/// Why Join is (not) available, straight from the server-derived eligibility:
/// "Your consultation room opens at [time] EAT" with a countdown, instead of a
/// generic connection error after tapping a button that could never work.
class _JoinGateNotice extends StatelessWidget {
  const _JoinGateNotice({required this.join, required this.l10n});
  final JoinEligibility join;
  final AppLocalizations l10n;

  @override
  Widget build(BuildContext context) {
    final gate = join.gate();
    if (gate == JoinGateState.open) return const SizedBox.shrink();
    final String text;
    switch (gate) {
      case JoinGateState.locked:
        text =
            '${l10n.waitingRoomOpensAt(formatEatDateTime(join.joinOpenAt))} · ${l10n.waitingRoomOpensIn(formatCountdown(join.untilOpen()))}';
      case JoinGateState.closed:
        text = l10n.waitingRoomWindowClosed;
      default:
        text = join.reason == 'NEEDS_REVIEW'
            ? l10n.waitingRoomNeedsReview
            : l10n.waitingRoomJoinUnavailable;
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Container(
        key: const Key('join-gate-notice'),
        width: double.infinity,
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
            color: PatientTokens.surface,
            borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
            border: Border.all(color: PatientTokens.border)),
        child: Row(children: [
          const Icon(Icons.lock_clock, size: 18, color: PatientTokens.textSub),
          const SizedBox(width: 8),
          Expanded(
              child: Text(text,
                  style: const TextStyle(
                      fontSize: 12, fontWeight: FontWeight.w700))),
        ]),
      ),
    );
  }
}
