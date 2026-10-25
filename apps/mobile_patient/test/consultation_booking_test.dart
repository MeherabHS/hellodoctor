import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
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
      required String paymentMethodId,
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
        selectedPaymentMethodId: 'pm-1'));
    final first = cubit.submit();
    expect(await cubit.submit(), isFalse);
    expect(repo.calls, 1);
    repo.pending.complete(const BookingResultDto(
        appointmentId: 'a',
        appointmentNumber: 'HD1',
        status: 'PENDING_PAYMENT',
        amount: '300'));
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
  for (final payment in ['INITIATED', 'FAILED', 'PAYMENT_HELD']) {
    testWidgets('$payment renders server truth and preserves narrow layout',
        (tester) async {
      tester.view.physicalSize = const Size(320, 850);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final family =
          FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
      final repo = StatusRepository(AppointmentDetailDto(
          id: 'a',
          status: payment == 'PAYMENT_HELD' ? 'CONFIRMED' : 'PENDING_PAYMENT',
          paymentStatus: payment));
      await tester.pumpWidget(MaterialApp(
          home: ConsultationStatusPage(
              appointment: AppointmentSummaryDto(
                  id: 'a',
                  appointmentNumber: 'HD1',
                  doctorId: 'd',
                  doctorDisplayName: 'Dr. Wanjiku',
                  primarySpecialty: 'General Medicine',
                  modality: 'CHAT',
                  status: 'PENDING_PAYMENT',
                  startTime: DateTime(2026, 11, 1),
                  endTime: DateTime(2026, 11, 1, 1),
                  fee: '750'),
              repo: repo,
              familyProfilesCubit: family)));
      await tester.pump();
      final button = tester.widget<FilledButton>(find.widgetWithText(
          FilledButton,
          payment == 'PAYMENT_HELD'
              ? 'Open consultation'
              : 'Proceed to payment method'));
      expect(button.onPressed != null, isTrue);
      expect(find.text('Open consultation'),
          payment == 'PAYMENT_HELD' ? findsOneWidget : findsNothing);
      expect(find.text('Proceed to payment method'),
          payment == 'PAYMENT_HELD' ? findsNothing : findsOneWidget);
      expect(
          find.text(payment == 'PAYMENT_HELD'
              ? 'Consultation confirmed'
              : payment == 'FAILED'
                  ? 'Payment failed'
                  : 'Payment required'),
          findsOneWidget);
      expect(find.text('KES 750'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      await family.close();
    });
  }

  testWidgets('renders a server-authoritative ten-minute hold countdown',
      (tester) async {
    final now = DateTime.now().toUtc();
    final family =
        FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
    final repo = StatusRepository(AppointmentDetailDto(
      id: 'a',
      status: 'PENDING_PAYMENT',
      paymentStatus: 'INITIATED',
      holdExpiresAt: now.add(const Duration(minutes: 10)),
      serverTime: now,
    ));
    await tester.pumpWidget(MaterialApp(
        home: ConsultationStatusPage(
            appointment: AppointmentSummaryDto(
                id: 'a',
                appointmentNumber: 'HD1',
                doctorId: 'd',
                doctorDisplayName: 'Dr. Wanjiku',
                primarySpecialty: 'General Medicine',
                modality: 'CHAT',
                status: 'PENDING_PAYMENT',
                startTime: now,
                endTime: now.add(const Duration(minutes: 30)),
                fee: '750'),
            repo: repo,
            familyProfilesCubit: family)));
    await tester.pump();
    expect(find.textContaining('Reservation expires in '), findsOneWidget);
    await tester.pump(const Duration(seconds: 1));
    expect(find.textContaining('Reservation expires in '), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    await family.close();
  });
}
