import 'package:flutter/material.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/l10n/patient_labels.dart';
import '../../../l10n/app_localizations.dart';
import 'directory_models.dart';

/// Paid CHAT uses the existing slot picker and payment flow, never instant chat.
Future<void> showConsultationChoices(BuildContext context,
    {required PublicDoctorDto doctor,
    required ValueChanged<String> onBook}) async {
  final modality = await showModalBottomSheet<String>(
    context: context,
    showDragHandle: true,
    isScrollControlled: true,
    builder: (_) => ConsultationChoices(doctor: doctor),
  );
  if (context.mounted && modality != null) onBook(modality);
}

class ConsultationChoices extends StatelessWidget {
  const ConsultationChoices({super.key, required this.doctor});
  final PublicDoctorDto doctor;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(24, 8, 24, 24),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(l.consultChoose, style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 8),
          Text(doctor.displayName,
              style: Theme.of(context).textTheme.bodyMedium),
          if (doctor.supportsVideo) ...[
            const SizedBox(height: 24),
            _option(context, Icons.videocam_outlined, l.consultVideo,
                doctor.consultationFeeVideo, l.consultBookVideo, 'VIDEO'),
          ],
          if (doctor.supportsVideo && doctor.supportsChat)
            const Padding(
                padding: EdgeInsets.symmetric(vertical: 24), child: Divider()),
          if (doctor.supportsChat) ...[
            if (!doctor.supportsVideo) const SizedBox(height: 24),
            _option(context, Icons.chat_bubble_outline_rounded, l.consultChat,
                doctor.consultationFeeChat, l.consultBookChat, 'CHAT'),
          ],
        ]),
      ),
    );
  }

  Widget _option(BuildContext context, IconData icon, String title, String fee,
      String action, String modality) {
    final l = AppLocalizations.of(context);
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Icon(icon, size: 22, color: PatientTokens.primary),
        const SizedBox(width: 10),
        Expanded(
            child: Text(title, style: Theme.of(context).textTheme.titleMedium)),
      ]),
      const SizedBox(height: 8),
      Text('KES $fee', style: Theme.of(context).textTheme.titleMedium),
      const SizedBox(height: 4),
      Text(l.consultSlotNotice),
      const SizedBox(height: 16),
      SizedBox(
          width: double.infinity,
          child: FilledButton(
              onPressed: () => Navigator.pop(context, modality),
              child: Text(action, textAlign: TextAlign.center))),
    ]);
  }
}

/// Content-size filter buttons scroll horizontally instead of sharing thirds.
class DoctorDirectoryFilters extends StatelessWidget {
  const DoctorDirectoryFilters(
      {super.key,
      required this.specialty,
      required this.availability,
      required this.visitType,
      required this.onSpecialty,
      required this.onAvailability,
      required this.onVisitType});
  final String specialty, availability, visitType;
  final VoidCallback onSpecialty, onAvailability, onVisitType;
  @override
  Widget build(BuildContext context) => SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: Row(children: [
          _button(specialty, onSpecialty),
          const SizedBox(width: 8),
          _button(availability, onAvailability),
          const SizedBox(width: 8),
          _button(visitType, onVisitType),
        ]),
      );
  Widget _button(String label, VoidCallback onTap) => OutlinedButton(
        style: OutlinedButton.styleFrom(
            minimumSize: const Size(0, 48),
            padding: const EdgeInsets.symmetric(horizontal: 16)),
        onPressed: onTap,
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          Text(label, maxLines: 1, softWrap: false),
          const SizedBox(width: 6),
          const Icon(Icons.expand_more_rounded, size: 18),
        ]),
      );
}

class PatientDoctorCard extends StatelessWidget {
  const PatientDoctorCard(
      {super.key,
      required this.doctor,
      required this.onProfile,
      required this.onBook});
  final PublicDoctorDto doctor;
  final VoidCallback onProfile;
  final ValueChanged<String> onBook;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
          color: PatientTokens.surface,
          border: Border.all(color: PatientTokens.border),
          borderRadius: BorderRadius.circular(PatientTokens.cardRadius)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          CircleAvatar(
              radius: 24,
              backgroundColor: PatientTokens.surfaceSubtle,
              child: Text(doctor.initials,
                  style: const TextStyle(
                      color: PatientTokens.primaryDark,
                      fontWeight: FontWeight.w700))),
          const SizedBox(width: 12),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(doctor.displayName,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 4),
                Text(patientSpecialtyLabel(l, doctor.primarySpecialty),
                    style: const TextStyle(
                        color: PatientTokens.primary, fontSize: 14)),
              ])),
          const SizedBox(width: 8),
          Tooltip(
              message: l.profileVerified,
              child: Icon(Icons.verified_outlined,
                  semanticLabel: l.profileVerified,
                  size: 18,
                  color: PatientTokens.primary)),
        ]),
        const SizedBox(height: 12),
        Text(doctor.currentHospital,
            style: const TextStyle(color: PatientTokens.textSub, fontSize: 13)),
        const SizedBox(height: 4),
        Text(l.profileExperienceLine(doctor.experienceYears.toString()),
            style: const TextStyle(color: PatientTokens.textSub, fontSize: 13)),
        const SizedBox(height: 16), const Divider(), const SizedBox(height: 12),
        Text(doctor.isOnDuty ? l.filterOnDuty : l.consultOffDuty,
            style: const TextStyle(color: PatientTokens.textSub, fontSize: 13)),
        if (doctor.supportsVideo)
          _price(context, l.consultVideo, doctor.consultationFeeVideo),
        if (doctor.supportsChat)
          _price(context, l.consultChat, doctor.consultationFeeChat),
        const SizedBox(height: 16),
        // Flow into separate rows on narrow screens or with larger text.
        LayoutBuilder(builder: (context, constraints) {
          final stack = constraints.maxWidth < 300 ||
              MediaQuery.textScalerOf(context).scale(1) > 1.2;
          final buttons = [
            OutlinedButton(
                onPressed: onProfile,
                child: Text(l.consultProfile, textAlign: TextAlign.center)),
            if (doctor.supportsVideo || doctor.supportsChat)
              FilledButton(
                  onPressed: () {
                    if (doctor.supportsVideo && doctor.supportsChat) {
                      showConsultationChoices(context,
                          doctor: doctor, onBook: onBook);
                    } else {
                      onBook(doctor.supportsVideo ? 'VIDEO' : 'CHAT');
                    }
                  },
                  child: Text(
                      doctor.supportsVideo && doctor.supportsChat
                          ? l.consultAction
                          : doctor.supportsVideo
                              ? l.consultBookVideo
                              : l.consultBookChat,
                      textAlign: TextAlign.center)),
          ];
          if (stack) {
            return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  for (final button in buttons)
                    Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: button)
                ]);
          }
          return Row(children: [
            Expanded(child: buttons.first),
            if (buttons.length > 1) ...[
              const SizedBox(width: 8),
              Expanded(child: buttons.last)
            ]
          ]);
        }),
      ]),
    );
  }

  Widget _price(BuildContext context, String label, String fee) => Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Wrap(
            spacing: 12,
            runSpacing: 4,
            alignment: WrapAlignment.spaceBetween,
            children: [
              Text(label,
                  style: const TextStyle(
                      fontSize: 13, color: PatientTokens.textSub)),
              Text('KES $fee',
                  style: const TextStyle(
                      fontSize: 14,
                      color: PatientTokens.text,
                      fontWeight: FontWeight.w600)),
            ]),
      );
}
