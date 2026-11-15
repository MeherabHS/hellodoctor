import '../../../core/network/screen_refresh.dart';
import 'package:dio/dio.dart' show CancelToken;
import 'dart:async';
import 'package:flutter/material.dart';
import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../consultation/consultation_scene.dart';
import '../consultation/post_consultation_page.dart';
import '../directory/directory_repository.dart';
import '../booking/booking_models.dart';
import '../booking/booking_repository.dart';
import '../booking/pre_consult_intake_modal.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import '../booking/waiting_room_page.dart';
import 'chat_repository.dart';
import 'patient_chat_page.dart';

/// "Consultation Confirmed" / consultation status. Everything shown is read
/// from the server's appointment state; the app is payment-free, so no price,
/// payment status or checkout appears here. Confirmed consultations continue
/// into the modality-aware waiting room (video or chat), completed ones into
/// the outcome screen, and ended ones show their real state.
class ConsultationStatusPage extends StatefulWidget {
  const ConsultationStatusPage(
      {super.key,
      required this.appointment,
      required this.repo,
      required this.familyProfilesCubit,
      this.myUserId});
  final AppointmentSummaryDto appointment;
  final BookingRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;
  final String? myUserId;
  @override
  State<ConsultationStatusPage> createState() => _ConsultationStatusPageState();
}

const _activeStatuses = ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'];
const _endedStatuses = [
  'CANCELLED',
  'REFUNDED',
  'EXPIRED',
  'PREMATURE_TERMINATION',
  'DISPUTED',
];

class _ConsultationStatusPageState extends State<ConsultationStatusPage> {
  AppointmentDetailDto? detail;
  String? error;
  bool loading = false, opening = false;
  late final ScreenRefresh _screenRefresh;
  final _openAcknowledgement = CancelToken();

  String get _status => detail?.status ?? widget.appointment.status;

  @override
  void initState() {
    super.initState();
    refresh();
    unawaited(_ackOpen());
    _screenRefresh = ScreenRefresh(refresh,
        interval: const Duration(seconds: 15),
        networkChanges: widget.repo.connectivity?.changes,
        hasError: () => error != null,
        enabled: () =>
            _status != 'COMPLETED' && !_endedStatuses.contains(_status));
  }

  Future<void> _ackOpen() async {
    try {
      final token = await widget.repo.api.read('access_token');
      if (token == null || !mounted) return;
      await ChatRepository(widget.repo.api,
              connectivity: widget.repo.connectivity)
          .acknowledge(widget.appointment.id, null, 'CONSULTATION_OPENED',
              cancelToken: _openAcknowledgement);
    } catch (_) {
      // An observation failure never implies that the consultation opened server-side.
    }
  }

  @override
  void dispose() {
    _screenRefresh.dispose();
    _openAcknowledgement.cancel();
    super.dispose();
  }

  Future<void> refresh() async {
    if (loading) return;
    loading = true;
    try {
      final value = await widget.repo.appointment(widget.appointment.id);
      if (mounted) {
        setState(() {
          detail = value;
          error = null;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          error = e is ApiException
              ? e.userMessage
              : AppLocalizations.of(context).consultStatusLoadError;
        });
      }
    } finally {
      loading = false;
    }
  }

  Future<void> _openWaitingRoom() => Navigator.push(
      context,
      MaterialPageRoute(
          builder: (_) => WaitingRoomPage(
              appointment: widget.appointment,
              repo: widget.repo,
              familyProfilesCubit: widget.familyProfilesCubit,
              myUserId: widget.myUserId)));

  Future<void> _openOutcome() => Navigator.push(
      context,
      MaterialPageRoute(
          builder: (_) => PostConsultationPage(
              appointment: widget.appointment,
              clinicalOutcome: detail?.clinicalOutcome,
              completedAt: DateTime.tryParse(detail?.completedAt ?? ''),
              repo: widget.repo,
              familyProfilesCubit: widget.familyProfilesCubit)));

  /// Read-only history of an ended consultation's messages, if it had any.
  Future<void> _openMessages() async {
    if (opening) return;
    setState(() => opening = true);
    try {
      final repo = ChatRepository(widget.repo.api,
          connectivity: widget.repo.connectivity);
      final conversation =
          await repo.conversationForAppointment(widget.appointment.id);
      if (!mounted) return;
      await Navigator.push(
          context,
          MaterialPageRoute(
              builder: (_) => PatientChatPage(
                  conversation: conversation,
                  doctorDisplayName: widget.appointment.doctorDisplayName,
                  repo: repo,
                  myUserId: widget.myUserId)));
    } catch (e) {
      if (mounted) {
        setState(() => error = e is ApiException && e.status == 404
            ? AppLocalizations.of(context).consultStatusNoMessages
            : e is ApiException
                ? e.userMessage
                : AppLocalizations.of(context).consultStatusLoadError);
      }
    } finally {
      if (mounted) setState(() => opening = false);
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _screenRefresh.attach(context);
  }

  @override
  Widget build(BuildContext context) {
    final a = widget.appointment;
    final l = AppLocalizations.of(context);
    final video = a.modality == 'VIDEO';
    final status = _status;
    final known = detail != null;
    final active = known && _activeStatuses.contains(status);
    final completed = known && status == 'COMPLETED';
    final ended = known && _endedStatuses.contains(status);
    final notActive = known && !active && !completed && !ended;

    final (String title, String body) = !known
        ? (l.consultStatusCheckingTitle, l.consultStatusCheckingBody)
        : completed
            ? (l.consultStatusCompletedTitle, l.consultStatusCompletedBody)
            : status == 'IN_CONSULTATION'
                ? (
                    l.consultStatusInProgressTitle,
                    video
                        ? l.consultStatusActiveVideoBody
                        : l.consultStatusActiveChatBody
                  )
                : active
                    ? (
                        l.consultStatusConfirmedTitle,
                        video
                            ? l.consultStatusActiveVideoBody
                            : l.consultStatusActiveChatBody
                      )
                    : ended
                        ? (l.consultStatusEndedTitle, l.consultStatusEndedBody)
                        : (
                            l.consultStatusNotActiveTitle,
                            l.consultStatusNotActiveBody
                          );

    return Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
            child: ListView(padding: EdgeInsets.zero, children: [
          // Product owner's "Consultation Confirmed" design. The celebratory
          // heading appears only while the server reports the booking active.
          ConsultationHeader(
              art: 'consult_confirmed_header',
              showCheck: active,
              title: active ? l.consultConfirmedTitle : l.consultYourTitle,
              subtitle: active
                  ? (video ? l.consultConfirmedVideo : l.consultConfirmedChat)
                  : null),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 14, 16, 24),
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  ConsultationDoctorCard(
                      appointment: a,
                      photos: DirectoryRepository(widget.repo.api)),
                  const SizedBox(height: 14),
                  PatientCard(
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                        Text(title,
                            style: const TextStyle(
                                fontWeight: FontWeight.w700,
                                color: PatientTokens.navy)),
                        const SizedBox(height: 6),
                        Text(body),
                      ])),
                  if (active) ...[
                    const SizedBox(height: 12),
                    ConsultationTipsCard(
                        title: l.consultBeforeTitle,
                        art: 'consult_tips',
                        tips: [
                          l.consultBefore1,
                          l.consultBefore2,
                          l.consultBefore3
                        ]),
                  ],
                  const SizedBox(height: 16),
                  if (error != null) InlineError(error!, onRetry: refresh),
                  if (active)
                    FilledButton(
                        onPressed: error == null ? _openWaitingRoom : null,
                        child: Text(video
                            ? l.consultOpenVideoWaitingRoom
                            : l.consultOpenChatWaitingRoom))
                  else if (completed)
                    FilledButton(
                        onPressed: _openOutcome,
                        child: Text(l.consultViewOutcome))
                  else if (ended)
                    OutlinedButton(
                        onPressed: opening ? null : _openMessages,
                        child: Text(l.consultViewMessages))
                  else if (notActive)
                    FilledButton(
                        onPressed: () => Navigator.of(context)
                            .popUntil((route) => route.isFirst),
                        child: Text(l.consultBookNew)),
                  if (!known)
                    TextButton(
                        onPressed: refresh,
                        child: Text(l.consultRefreshStatus)),
                  if (active) ...[
                    const SizedBox(height: 8),
                    OutlinedButton(
                        onPressed: () => showModalBottomSheet<void>(
                            context: context,
                            isScrollControlled: true,
                            builder: (_) => PreConsultIntakeModal(
                                doctorName: a.doctorDisplayName,
                                appointmentId: a.id,
                                repo: widget.repo)),
                        child: Text(l.consultAttachRecords)),
                  ],
                  const SizedBox(height: 16),
                  Text(l.consultReference(a.appointmentNumber),
                      style: const TextStyle(color: PatientTokens.textSub)),
                ]),
          ),
        ])));
  }
}
