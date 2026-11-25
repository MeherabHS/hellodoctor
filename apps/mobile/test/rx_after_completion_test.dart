import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:dio/dio.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

AppointmentDto _done(DateTime completed, {String status = 'COMPLETED'}) =>
    AppointmentDto(
        id: 'a1',
        status: status,
        lifecycle: status,
        patientId: 'p1',
        patientName: 'Gap Audit Tester',
        modality: 'VIDEO',
        start: completed.subtract(const Duration(minutes: 10)),
        end: completed,
        completedAt: completed.toIso8601String());

class _Repo extends StubDoctorRepository {
  @override
  Future<Json> summary(String id) async =>
      {'documents': [], 'prescriptions': []};
  @override
  Future<List<AppointmentDto>> consultationHistory({int offset = 0}) async =>
      [];
}

class _QuietNotifications extends NotificationCubit {
  _QuietNotifications(super.repo);
  @override
  Future<void> load() async {}
}

Future<void> _open(WidgetTester tester, AppointmentDto a) async {
  tester.view.physicalSize = const Size(412, 1600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final cubit = DoctorCubit(_Repo());
  addTearDown(cubit.close);
  final api = ApiClient(dio: Dio());
  final notifications = _QuietNotifications(NotificationRepository(api));
  addTearDown(notifications.close);
  await tester.pumpWidget(MultiBlocProvider(
      providers: [
        BlocProvider<DoctorCubit>.value(value: cubit),
        BlocProvider<NotificationCubit>.value(value: notifications),
      ],
      child: MaterialApp(
          home: EncounterHistoryPage(appointment: a, previous: const []))));
  await tester.pumpAndSettle();
}

void main() {
  final now = DateTime.utc(2026, 11, 13, 12);

  test('the Rx window is open for 24 hours after completion only', () {
    expect(rxWindowOpen(_done(now.subtract(const Duration(hours: 1))), now),
        isTrue);
    expect(
        rxWindowOpen(
            _done(now.subtract(const Duration(hours: 23, minutes: 59))), now),
        isTrue);
    expect(rxWindowOpen(_done(now.subtract(const Duration(hours: 25))), now),
        isFalse);
    expect(
        rxWindowOpen(
            _done(now.subtract(const Duration(hours: 1)), status: 'CONFIRMED'),
            now),
        isFalse,
        reason: 'before completion the consultation screen sends the Rx');
  });

  test('consultations with the same patient are told apart', () {
    final a = _done(DateTime.utc(2026, 11, 13, 8, 37));
    expect(
        rxConsultationLabel(a), 'Gap Audit Tester · Video · Nov 13 · 11:27 AM');
  });

  testWidgets('a recently completed consultation offers Send prescription',
      (tester) async {
    await _open(tester,
        _done(DateTime.now().toUtc().subtract(const Duration(hours: 2))));
    expect(find.text('Send prescription'), findsOneWidget);
    await tester.tap(find.text('Send prescription'));
    await tester.pumpAndSettle();
    expect(find.byType(ScannerPage), findsOneWidget);
    // The scanner opens on the capture step for this consultation.
    expect(find.text('Gap Audit Tester'), findsWidgets);
    expect(find.text('Capture Pages (Max 5)'), findsOneWidget);
  });

  testWidgets('an older consultation does not offer it', (tester) async {
    await _open(tester,
        _done(DateTime.now().toUtc().subtract(const Duration(days: 2))));
    expect(find.text('Send prescription'), findsNothing);
  });
}
