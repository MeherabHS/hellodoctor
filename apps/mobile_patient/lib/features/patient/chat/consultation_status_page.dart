import '../../../core/network/screen_refresh.dart';
import 'package:dio/dio.dart' show CancelToken;
import 'package:url_launcher/url_launcher.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../booking/booking_models.dart';
import '../booking/booking_repository.dart';
import '../booking/pre_consult_intake_modal.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import '../booking/waiting_room_page.dart';
import 'chat_repository.dart';
import 'patient_chat_page.dart';

/// Payment and activation are always read from the server. Opening a chat
/// resolves the existing conversation; it never creates or charges a booking.
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

class _ConsultationStatusPageState extends State<ConsultationStatusPage> {
  AppointmentDetailDto? detail;
  String? error;
  bool loading = false, opening = false, paying = false;
  String? paymentError;
  late final ScreenRefresh _screenRefresh;
  final _openAcknowledgement = CancelToken();
  Timer? _holdTimer;
  Stopwatch? _holdStopwatch;
  int? _holdBaseSeconds;
  int? _remainingHoldSeconds;
  @override
  void initState() {
    super.initState();
    refresh();
    _holdTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      _updateHoldCountdown();
    });
    unawaited(_ackOpen());
    _screenRefresh = ScreenRefresh(refresh,
        interval: const Duration(seconds: 15),
        networkChanges: widget.repo.connectivity?.changes,
        hasError: () => error != null,
        enabled: () => !const [
              'COMPLETED',
              'CANCELLED',
              'REFUNDED',
              'EXPIRED',
              'PREMATURE_TERMINATION',
              'DISPUTED',
            ].contains(detail?.status ?? widget.appointment.status));
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
    _holdTimer?.cancel();
    _holdStopwatch?.stop();
    _openAcknowledgement.cancel();
    super.dispose();
  }

  Future<void> refresh() async {
    if (loading) return;
    loading = true;
    final requestClock = Stopwatch()..start();
    try {
      final value = await widget.repo.appointment(widget.appointment.id);
      if (mounted) {
        setState(() {
          detail = value;
          if (value.holdExpiresAt != null && value.serverTime != null) {
            _holdBaseSeconds = value.holdExpiresAt!
                .difference(value.serverTime!)
                .inSeconds
                .clamp(0, 600)
                .toInt();
            // Start from request initiation. This makes network latency reduce
            // the displayed hold, never extend a server deadline.
            _holdStopwatch = requestClock;
          } else {
            _holdBaseSeconds = null;
            _holdStopwatch = null;
          }
          error = null;
        });
        _updateHoldCountdown();
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          error = e is ApiException
              ? e.message
              : 'Unable to check payment. Please retry.';
        });
      }
    } finally {
      loading = false;
    }
  }

  void _updateHoldCountdown() {
    final expires = detail?.holdExpiresAt;
    final stopwatch = _holdStopwatch;
    final baseSeconds = _holdBaseSeconds;
    if (expires == null ||
        detail?.status != 'PENDING_PAYMENT' ||
        stopwatch == null ||
        baseSeconds == null) {
      if (_remainingHoldSeconds != null && mounted) {
        setState(() => _remainingHoldSeconds = null);
      }
      return;
    }
    final seconds =
        (baseSeconds - stopwatch.elapsed.inSeconds).clamp(0, 600).toInt();
    if (!mounted || seconds == _remainingHoldSeconds) return;
    setState(() => _remainingHoldSeconds = seconds);
    if (seconds == 0) unawaited(refresh());
  }

  String _formatHold(int seconds) {
    final minutes = (seconds ~/ 60).toString().padLeft(2, '0');
    final remaining = (seconds % 60).toString().padLeft(2, '0');
    return '$minutes:$remaining';
  }

  Future<void> pay() async {
    if (paying || detail?.status != 'PENDING_PAYMENT') return;
    setState(() {
      paying = true;
      paymentError = null;
    });
    try {
      final uri = await widget.repo.initiatePayment(widget.appointment.id);
      if (!mounted) return;
      if (!await launchUrl(uri, mode: LaunchMode.externalApplication)) {
        throw const ApiException(
            null, 'Unable to open payment. Please try again.');
      }
      await refresh();
    } catch (e) {
      if (mounted) {
        setState(() => paymentError = e is ApiException && e.status == 409
            ? e.message
            : 'Unable to start payment. Please try again.');
      }
    } finally {
      if (mounted) setState(() => paying = false);
    }
  }

  Future<void> open() async {
    if (detail?.status == 'EXPIRED') return;
    final history = const [
      'CANCELLED',
      'REFUNDED',
      'EXPIRED',
      'PREMATURE_TERMINATION',
      'DISPUTED'
    ].contains(detail?.status);
    if (opening || error != null || (detail?.isReady != true && !history)) {
      return;
    }
    setState(() => opening = true);
    try {
      if (widget.appointment.modality == 'CHAT' || history) {
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
      } else {
        await Navigator.push(
            context,
            MaterialPageRoute(
                builder: (_) => WaitingRoomPage(
                    appointment: widget.appointment,
                    repo: widget.repo,
                    familyProfilesCubit: widget.familyProfilesCubit,
                    myUserId: widget.myUserId)));
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          error = e is ApiException && e.status == 404
              ? 'Payment is confirmed. Your consultation is being prepared; please retry shortly.'
              : e is ApiException
                  ? e.message
                  : 'Unable to open your consultation. Please retry.';
        });
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
    final ready = detail?.isReady == true && error == null;
    final failed = detail?.paymentStatus == 'FAILED';
    final ended = const [
      'CANCELLED',
      'REFUNDED',
      'EXPIRED',
      'PREMATURE_TERMINATION',
      'DISPUTED'
    ].contains(detail?.status);
    final expired = detail?.status == 'EXPIRED';
    return Scaffold(
        appBar: AppBar(title: const Text('Your consultation')),
        body: SafeArea(
            child: ListView(padding: const EdgeInsets.all(20), children: [
          Text(a.doctorDisplayName,
              style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 8),
          Text(a.primarySpecialty),
          const SizedBox(height: 20),
          PatientCard(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(
                    a.modality == 'CHAT'
                        ? 'Chat consultation'
                        : 'Video consultation',
                    style: Theme.of(context).textTheme.titleMedium),
                if (!a.paymentRequired)
                  const Padding(
                      padding: EdgeInsets.only(top: 8),
                      child: Text('No payment required'))
                else if (a.fee != null)
                  Padding(
                      padding: const EdgeInsets.only(top: 8),
                      child: Text('KES ${a.fee}')),
                const SizedBox(height: 16),
                Text(
                    expired
                        ? 'Reservation expired'
                        : ended
                            ? 'Consultation ${detail!.status.toLowerCase()}'
                            : ready
                                ? 'Consultation confirmed'
                                : failed
                                    ? 'Payment failed'
                                    : detail == null
                                        ? 'Checking consultation'
                                        : 'Payment required',
                    style: const TextStyle(fontWeight: FontWeight.w600)),
                const SizedBox(height: 8),
                Text(expired
                    ? 'The unpaid reservation has expired. Make a new booking and upload your records again.'
                    : ended
                        ? 'Messaging and calling are unavailable for this booking.'
                        : ready
                            ? 'Your consultation is available.'
                            : failed
                                ? 'Payment was not completed. You can try again.'
                                : 'Complete payment to confirm your consultation.'),
              ])),
          const SizedBox(height: 16),
          if (error != null) InlineError(error!, onRetry: refresh),
          if (paymentError != null)
            Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Text(paymentError!,
                    style: const TextStyle(color: PatientTokens.danger))),
          if (expired)
            FilledButton(
                onPressed: () =>
                    Navigator.of(context).popUntil((route) => route.isFirst),
                child: const Text('Choose a new booking'))
          else if (ready || ended)
            FilledButton(
                onPressed: error == null && !opening ? open : null,
                child: Text(opening
                    ? 'Opening...'
                    : ended
                        ? 'View consultation history'
                        : a.modality == 'CHAT'
                            ? 'Open consultation'
                            : 'Open waiting room'))
          else if (detail?.status == 'PENDING_PAYMENT') ...[
            if ((_remainingHoldSeconds ?? 0) > 0)
              Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: Text(
                      'Reservation expires in ${_formatHold(_remainingHoldSeconds!)}',
                      style: const TextStyle(
                          fontWeight: FontWeight.w700,
                          color: PatientTokens.danger))),
            FilledButton(
                onPressed: paying ||
                        (detail?.holdExpiresAt != null &&
                            (_remainingHoldSeconds ?? 0) == 0)
                    ? null
                    : pay,
                child: Text(paying
                    ? 'Starting payment...'
                    : 'Proceed to payment method')),
          ],
          if (!ready && !ended)
            TextButton(onPressed: refresh, child: const Text('Refresh status')),
          if (!expired)
            OutlinedButton(
                onPressed: () => showModalBottomSheet<void>(
                    context: context,
                    isScrollControlled: true,
                    builder: (_) => PreConsultIntakeModal(
                        doctorName: a.doctorDisplayName,
                        appointmentId: a.id,
                        repo: widget.repo)),
                child: const Text('Attach health records')),
          const SizedBox(height: 16),
          Text('Reference: ${a.appointmentNumber}',
              style: const TextStyle(color: PatientTokens.textSub)),
        ])));
  }
}
