import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/consultation_status_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'booking_cubit_test.dart' show FakeBookingRepository;

class ControlledBooking extends FakeBookingRepository {
  int calls = 0;
  final pending = Completer<BookingResultDto>();
  @override
  Future<BookingResultDto> book(
      {required String doctorId,
      required String slotId,
      required String modality,
      required String? paymentMethodId,
      String? chiefComplaint,
      String? profileId}) {
    calls++;
    return pending.future;
  }
}

class StatusRepository extends FakeBookingRepository {
  StatusRepository(this.detail);
  AppointmentDetailDto detail;
  @override
  Future<AppointmentDetailDto> appointment(String id) async => detail;
}

void main() {
  test('double tap and repeat after success cannot create another booking',
      () async {
    final repo = ControlledBooking();
    final cubit = BookingCubit(repo, doctorId: 'doctor', modality: 'CHAT');
    cubit.emit(BookingState(
        selectedDate: DateTime(2026, 11, 1),
        selectedSlotId: 'slot',
        loadingPolicy: false,
        paymentRequired: false));
    final first = cubit.submit();
    expect(await cubit.submit(), isFalse);
    expect(repo.calls, 1);
    repo.pending.complete(const BookingResultDto(
        appointmentId: 'a',
        appointmentNumber: 'HD1',
        status: 'CONFIRMED',
        paymentPolicy: 'NOT_REQUIRED',
        amount: '0'));
    expect(await first, isTrue);
    expect(cubit.state.canProceed, isFalse);
    expect(await cubit.submit(), isFalse);
    expect(repo.calls, 1);
    await cubit.close();
  });
  test('only confirmed backend appointment states unlock consultation', () {
    for (final status in [
      'PENDING_PAYMENT',
      'CANCELLED',
      'REFUNDED',
      'EXPIRED',
      'PREMATURE_TERMINATION',
      'DISPUTED',
    ]) {
      expect(
          AppointmentDetailDto.fromJson(
                  {'id': 'a', 'status': status, 'payment_status': 'INITIATED'})
              .isReady,
          isFalse);
    }
    final ready = AppointmentDetailDto.fromJson(
        {'id': 'a', 'status': 'CONFIRMED', 'payment_status': 'PAYMENT_HELD'});
    expect(ready.isReady, isTrue);
    expect(ready.paymentStatus, 'PAYMENT_HELD');
    expect(AppointmentSummaryDto.fromJson({'consultation_fee': '750.00'}).fee,
        '750.00');
  });
  // Payment-free status page: every state is shown from the server's
  // appointment status, with no price, payment status or checkout.
  final cases = <(String, String, String, String)>[
    ('CONFIRMED', 'CHAT', 'Consultation confirmed', 'Open chat waiting room'),
    ('CONFIRMED', 'VIDEO', 'Consultation confirmed', 'Open waiting room'),
    ('IN_CONSULTATION', 'VIDEO', 'Consultation in progress', 'Open waiting room'),
    ('COMPLETED', 'CHAT', 'Consultation completed', 'View outcome'),
    ('CANCELLED', 'CHAT', 'This consultation is closed', 'View messages'),
    ('PENDING_PAYMENT', 'CHAT', 'This booking is not active',
        'Book a new consultation'),
  ];
  for (final (status, modality, title, action) in cases) {
    testWidgets('$status $modality shows its real state and no payment text',
        (tester) async {
      tester.view.physicalSize = const Size(320, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final family =
          FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
      final repo = StatusRepository(AppointmentDetailDto(
          id: 'a', status: status, paymentPolicy: 'NOT_REQUIRED'));
      await tester.pumpWidget(MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: ConsultationStatusPage(
              appointment: AppointmentSummaryDto(
                  id: 'a',
                  appointmentNumber: 'HD1',
                  doctorId: 'd',
                  doctorDisplayName: 'Dr. Wanjiku',
                  primarySpecialty: 'General Medicine',
                  modality: modality,
                  status: status,
                  startTime: DateTime(2026, 11, 1),
                  endTime: DateTime(2026, 11, 1, 1),
                  fee: '750'),
              repo: repo,
              familyProfilesCubit: family)));
      await tester.pump();
      expect(find.text(title), findsOneWidget);
      await tester.scrollUntilVisible(find.text(action), 200,
          scrollable: find.byType(Scrollable).first);
      expect(find.text(action), findsOneWidget);
      for (final text in const ['KES', 'Payment', 'payment', 'Paid', '750']) {
        expect(find.textContaining(text), findsNothing, reason: text);
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      await family.close();
    });
  }
}