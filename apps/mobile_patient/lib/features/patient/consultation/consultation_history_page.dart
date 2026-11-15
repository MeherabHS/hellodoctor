import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../../../core/l10n/kenya_time.dart';
import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import 'appointment_destination.dart';

/// The patient's consultations, both modalities: current (booked, waiting,
/// in progress) and past (completed, cancelled, expired). Each entry opens
/// through [openAppointment], so it always reflects the latest server state.
/// Only what the server returns is shown; no clinical summary is invented.
class ConsultationHistoryPage extends StatefulWidget {
  const ConsultationHistoryPage(
      {super.key,
      required this.repo,
      required this.familyProfilesCubit,
      this.myUserId});
  final BookingRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;
  final String? myUserId;

  @override
  State<ConsultationHistoryPage> createState() =>
      _ConsultationHistoryPageState();
}

class _ConsultationHistoryPageState extends State<ConsultationHistoryPage> {
  bool _past = false;
  bool _loading = true;
  String? _error;
  List<AppointmentSummaryDto> _items = const [];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final items = await widget.repo.myAppointments(
          _past ? 'past' : 'upcoming',
          profileId: widget.familyProfilesCubit.state.activeProfileId);
      if (!mounted) return;
      setState(() {
        _items = items;
        _loading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = e is ApiException
            ? e.userMessage
            : AppLocalizations.of(context).consultStatusLoadError;
      });
    }
  }

  Future<void> _open(AppointmentSummaryDto a) async {
    try {
      await openAppointment(context,
          appointmentId: a.id,
          repo: widget.repo,
          familyProfilesCubit: widget.familyProfilesCubit,
          myUserId: widget.myUserId);
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text(e is ApiException
              ? e.userMessage
              : AppLocalizations.of(context).consultStatusLoadError)));
    }
    if (mounted) _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          PatientPageHeader(l.historyTitle, l.historySubtitle,
              onRefresh: _load),
          Padding(
            padding: const EdgeInsets.fromLTRB(18, 4, 18, 8),
            child: Row(children: [
              Expanded(child: _tab(l.historyCurrent, !_past, false)),
              const SizedBox(width: 10),
              Expanded(child: _tab(l.historyPast, _past, true)),
            ]),
          ),
          Expanded(
            child: RefreshIndicator(
              onRefresh: _load,
              child: _loading
                  ? ListView(
                      padding: const EdgeInsets.all(18),
                      children: const [
                          SkeletonBlock(height: 84),
                          SizedBox(height: 10),
                          SkeletonBlock(height: 84),
                        ])
                  : _error != null
                      ? ListView(
                          padding: const EdgeInsets.all(18),
                          children: [InlineError(_error!, onRetry: _load)])
                      : _items.isEmpty
                          ? ListView(
                              padding: const EdgeInsets.all(24),
                              children: [
                                  Text(
                                      _past
                                          ? l.historyEmptyPast
                                          : l.historyEmptyCurrent,
                                      textAlign: TextAlign.center,
                                      style: const TextStyle(
                                          color: PatientTokens.textSub)),
                                ])
                          : ListView.separated(
                              padding: const EdgeInsets.fromLTRB(18, 4, 18, 24),
                              itemCount: _items.length,
                              separatorBuilder: (_, __) =>
                                  const SizedBox(height: 10),
                              itemBuilder: (context, i) =>
                                  _HistoryCard(item: _items[i], onTap: _open),
                            ),
            ),
          ),
        ]),
      ),
    );
  }

  Widget _tab(String label, bool selected, bool past) => SizedBox(
        height: 44,
        child: selected
            ? FilledButton(onPressed: () {}, child: Text(label))
            : OutlinedButton(
                onPressed: () {
                  setState(() => _past = past);
                  _load();
                },
                child: Text(label)),
      );
}

class _HistoryCard extends StatelessWidget {
  const _HistoryCard({required this.item, required this.onTap});
  final AppointmentSummaryDto item;
  final ValueChanged<AppointmentSummaryDto> onTap;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final video = item.modality == 'VIDEO';
    final locale = Localizations.localeOf(context).toLanguageTag();
    final outcome = historyOutcomeLabel(l, item.clinicalOutcome);
    return PatientCard(
      child: InkWell(
        onTap: () => onTap(item),
        child: Row(children: [
          Icon(video ? Icons.videocam_rounded : Icons.chat_bubble_rounded,
              color: PatientTokens.primaryDark),
          const SizedBox(width: 12),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(item.doctorDisplayName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontWeight: FontWeight.w800, color: PatientTokens.navy)),
              const SizedBox(height: 2),
              Text(
                  '${video ? l.consultVideo : l.consultChat} • '
                  '${DateFormat('EEE, d MMM y', locale).format(kenyaTime(item.startTime))} '
                  '${formatTimeEat(item.startTime)}',
                  style: const TextStyle(
                      fontSize: 12.5, color: PatientTokens.textSub)),
              const SizedBox(height: 6),
              Wrap(spacing: 8, runSpacing: 4, children: [
                StatusChip(historyStatusLabel(l, item.status)),
                if (outcome != null)
                  Text(outcome,
                      style: const TextStyle(
                          fontSize: 12, color: PatientTokens.ink)),
              ]),
            ]),
          ),
          const Icon(Icons.chevron_right_rounded),
        ]),
      ),
    );
  }
}

String historyStatusLabel(AppLocalizations l, String status) =>
    switch (status) {
      'PENDING_PAYMENT' => l.apptStatusPendingPayment,
      'CONFIRMED' => l.apptStatusConfirmed,
      'WAITING' => l.apptStatusWaiting,
      'IN_CONSULTATION' => l.apptStatusInConsultation,
      'COMPLETED' => l.apptStatusCompleted,
      'CANCELLED' => l.apptStatusCancelled,
      'EXPIRED' => l.apptStatusExpired,
      _ => l.historyStatusClosed,
    };

String? historyOutcomeLabel(AppLocalizations l, String? outcome) =>
    switch (outcome) {
      'COMPLETED_WITH_RX' => l.historyOutcomeWithRx,
      'COMPLETED_NO_RX' => l.historyOutcomeNoRx,
      'REFERRED' => l.historyOutcomeReferred,
      'ESCALATED' => l.historyOutcomeEscalated,
      _ => null,
    };
