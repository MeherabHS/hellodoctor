import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import '../directory/directory_models.dart';
import '../profile/family_profiles_cubit.dart';
import 'booking_repository.dart';
import 'patient_booking_page.dart';

/// `prototype/index.html`'s `#paidChatBookingModal`. The prototype's version
/// is a self-contained, final booking screen with no slot picker — but the
/// real backend requires a `slot_id` for CHAT bookings exactly as it does
/// for VIDEO (`AppointmentService::book_appointment` locks a schedule slot
/// regardless of modality; confirmed directly in
/// `services/backend/src/services/appointment_service.rs`). Faking a
/// skip-the-slot "instant chat" booking here would silently diverge from
/// what the backend actually does. This modal keeps the prototype's real
/// informational content (inclusions, fee breakdown, M-Pesa branding) and
/// then hands off to the same real slot-picking [PatientBookingPage] used
/// for Video, rather than inventing a parallel slot-free booking path.
Future<void> showPaidChatBookingModal(
  BuildContext context, {
  required PublicDoctorDto doctor,
  required BookingRepository repo,
  required FamilyProfilesCubit familyProfilesCubit,
}) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    builder: (sheetContext) => _PaidChatBookingModal(
      doctor: doctor,
      repo: repo,
      familyProfilesCubit: familyProfilesCubit,
    ),
  );
}

class _PaidChatBookingModal extends StatelessWidget {
  const _PaidChatBookingModal({
    required this.doctor,
    required this.repo,
    required this.familyProfilesCubit,
  });
  final PublicDoctorDto doctor;
  final BookingRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return SafeArea(
        child: SingleChildScrollView(
            child: Padding(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(children: [
            Expanded(
              child: Text(l10n.paidChatTitle,
                  style: const TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.text)),
            ),
            IconButton(
                onPressed: () => Navigator.of(context).maybePop(),
                icon: const Icon(Icons.close, size: 18)),
          ]),
          Text(l10n.paidChatSubtitle,
              style:
                  const TextStyle(fontSize: 11, color: PatientTokens.textSub)),
          const SizedBox(height: 14),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
                color: PatientTokens.surfaceAlt,
                borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
                border: Border.all(color: PatientTokens.border)),
            child: Row(children: [
              CircleAvatar(
                radius: 20,
                backgroundColor: PatientTokens.primaryLight,
                child: Text(doctor.initials,
                    style: const TextStyle(
                        fontWeight: FontWeight.w800,
                        color: PatientTokens.primaryDark)),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(doctor.displayName,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontWeight: FontWeight.w800,
                              color: PatientTokens.text)),
                      Text(
                          '${doctor.primarySpecialty} • ${doctor.licenseAuthority} #${doctor.licenseNumber}',
                          style: const TextStyle(
                              fontSize: 10.5,
                              color: PatientTokens.primaryDark,
                              fontWeight: FontWeight.w700)),
                    ]),
              ),
            ]),
          ),
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
                color: PatientTokens.surfaceAlt,
                borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
                border: Border.all(color: PatientTokens.border)),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(l10n.paidChatIncludedHeader,
                  style: const TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.primaryDark)),
              const SizedBox(height: 4),
              Text(l10n.paidChatInclusion1,
                  style: const TextStyle(
                      fontSize: 10.5, color: PatientTokens.textSub)),
              Text(l10n.paidChatInclusion2,
                  style: const TextStyle(
                      fontSize: 10.5, color: PatientTokens.textSub)),
              Text(l10n.paidChatInclusion3,
                  style: const TextStyle(
                      fontSize: 10.5, color: PatientTokens.textSub)),
              Text(l10n.paidChatInclusion4,
                  style: const TextStyle(
                      fontSize: 10.5, color: PatientTokens.textSub)),
            ]),
          ),
          const SizedBox(height: 12),
          Wrap(spacing: 16, runSpacing: 8, children: [
            Text(l10n.paidChatFeeLabel,
                style: const TextStyle(
                    fontSize: 11.5, color: PatientTokens.textSub)),
            Text(l10n.bookingFeeAmount(doctor.consultationFeeChat),
                style: const TextStyle(fontWeight: FontWeight.w700)),
          ]),
          const SizedBox(height: 14),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: () {
                Navigator.of(context).pop();
                Navigator.of(context).push(MaterialPageRoute(
                  builder: (_) => PatientBookingPage(
                    doctor: doctor,
                    modality: 'CHAT',
                    repo: repo,
                    familyProfilesCubit: familyProfilesCubit,
                  ),
                ));
              },
              style: FilledButton.styleFrom(
                  backgroundColor: PatientTokens.primary,
                  padding: const EdgeInsets.symmetric(vertical: 14)),
              child: Text(l10n.paidChatSelectSlot,
                  style: const TextStyle(fontWeight: FontWeight.w800)),
            ),
          ),
        ],
      ),
    )));
  }
}
