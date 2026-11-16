import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/consultation/appointment_destination.dart';
import 'package:hellodoctor_mobile_patient/features/patient/consultation/consultation_history_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

import 'booking_cubit_test.dart' show FakeBookingRepository;

/// An appointment whose server window opens [opensIn] after the server's
/// current time [serverNow] (window = start - 15 min .. end + 60 min).
AppointmentSummaryDto appointment({
  required DateTime serverNow,
  required Duration opensIn,
  String status = 'CONFIRMED',
  String modality = 'VIDEO',
  String? clinicalOutcome,
  bool lapsed = false,
}) {
  final open = serverNow.add(opensIn);
  final start = open.add(const Duration(minutes: 15));
  final end = start.add(const Duration(minutes: 10));
  return AppointmentSummaryDto.fromJson({
    'id': 'a-$status-$modality',
    'appointment_number': 'APT-1',
    'doctor_id': 'd',
    'doctor_display_name': 'Dr. Wanjiku',
    'primary_specialty': 'General Practice',
    'modality': modality,
    'status': status,
    'start_time': start.toIso8601String(),
    'end_time': end.toIso8601String(),
    'clinical_outcome': clinicalOutcome,
    'lapsed': lapsed,
    'join': {
      'eligible': false,
      'reason': modality == 'VIDEO' ? 'NOT_YET_OPEN' : 'NOT_VIDEO',
      'join_open_at': open.toIso8601String(),
      'join_close_at': end.add(const Duration(minutes: 60)).toIso8601String(),
      'server_time': serverNow.toIso8601String(),
    },
  });
}

void main() {
  group('Home card is visible only inside the server window (T-15)', () {
    for (final modality in ['VIDEO', 'CHAT']) {
      test('$modality: hidden at T-16, visible at T-15', () {
        final now = DateTime.now().toUtc();
        final state = HomeState(upcoming: [
          appointment(
              serverNow: now,
              opensIn: const Duration(minutes: 1),
              modality: modality)
        ]);
        // T-16: the window opens in one minute.
        expect(state.homeCardAppointment(DateTime.now()), isNull);
        // T-15 and later.
        expect(
            state.homeCardAppointment(
                DateTime.now().add(const Duration(minutes: 1, seconds: 1))),
            isNotNull);
      });
    }

    test('a wrong device clock does not change eligibility', () {
      // The server clock is two hours behind the device clock.
      final serverNow =
          DateTime.now().toUtc().subtract(const Duration(hours: 2));
      final state = HomeState(upcoming: [
        appointment(serverNow: serverNow, opensIn: const Duration(minutes: 1))
      ]);
      // By the device clock the start is long past, but by the server's it is
      // still T-16, so the card stays hidden until the server window opens.
      expect(state.homeCardAppointment(DateTime.now()), isNull);
      expect(
          state.homeCardAppointment(
              DateTime.now().add(const Duration(minutes: 2))),
          isNotNull);
    });

    test('cached data never shows the card; completed or cancelled never do',
        () {
      final now = DateTime.now().toUtc();
      final open =
          appointment(serverNow: now, opensIn: const Duration(minutes: -1));
      expect(HomeState(upcoming: [open]).homeCardAppointment(), isNotNull);
      expect(HomeState(upcoming: [open], fromCache: true).homeCardAppointment(),
          isNull);
      for (final status in ['COMPLETED', 'CANCELLED', 'EXPIRED']) {
        expect(
            HomeState(upcoming: [
              appointment(
                  serverNow: now,
                  opensIn: const Duration(minutes: -1),
                  status: status)
            ]).homeCardAppointment(),
            isNull,
            reason: status);
      }
    });
  });

  group('appointment destinations follow the latest server state', () {
    test('active opens the waiting room, completed the outcome, others status',
        () {
      for (final s in ['CONFIRMED', 'WAITING', 'IN_CONSULTATION']) {
        expect(destinationFor(s), AppointmentDestinationKind.waitingRoom);
      }
      expect(destinationFor('COMPLETED'), AppointmentDestinationKind.outcome);
      for (final s in ['CANCELLED', 'EXPIRED', 'PENDING_PAYMENT', 'REFUNDED']) {
        expect(destinationFor(s), AppointmentDestinationKind.status);
      }
    });

    test('a booking whose time passed unstarted never opens a waiting room',
        () {
      for (final s in ['CONFIRMED', 'WAITING', 'IN_CONSULTATION']) {
        expect(
            destinationFor(s, lapsed: true), AppointmentDestinationKind.status);
      }
    });

    test('a lapsed booking is never the upcoming one or the Home card', () {
      final now = DateTime.now().toUtc();
      final stale = appointment(
          serverNow: now, opensIn: const Duration(minutes: -1), lapsed: true);
      final next = appointment(
          serverNow: now, opensIn: const Duration(days: 1), modality: 'CHAT');
      final state = HomeState(upcoming: [stale, next]);
      expect(state.nextUpcoming, same(next));
      expect(state.homeCardAppointment(), isNull);
      expect(HomeState(upcoming: [stale]).nextUpcoming, isNull);
    });

    testWidgets(
        'an old reminder for a completed chat opens its outcome, not a waiting room',
        (tester) async {
      final family =
          FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
      await tester.pumpWidget(MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: appointmentDestinationPage(
              appointment: appointment(
                  serverNow: DateTime.now().toUtc(),
                  opensIn: const Duration(hours: -2),
                  status: 'COMPLETED',
                  modality: 'CHAT',
                  clinicalOutcome: 'COMPLETED_NO_RX'),
              repo: FakeBookingRepository(),
              familyProfilesCubit: family)));
      await tester.pumpAndSettle();
      expect(find.text('Open Chat'), findsNothing);
      expect(find.text('Join Video Call'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      await family.close();
    });
  });

  testWidgets('history lists a completed no-prescription consultation',
      (tester) async {
    final repo = _HistoryRepository();
    final family =
        FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
    await tester.pumpWidget(MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home:
            ConsultationHistoryPage(repo: repo, familyProfilesCubit: family)));
    await tester.pumpAndSettle();
    expect(find.text('You have no current consultations.'), findsOneWidget);
    await tester.tap(find.text('Past'));
    await tester.pumpAndSettle();
    expect(find.text('Dr. Wanjiku'), findsOneWidget);
    expect(find.text('No prescription needed'), findsOneWidget);
    expect(find.text('Completed'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await family.close();
  });
}

class _HistoryRepository extends FakeBookingRepository {
  @override
  Future<List<AppointmentSummaryDto>> myAppointments(String status,
          {String? profileId}) async =>
      status == 'past'
          ? [
              appointment(
                  serverNow: DateTime.now().toUtc(),
                  opensIn: const Duration(days: -1),
                  status: 'COMPLETED',
                  clinicalOutcome: 'COMPLETED_NO_RX')
            ]
          : const [];
}
