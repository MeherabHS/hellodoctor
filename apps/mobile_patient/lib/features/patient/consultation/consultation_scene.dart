import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../../../core/l10n/kenya_time.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../directory/directory_repository.dart';
import '../directory/doctor_photo_avatar.dart';
import '../home/home_models.dart';

/// Illustrated banner shared by the consultation screens (confirmed, waiting
/// room, completed): sky wash, back arrow and brand lockup, a native heading
/// on the left and textless artwork on the right. With [wideArt] the artwork
/// spans the full width under the brand row instead (the waiting room).
class ConsultationHeader extends StatelessWidget {
  const ConsultationHeader(
      {super.key,
      required this.art,
      this.title,
      this.subtitle,
      this.showCheck = false,
      this.wideArt = false});
  final String art;
  final String? title, subtitle;
  final bool showCheck, wideArt;

  @override
  Widget build(BuildContext context) => Container(
        decoration: const BoxDecoration(
            gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [Color(0xFFCFEAF8), Color(0xFFE6F6F1)])),
        child: Column(children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 6, 12, 0),
            child: Row(children: [
              IconButton(
                  onPressed: () => Navigator.maybePop(context),
                  tooltip: MaterialLocalizations.of(context).backButtonTooltip,
                  icon: const Icon(Icons.arrow_back_rounded,
                      color: PatientTokens.navy)),
              const Expanded(child: Center(child: AfyaBrandLockup())),
              const SizedBox(width: 48),
            ]),
          ),
          if (wideArt)
            Image.asset('assets/illustrations/$art.jpg',
                width: double.infinity,
                fit: BoxFit.fitWidth,
                excludeFromSemantics: true)
          else
            IntrinsicHeight(
              child: Row(crossAxisAlignment: CrossAxisAlignment.end, children: [
                Expanded(
                  flex: 11,
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(18, 6, 4, 18),
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                if (showCheck) ...[
                                  const Padding(
                                    padding: EdgeInsets.only(top: 2),
                                    child: Icon(Icons.check_circle_rounded,
                                        size: 28, color: PatientTokens.primary),
                                  ),
                                  const SizedBox(width: 8),
                                ],
                                Expanded(
                                  child: Text(title ?? '',
                                      style: const TextStyle(
                                          fontSize: 23,
                                          height: 1.15,
                                          fontWeight: FontWeight.w800,
                                          letterSpacing: -.3,
                                          color: PatientTokens.navy)),
                                ),
                              ]),
                          if (subtitle != null) ...[
                            const SizedBox(height: 8),
                            Text(subtitle!,
                                style: const TextStyle(
                                    fontSize: 13.5,
                                    height: 1.35,
                                    color: PatientTokens.navy)),
                          ],
                        ]),
                  ),
                ),
                Expanded(
                  flex: 9,
                  child: ClipRRect(
                    borderRadius:
                        const BorderRadius.only(topLeft: Radius.circular(18)),
                    child: Image.asset('assets/illustrations/$art.jpg',
                        height: 132,
                        fit: BoxFit.cover,
                        alignment: Alignment.bottomLeft,
                        excludeFromSemantics: true),
                  ),
                ),
              ]),
            ),
        ]),
      );
}

/// Doctor identity and the appointment's date, time and type, as the
/// consultation designs show them. The photo is the doctor's real profile
/// photo (or initials); the designs' sample portraits are not used.
/// Clinical times are Nairobi time and labelled EAT.
class ConsultationDoctorCard extends StatelessWidget {
  const ConsultationDoctorCard(
      {super.key,
      required this.appointment,
      required this.photos,
      this.verifiedLabel,
      this.trailing});
  final AppointmentSummaryDto appointment;
  final DirectoryRepository photos;
  final String? verifiedLabel;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final a = appointment;
    final locale = Localizations.localeOf(context).toLanguageTag();
    final start = kenyaTime(a.startTime);
    final minutes = a.endTime.difference(a.startTime).inMinutes;
    Widget fact(IconData icon, String label, String value, [String? extra]) =>
        Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Icon(icon, size: 22, color: PatientTokens.primary),
          const SizedBox(width: 10),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(label,
                  style: const TextStyle(
                      fontSize: 12, color: PatientTokens.textSub)),
              const SizedBox(height: 2),
              Text(value,
                  style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      color: PatientTokens.navy)),
              if (extra != null)
                Text(extra,
                    style: const TextStyle(
                        fontSize: 12, color: PatientTokens.textSub)),
            ]),
          ),
        ]);
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
          color: PatientTokens.surface,
          borderRadius: BorderRadius.circular(18),
          boxShadow: PatientTokens.softShadow),
      child: Column(children: [
        Row(children: [
          DoctorPhotoAvatar(
              doctorId: a.doctorId,
              initials: a.doctorInitials,
              hasPhoto: a.hasPhoto,
              photoUpdatedAt: a.photoUpdatedAt,
              repo: photos,
              radius: 30),
          const SizedBox(width: 14),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(a.doctorDisplayName,
                  style: const TextStyle(
                      fontSize: 17,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.navy)),
              const SizedBox(height: 2),
              Text(a.primarySpecialty,
                  style: const TextStyle(
                      fontSize: 13.5, color: PatientTokens.ink)),
              const SizedBox(height: 6),
              // The pills wrap onto a second line on narrow screens or at
              // large text sizes instead of overflowing.
              Wrap(spacing: 8, runSpacing: 6, children: [
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                      color: PatientTokens.tileBlueBg,
                      borderRadius: BorderRadius.circular(20)),
                  child: Row(mainAxisSize: MainAxisSize.min, children: [
                    const Icon(Icons.verified_rounded,
                        size: 16, color: PatientTokens.tileBlue),
                    const SizedBox(width: 5),
                    Flexible(
                      child: Text(verifiedLabel ?? l.consultVerified,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w700,
                              color: PatientTokens.tileBlue)),
                    ),
                  ]),
                ),
                if (trailing != null) trailing!,
              ]),
            ]),
          ),
        ]),
        const Divider(height: 26),
        Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Expanded(
              child: fact(Icons.calendar_month_outlined, l.consultDateLabel,
                  DateFormat('EEE, d MMM y', locale).format(start))),
          const SizedBox(width: 10),
          Expanded(
              child: fact(
                  Icons.schedule_rounded,
                  l.consultTimeLabel,
                  '${formatTimeEat(a.startTime)} – ${formatTimeEat(a.endTime)}'
                      .replaceAll(' EAT', ''),
                  minutes > 0 ? l.consultDurationMinutes('$minutes') : null)),
        ]),
        const Divider(height: 26),
        fact(
            a.modality == 'VIDEO'
                ? Icons.videocam_outlined
                : Icons.chat_bubble_outline_rounded,
            l.consultTypeLabel,
            a.modality == 'VIDEO' ? l.consultTypeVideo : l.consultTypeChat),
      ]),
    );
  }
}

/// Blue guidance card with a bulleted list and optional artwork.
class ConsultationTipsCard extends StatelessWidget {
  const ConsultationTipsCard(
      {super.key, required this.title, required this.tips, this.art});
  final String title;
  final List<String> tips;
  final String? art;
  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.fromLTRB(14, 14, 10, 14),
        decoration: BoxDecoration(
            color: PatientTokens.infoCard,
            borderRadius: BorderRadius.circular(16)),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Icon(Icons.info_outline_rounded,
              size: 24, color: PatientTokens.tileBlue),
          const SizedBox(width: 10),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(title,
                  style: const TextStyle(
                      fontSize: 15,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.navy)),
              const SizedBox(height: 6),
              for (final tip in tips)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Padding(
                          padding: EdgeInsets.only(top: 7, right: 8),
                          child: CircleAvatar(
                              radius: 2.5,
                              backgroundColor: PatientTokens.tileBlue),
                        ),
                        Expanded(
                            child: Text(tip,
                                style: const TextStyle(
                                    fontSize: 13,
                                    height: 1.4,
                                    color: PatientTokens.ink))),
                      ]),
                ),
            ]),
          ),
          if (art != null)
            Padding(
              padding: const EdgeInsets.only(left: 6),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(10),
                child: Image.asset('assets/illustrations/$art.jpg',
                    width: 84, fit: BoxFit.contain, excludeFromSemantics: true),
              ),
            ),
        ]),
      );
}
