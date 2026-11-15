import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../booking/waiting_room_page.dart';
import '../directory/directory_repository.dart';
import '../directory/doctor_photo_avatar.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';
import 'home_cubit.dart';
import 'home_models.dart';

/// Opens the Waiting Room for an upcoming appointment, then refreshes
/// `HomeCubit` in case its status changed while the patient was there.
/// `context` must be able to read `HomeCubit`, `FamilyProfilesCubit` and
/// `ProfileCubit`.
Future<void> openWaitingRoomFor(
    BuildContext context, AppointmentSummaryDto appointment) async {
  final home = context.read<HomeCubit>();
  await Navigator.of(context).push(MaterialPageRoute(
    builder: (_) => WaitingRoomPage(
      appointment: appointment,
      repo: BookingRepository(home.repo.api,
          connectivity: home.repo.connectivity),
      familyProfilesCubit: context.read<FamilyProfilesCubit>(),
      myUserId: context.read<ProfileCubit>().state.profile?.userId,
    ),
  ));
  home.load();
}

/// The patient's nearest upcoming consultation, with the Rejoin strip for an
/// active session. It moved off the Home dashboard on 2026-11-12 (product
/// owner decision) and is shown on the consultation landing screen instead;
/// the data, states and waiting-room entry are unchanged.
class UpcomingConsultationSection extends StatelessWidget {
  const UpcomingConsultationSection(
      {super.key, required this.state, required this.onManage});
  final HomeState state;
  final VoidCallback onManage;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(18, 10, 18, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                  child: Text(l10n.homeUpcomingConsultation,
                      style: const TextStyle(
                          fontSize: 15,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text))),
              if (state.nextUpcoming != null)
                TextButton(
                    onPressed: onManage, child: Text(l10n.homeManageButton)),
            ],
          ),
          if (state.loading)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Column(children: [
                SkeletonBlock(height: 70),
              ]),
            )
          else if (state.error != null && state.nextUpcoming == null)
            InlineError(state.error!)
          else if (state.nextUpcoming == null)
            PatientCard(
              child: Text(l10n.homeNoUpcoming,
                  style: const TextStyle(color: PatientTokens.textSub)),
            )
          else
            _ConsultCard(
                appointment: state.nextUpcoming!,
                onOpen: onManage,
                repo: DirectoryRepository(context.read<HomeCubit>().repo.api)),
        ],
      ),
    );
  }
}

class _ConsultCard extends StatelessWidget {
  const _ConsultCard(
      {required this.appointment, required this.onOpen, required this.repo});
  final AppointmentSummaryDto appointment;
  final VoidCallback onOpen;
  final DirectoryRepository repo;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final start = appointment.startTime;
    final time = formatTimeEat(start);
    return PatientCard(
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (appointment.hasActiveConsultation)
          Container(
              padding: const EdgeInsets.all(12),
              margin: const EdgeInsets.only(bottom: 10),
              decoration: BoxDecoration(
                  color: const Color(0xFFDCFCE7),
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(color: const Color(0xFF86EFAC))),
              child: Row(children: [
                const Icon(Icons.videocam, color: Color(0xFF166534)),
                const SizedBox(width: 8),
                const Expanded(
                    child: Text('Active Consultation',
                        style: TextStyle(
                            color: Color(0xFF166534),
                            fontWeight: FontWeight.w800))),
                FilledButton(
                    style: FilledButton.styleFrom(
                        backgroundColor: const Color(0xFF16A34A)),
                    onPressed: onOpen,
                    child: const Text('Rejoin'))
              ])),
        Row(children: [
          DoctorPhotoAvatar(
            doctorId: appointment.doctorId,
            initials: appointment.doctorInitials,
            hasPhoto: appointment.hasPhoto,
            photoUpdatedAt: appointment.photoUpdatedAt,
            repo: repo,
            radius: 22,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(appointment.doctorDisplayName,
                    style: const TextStyle(
                        fontWeight: FontWeight.w800,
                        color: PatientTokens.text)),
                Text(appointment.primarySpecialty,
                    style: const TextStyle(
                        fontSize: 12, color: PatientTokens.textSub)),
                Text(
                    l10n.homeConsultCardLine(
                        '${start.day}',
                        '${start.month}',
                        time,
                        appointment.modality == 'VIDEO'
                            ? l10n.modalityVideo
                            : l10n.modalityChat),
                    style: const TextStyle(
                        fontSize: 11, color: PatientTokens.textSub)),
              ],
            ),
          ),
          StatusChip(_statusLabel(l10n, appointment.status)),
        ]),
        if (appointment.isWaitingForReconnect)
          Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                  appointment.doctorPresent == false &&
                          appointment.patientPresent == true
                      ? 'Doctor is reconnecting. Your consultation remains active.'
                      : appointment.doctorPresent == true &&
                              appointment.patientPresent == false
                          ? 'You are disconnected. Rejoin to continue the consultation.'
                          : 'The room is empty. Either participant can rejoin now.',
                  style: const TextStyle(
                      color: Color(0xFF166534),
                      fontSize: 11,
                      fontWeight: FontWeight.w600))),
      ]),
    );
  }

  String _statusLabel(AppLocalizations l10n, String status) => switch (status) {
        'PENDING_PAYMENT' => l10n.apptStatusPendingPayment,
        'CONFIRMED' => l10n.apptStatusConfirmed,
        'WAITING' => l10n.apptStatusWaiting,
        'IN_CONSULTATION' => l10n.apptStatusInConsultation,
        _ => status,
      };
}
