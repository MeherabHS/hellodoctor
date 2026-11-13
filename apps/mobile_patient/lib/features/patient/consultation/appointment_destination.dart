import 'package:flutter/material.dart';

import '../booking/booking_repository.dart';
import '../booking/waiting_room_page.dart';
import '../chat/consultation_status_page.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import 'post_consultation_page.dart';

/// Where an appointment should open, decided only from its latest server
/// state — never from a notification title or an old cached list.
enum AppointmentDestinationKind { waitingRoom, outcome, status }

AppointmentDestinationKind destinationFor(String status) => switch (status) {
      'CONFIRMED' || 'WAITING' || 'IN_CONSULTATION' =>
        AppointmentDestinationKind.waitingRoom,
      'COMPLETED' => AppointmentDestinationKind.outcome,
      _ => AppointmentDestinationKind.status,
    };

/// The page for [appointment] in its current state: the modality-aware
/// waiting room (video or chat) while it is active, the outcome screen once
/// completed, and the truthful status screen for anything else (cancelled,
/// expired, never confirmed).
Widget appointmentDestinationPage({
  required AppointmentSummaryDto appointment,
  required BookingRepository repo,
  required FamilyProfilesCubit familyProfilesCubit,
  String? myUserId,
}) =>
    switch (destinationFor(appointment.status)) {
      AppointmentDestinationKind.waitingRoom => WaitingRoomPage(
          appointment: appointment,
          repo: repo,
          familyProfilesCubit: familyProfilesCubit,
          myUserId: myUserId),
      AppointmentDestinationKind.outcome => PostConsultationPage(
          appointment: appointment,
          clinicalOutcome: appointment.clinicalOutcome,
          completedAt: DateTime.tryParse(appointment.completedAt ?? ''),
          repo: repo,
          familyProfilesCubit: familyProfilesCubit),
      AppointmentDestinationKind.status => ConsultationStatusPage(
          appointment: appointment,
          repo: repo,
          familyProfilesCubit: familyProfilesCubit,
          myUserId: myUserId),
    };

/// Re-reads [appointmentId] from the server, then pushes its destination.
/// Used by notification taps, history and Home so every entry point agrees.
Future<void> openAppointment(
  BuildContext context, {
  required String appointmentId,
  required BookingRepository repo,
  required FamilyProfilesCubit familyProfilesCubit,
  String? myUserId,
}) async {
  final appointment = await repo.appointmentSummary(appointmentId);
  if (!context.mounted) return;
  await Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => appointmentDestinationPage(
          appointment: appointment,
          repo: repo,
          familyProfilesCubit: familyProfilesCubit,
          myUserId: myUserId)));
}
