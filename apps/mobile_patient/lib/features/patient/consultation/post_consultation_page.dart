// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../grievance/grievance_modal.dart';
import '../grievance/grievance_repository.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import '../vault/health_vault_page.dart';
import '../vault/vault_repository.dart';

/// `prototype/index.html` view-11 — "Consultation Completed" / Post
/// Consultation. Reached from the Waiting Room once polling detects the
/// appointment has moved to `COMPLETED` (`WaitingRoomCubit`).
///
/// Deliberately drops the prototype's `#v11TelemetryCard` ("Session
/// Telemetry Record": a fixed "2m 14s" duration, "100% Quality", a fake
/// escrow amount, a fake `#CONS-9481` ref) — that panel is `style="display:
/// none"` in the prototype's own markup and has no backing endpoint a
/// patient can call; real telemetry (wired earlier this session) reaches
/// only Admin's own adjudication view, which this app has no patient-facing
/// read for. Also honestly branches on the real clinical outcome
/// (CLAUDE.md §6: completion never requires a prescription — outcomes are
/// `COMPLETED_WITH_RX` / `COMPLETED_NO_RX` / `REFERRED` / `ESCALATED`) rather
/// than always assuming an e-Prescription was issued, as the prototype's
/// static demo content does.
class PostConsultationPage extends StatelessWidget {
  const PostConsultationPage({
    super.key,
    required this.appointment,
    required this.clinicalOutcome,
    required this.completedAt,
    required this.repo,
    required this.familyProfilesCubit,
  });

  final AppointmentSummaryDto appointment;
  final String? clinicalOutcome;
  final DateTime? completedAt;
  final BookingRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;

  void _openVault(BuildContext context) {
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => HealthVaultPage(
        repo: VaultRepository(repo.api, connectivity: repo.connectivity),
        familyProfilesCubit: familyProfilesCubit,
      ),
    ));
  }

  void _returnHome(BuildContext context) {
    Navigator.of(context).popUntil((route) => route.isFirst);
  }

  String _outcomeMessage(AppLocalizations l10n) => switch (clinicalOutcome) {
        'COMPLETED_WITH_RX' => l10n.postConsultOutcomeWithRx,
        'COMPLETED_NO_RX' => l10n.postConsultOutcomeNoRx,
        'REFERRED' => l10n.postConsultOutcomeReferred,
        'ESCALATED' => l10n.postConsultOutcomeEscalated,
        _ => l10n.postConsultOutcomeDefault,
      };

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final grievanceRepo = GrievanceRepository(repo.api);
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          PatientPageHeader(l10n.postConsultHeader,
              l10n.postConsultSubtitle(appointment.doctorDisplayName)),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(18),
              children: [
                const SizedBox(height: 8),
                const Icon(Icons.check_circle,
                    color: PatientTokens.primary, size: 56),
                const SizedBox(height: 14),
                Text(l10n.postConsultHeader,
                    textAlign: TextAlign.center,
                    style: TextStyle(
                        fontSize: 17,
                        fontWeight: FontWeight.w800,
                        color: PatientTokens.text)),
                const SizedBox(height: 6),
                Text(_outcomeMessage(l10n),
                    textAlign: TextAlign.center,
                    style: const TextStyle(
                        fontSize: 12,
                        color: PatientTokens.textSub,
                        height: 1.45)),
                const SizedBox(height: 22),
                if (clinicalOutcome == 'COMPLETED_WITH_RX')
                  _RxCard(
                    appointment: appointment,
                    completedAt: completedAt,
                    onTap: () => _openVault(context),
                  ),
                const SizedBox(height: 26),
                SizedBox(
                  width: double.infinity,
                  child: FilledButton(
                    onPressed: () => _returnHome(context),
                    style: FilledButton.styleFrom(
                        backgroundColor: PatientTokens.primary,
                        padding: const EdgeInsets.symmetric(vertical: 13)),
                    child: Text(l10n.postConsultReturnHome),
                  ),
                ),
                const SizedBox(height: 10),
                SizedBox(
                  width: double.infinity,
                  child: OutlinedButton.icon(
                    onPressed: () => showGrievanceModal(
                      context,
                      appointmentId: appointment.id,
                      doctorName: appointment.doctorDisplayName,
                      repo: grievanceRepo,
                    ),
                    icon: const Icon(Icons.report_problem_outlined,
                        size: 16, color: PatientTokens.danger),
                    label: Text(l10n.postConsultReportIssue,
                        style: TextStyle(
                            color: PatientTokens.danger,
                            fontWeight: FontWeight.w700)),
                    style: OutlinedButton.styleFrom(
                        side: const BorderSide(color: Color(0xFFFECACA)),
                        padding: const EdgeInsets.symmetric(vertical: 11)),
                  ),
                ),
              ],
            ),
          ),
        ]),
      ),
    );
  }
}

class _RxCard extends StatelessWidget {
  const _RxCard(
      {required this.appointment,
      required this.completedAt,
      required this.onTap});
  final AppointmentSummaryDto appointment;
  final DateTime? completedAt;
  final VoidCallback onTap;

  String _formatted(DateTime? dt) {
    if (dt == null) return '';
    final local = dt.toLocal();
    return '${local.day}/${local.month}, ${formatTime12h(local)}';
  }

  @override
  Widget build(BuildContext context) {
    final when = _formatted(completedAt);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
      child: PatientCard(
        child: Row(children: [
          const Icon(Icons.description_outlined,
              color: PatientTokens.primaryDark, size: 26),
          const SizedBox(width: 12),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(AppLocalizations.of(context).postConsultRxIssued,
                  style: TextStyle(
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.text,
                      fontSize: 12.5)),
              const SizedBox(height: 2),
              Text(
                  when.isEmpty
                      ? appointment.doctorDisplayName
                      : AppLocalizations.of(context).postConsultDoctorAndDate(
                          appointment.doctorDisplayName, when),
                  style: const TextStyle(
                      fontSize: 11, color: PatientTokens.textSub)),
            ]),
          ),
          const Icon(Icons.chevron_right, color: PatientTokens.textSub),
        ]),
      ),
    );
  }
}
