import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/l10n/kenya_time.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/availability_refresh.dart';
import 'package:hellodoctor_mobile_patient/core/network/connectivity_service.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/consultation_status_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';

class ControlledAvailability extends BookingRepository {
  ControlledAvailability() : super(ApiClient(dio: Dio()));
  Completer<List<DoctorSlotDto>>? request;
  @override
  Future<List<DoctorSlotDto>> slots(String id) =>
      (request = Completer<List<DoctorSlotDto>>()).future;
}

class ControlledPayment extends BookingRepository {
  ControlledPayment() : super(ApiClient(dio: Dio()));
  int calls = 0;
  final payment = Completer<Uri>();
  @override
  Future<AppointmentDetailDto> appointment(String id) async =>
      const AppointmentDetailDto(
        id: 'a',
        status: 'PENDING_PAYMENT',
        paymentPolicy: 'REQUIRED',
        paymentStatus: 'INITIATED',
      );
  @override
  Future<Uri> initiatePayment(String id) {
    calls++;
    return payment.future;
  }
}

void main() {
  test(
      'availability refresh clears obsolete selection and disables stale booking during fetch',
      () async {
    final repo = ControlledAvailability();
    final cubit = BookingCubit(repo, doctorId: 'd', modality: 'CHAT');
    final now = DateTime.now().toUtc();
    final slot = DoctorSlotDto(
      id: 'old',
      doctorId: 'd',
      startTime: now.add(const Duration(minutes: 30)),
      endTime: now.add(const Duration(minutes: 40)),
    );
    cubit.emit(
      BookingState(
        slots: [slot],
        selectedDate: slot.startTime,
        selectedSlotId: slot.id,
        selectedPaymentMethodId: 'pm-1',
      ),
    );
    expect(cubit.state.canProceed, isTrue);
    final fetch = cubit.loadSlots();
    expect(cubit.state.slots, isEmpty);
    expect(cubit.state.canProceed, isFalse);
    repo.request!.complete([]);
    await fetch;
    expect(cubit.state.selectedSlotId, isNull);
    expect(cubit.state.canProceed, isFalse);
    await cubit.close();
  });
  test(
    'refresh preserves a still-valid selected slot and its matching date',
    () async {
      final repo = ControlledAvailability();
      final cubit = BookingCubit(repo, doctorId: 'd', modality: 'CHAT');
      final now = DateTime.now().toUtc();
      final today = DoctorSlotDto(
        id: 'today',
        doctorId: 'd',
        startTime: now.add(const Duration(minutes: 30)),
        endTime: now.add(const Duration(minutes: 40)),
      );
      final tomorrow = DoctorSlotDto(
        id: 'tomorrow',
        doctorId: 'd',
        startTime: now.add(const Duration(days: 1)),
        endTime: now.add(const Duration(days: 1, minutes: 10)),
      );
      cubit.emit(
        BookingState(
          slots: [today, tomorrow],
          selectedDate: tomorrow.startTime,
          selectedSlotId: 'tomorrow',
          selectedPaymentMethodId: 'pm-1',
        ),
      );
      final refresh = cubit.loadSlots();
      repo.request!.complete([today, tomorrow]);
      await refresh;
      expect(cubit.state.selectedSlotId, 'tomorrow');
      expect(cubit.state.selectedDate!.day, tomorrow.startTime.day);
      expect(cubit.state.canProceed, isTrue);
      await cubit.close();
    },
  );
  test(
    'failed availability refresh cannot leave stale slots actionable',
    () async {
      final repo = ControlledAvailability();
      final cubit = BookingCubit(repo, doctorId: 'd', modality: 'CHAT');
      cubit.emit(
        BookingState(selectedDate: DateTime.now(), selectedSlotId: 'old'),
      );
      final fetch = cubit.loadSlots();
      repo.request!.completeError(
        const ApiException(null, 'Network unavailable'),
      );
      await fetch;
      expect(cubit.state.slots, isEmpty);
      expect(cubit.state.selectedSlotId, isNull);
      expect(cubit.state.canProceed, isFalse);
      expect(cubit.state.slotsError, isNotNull);
      await cubit.close();
    },
  );
  test(
    'Nairobi availability dates preserve UTC instants across local midnight',
    () {
      final utc = DateTime.now().toUtc();
      final value = DateTime.utc(utc.year, utc.month, utc.day, 21, 30);
      final slot = DoctorSlotDto.fromJson({
        'id': 's',
        'doctor_id': 'd',
        'start_time': value.toIso8601String(),
        'end_time': value.add(const Duration(minutes: 10)).toIso8601String(),
      });
      expect(slot.startTime.toUtc(), value);
      expect(slot.startTime.hour, 0);
      expect(slot.startTime.minute, 30);
      expect(slot.startTime.day, kenyaTime(value).day);
      expect(slot.startTime.timeZoneOffset, const Duration(hours: 3));
    },
  );
  testWidgets(
    'resume and network recovery request new server availability and unsubscribe on disposal',
    (tester) async {
      final network = StreamController<NetworkStatus>.broadcast();
      var calls = 0;
      final refresh = AvailabilityRefresh(
        () => calls++,
        null,
        networkChanges: network.stream,
      );
      refresh.didChangeAppLifecycleState(AppLifecycleState.paused);
      expect(calls, 0);
      refresh.didChangeAppLifecycleState(AppLifecycleState.resumed);
      expect(calls, 0);
      await tester.pump(const Duration(seconds: 2));
      expect(calls, 1);
      network.add(NetworkStatus.online);
      await tester.pump();
      expect(calls, 1);
      await tester.pump(const Duration(seconds: 2));
      expect(calls, 2);
      refresh.dispose();
      network.add(NetworkStatus.online);
      await tester.pump();
      expect(calls, 2);
      await network.close();
    },
  );
  for (final width in [320.0, 360.0, 390.0, 412.0]) {
    testWidgets(
      'unpaid consultation has one payment CTA, no fake success, and safe layout at $width',
      (tester) async {
        tester.view.physicalSize = Size(width, 850);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final repo = ControlledPayment();
        final family = FamilyProfilesCubit(
          FamilyProfilesRepository(ApiClient(dio: Dio())),
        );
        final now = DateTime.now().toUtc();
        await tester.pumpWidget(
          MaterialApp(
            home: ConsultationStatusPage(
              appointment: AppointmentSummaryDto(
                id: 'a',
                appointmentNumber: 'APT-TEST',
                doctorId: 'd',
                doctorDisplayName: 'Dr. Test',
                primarySpecialty: 'General Medicine',
                modality: 'CHAT',
                status: 'PENDING_PAYMENT',
                startTime: now.add(const Duration(hours: 1)),
                endTime: now.add(const Duration(hours: 2)),
                fee: '500',
              ),
              repo: repo,
              familyProfilesCubit: family,
            ),
          ),
        );
        await tester.pump();
        expect(find.byType(FilledButton), findsOneWidget);
        expect(find.text('Proceed to payment method'), findsOneWidget);
        expect(find.text('Open waiting room'), findsNothing);
        expect(find.text('Open consultation'), findsNothing);
        expect(find.textContaining('development environment'), findsNothing);
        await tester.tap(find.text('Proceed to payment method'));
        await tester.pump();
        expect(repo.calls, 1);
        expect(
          tester.widget<FilledButton>(find.byType(FilledButton)).onPressed,
          isNull,
        );
        repo.payment.completeError(
          const ApiException(
            409,
            'Payment service is currently unavailable. Please try again later.',
          ),
        );
        await tester.pump();
        expect(
          find.text(
            'Payment service is currently unavailable. Please try again later.',
          ),
          findsOneWidget,
        );
        expect(find.text('Consultation confirmed'), findsNothing);
        expect(find.text('Open consultation'), findsNothing);
        expect(repo.calls, 1);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
        await family.close();
      },
    );
  }
  test(
      'attachment metadata maps to shared message identity without raw storage URLs',
      () {
    final message = ChatMessageDto.fromJson({
      'id': 'message',
      'sender_id': 'doctor-user',
      'content': '',
      'sent_at': DateTime.now().toUtc().toIso8601String(),
      'attachment_id': 'message',
      'attachment': {'size_bytes': 32768, 'display_name': 'Clinical image'},
    });
    expect(message.attachmentId, message.id);
    expect(message.attachmentSize, 32768);
    expect(message.senderId, 'doctor-user');
  });
}
