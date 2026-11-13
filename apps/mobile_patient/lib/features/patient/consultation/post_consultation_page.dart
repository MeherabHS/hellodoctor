// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/l10n/kenya_time.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../directory/directory_repository.dart';
import '../directory/patient_directory_page.dart';
import 'consultation_scene.dart';
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

  void _bookAnother(BuildContext context) {
    final connectivity = repo.connectivity;
    if (connectivity == null) return _returnHome(context);
    Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => PatientDirectoryPage(
            api: repo.api,
            cache: null,
            connectivity: connectivity,
            familyProfilesCubit: familyProfilesCubit)));
  }

  /// Product owner's "Consultation Complete" design.
  ///
  /// The design's "Consultation Summary" and "Follow-up Plan" cards hold
  /// clinical text written by a doctor. The app receives only the recorded
  /// outcome (and a prescription when one was issued), so this screen shows
  /// that outcome and never invents a summary or a plan.
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final grievanceRepo = GrievanceRepository(repo.api);
    final video = appointment.modality == 'VIDEO';
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: ListView(padding: EdgeInsets.zero, children: [
          ConsultationHeader(
              art: 'consult_complete_header',
              title: video ? l10n.completeTitleVideo : l10n.completeTitleChat,
              subtitle: l10n.completeBody),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 14, 16, 24),
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  ConsultationDoctorCard(
                      appointment: appointment,
                      photos: DirectoryRepository(repo.api),
                      trailing: Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 10, vertical: 6),
                        decoration: BoxDecoration(
                            color: PatientTokens.primaryDark,
                            borderRadius: BorderRadius.circular(20)),
                        child: Row(mainAxisSize: MainAxisSize.min, children: [
                          const Icon(Icons.check_rounded,
                              size: 16, color: Colors.white),
                          const SizedBox(width: 4),
                          Text(l10n.completeStatus,
                              style: const TextStyle(
                                  fontSize: 12,
                                  fontWeight: FontWeight.w700,
                                  color: Colors.white)),
                        ]),
                      )),
                  const SizedBox(height: 14),
                  Container(
                    padding: const EdgeInsets.all(14),
                    decoration: BoxDecoration(
                        color: PatientTokens.surface,
                        borderRadius: BorderRadius.circular(16),
                        border: Border.all(color: PatientTokens.border)),
                    child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Container(
                              width: 40,
                              height: 40,
                              decoration: BoxDecoration(
                                  color: PatientTokens.primaryLight,
                                  borderRadius: BorderRadius.circular(12)),
                              child: const Icon(Icons.description_outlined,
                                  color: PatientTokens.primaryDark)),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(l10n.completeOutcomeTitle,
                                      style: const TextStyle(
                                          fontSize: 15,
                                          fontWeight: FontWeight.w800,
                                          color: PatientTokens.navy)),
                                  const SizedBox(height: 4),
                                  Text(_outcomeMessage(l10n),
                                      style: const TextStyle(
                                          fontSize: 13.5,
                                          height: 1.45,
                                          color: PatientTokens.ink)),
                                ]),
                          ),
                        ]),
                  ),
                  if (clinicalOutcome == 'COMPLETED_WITH_RX') ...[
                    const SizedBox(height: 12),
                    _RxCard(
                      appointment: appointment,
                      completedAt: completedAt,
                      onTap: () => _openVault(context),
                    ),
                  ],
                  const SizedBox(height: 18),
                  FilledButton.icon(
                      onPressed: () => _openVault(context),
                      icon: const Icon(Icons.description_outlined),
                      label: Text(l10n.completeViewRecords),
                      style: FilledButton.styleFrom(
                          backgroundColor: PatientTokens.primaryDark,
                          minimumSize: const Size.fromHeight(52),
                          shape: const StadiumBorder())),
                  const SizedBox(height: 10),
                  OutlinedButton.icon(
                      onPressed: () => _bookAnother(context),
                      icon: const Icon(Icons.event_repeat_outlined),
                      label: Text(l10n.completeBookAnother),
                      style: OutlinedButton.styleFrom(
                          foregroundColor: PatientTokens.primaryDark,
                          side: const BorderSide(
                              color: PatientTokens.primaryDark, width: 1.5),
                          minimumSize: const Size.fromHeight(52),
                          shape: const StadiumBorder())),
                  const SizedBox(height: 6),
                  TextButton(
                      onPressed: () => _returnHome(context),
                      child: Text(l10n.postConsultReturnHome)),
                  TextButton.icon(
                    onPressed: () => showGrievanceModal(
                      context,
                      appointmentId: appointment.id,
                      doctorName: appointment.doctorDisplayName,
                      repo: grievanceRepo,
                    ),
                    icon: const Icon(Icons.report_problem_outlined,
                        size: 16, color: PatientTokens.danger),
                    label: Text(l10n.postConsultReportIssue,
                        style: const TextStyle(
                            color: PatientTokens.danger,
                            fontWeight: FontWeight.w700)),
                  ),
                ]),
          ),
          Image.asset('assets/illustrations/hills_footer.jpg',
              width: double.infinity,
              fit: BoxFit.fitWidth,
              excludeFromSemantics: true),
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
    final local = kenyaTime(dt);
    return '${local.day}/${local.month}, ${formatTimeEat(dt)}';
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
