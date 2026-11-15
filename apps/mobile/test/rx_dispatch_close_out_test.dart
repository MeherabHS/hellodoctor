import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/call_close_out_page.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

// Product-owner rules, 2026-11-13: ending a video call requires a recorded
// prescription decision; "no Rx" is Follow-up or an explained Other reason;
// once dispatched, the Rx for that consultation cannot be sent again.

class _Repo extends StubDoctorRepository {
  final noRx = <String, String>{};
  final completed = <String, String>{};
  @override
  Future<Json> completeNoRx(String id, String reason) async {
    noRx[id] = reason;
    return {};
  }

  @override
  Future<Json> complete(String id, String outcome) async {
    completed[id] = outcome;
    return {};
  }
}

class _QuietNotifications extends NotificationCubit {
  _QuietNotifications(super.repo);
  @override
  Future<void> load() async {}
}

AppointmentDto _apt(
        {String status = 'IN_CONSULTATION',
        String? outcome,
        DateTime? completed}) =>
    AppointmentDto(
        id: 'a1',
        status: status,
        lifecycle: status,
        patientName: 'Close Out Tester',
        modality: 'VIDEO',
        clinicalOutcome: outcome,
        completedAt: completed?.toIso8601String(),
        end: completed);

Future<_Repo> _pump(WidgetTester tester, Widget home) async {
  tester.view.physicalSize = const Size(412, 1600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final repo = _Repo();
  final cubit = DoctorCubit(repo);
  addTearDown(cubit.close);
  final notifications =
      _QuietNotifications(NotificationRepository(ApiClient(dio: Dio())));
  addTearDown(notifications.close);
  await tester.pumpWidget(MultiBlocProvider(
      providers: [
        BlocProvider<DoctorCubit>.value(value: cubit),
        BlocProvider<NotificationCubit>.value(value: notifications),
      ],
      child: MaterialApp(
          home: Scaffold(
              body: Builder(
                  builder: (c) => Center(
                      child: TextButton(
                          onPressed: () => Navigator.push(
                              c, MaterialPageRoute(builder: (_) => home)),
                          child: const Text('origin'))))))));
  await tester.tap(find.text('origin'));
  await tester.pumpAndSettle();
  return repo;
}

void main() {
  final now = DateTime.utc(2026, 11, 13, 12);

  test('a dispatched or no-Rx consultation can never take an Rx again', () {
    final recent = now.subtract(const Duration(hours: 1));
    expect(rxDispatchAllowed(_apt(), now), isTrue);
    expect(
        rxDispatchAllowed(
            _apt(
                status: 'COMPLETED',
                outcome: 'COMPLETED_WITH_RX',
                completed: recent),
            now),
        isFalse);
    expect(
        rxDispatchAllowed(
            _apt(
                status: 'COMPLETED',
                outcome: 'COMPLETED_NO_RX',
                completed: recent),
            now),
        isFalse);
    // A referred consultation keeps the 24-hour late-Rx path.
    expect(
        rxDispatchAllowed(
            _apt(status: 'COMPLETED', outcome: 'REFERRED', completed: recent),
            now),
        isTrue);
  });

  testWidgets('the Rx screen for an already-decided consultation is locked',
      (tester) async {
    await _pump(
        tester,
        ScannerPage(
            initialAppointment: _apt(
                status: 'COMPLETED',
                outcome: 'COMPLETED_WITH_RX',
                completed: DateTime.now()
                    .toUtc()
                    .subtract(const Duration(hours: 1)))));
    expect(find.textContaining('is already recorded and cannot be changed'),
        findsOneWidget);
    expect(find.text('Select current consultation'), findsOneWidget,
        reason: 'the consultation is not preselected');
  });

  testWidgets('the close-out cannot be dismissed with back', (tester) async {
    await _pump(tester, CallCloseOutPage(appointment: _apt()));
    expect(find.text('Video call ended'), findsOneWidget);
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.byType(CallCloseOutPage), findsOneWidget);
    expect(find.textContaining('Record the prescription decision before'),
        findsOneWidget);
  });

  testWidgets('No prescription needed â†’ Follow-up records it and closes',
      (tester) async {
    final repo = await _pump(tester, CallCloseOutPage(appointment: _apt()));
    await tester.tap(find.text('No prescription needed'));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text(noRxFollowUpReason).last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(repo.noRx['a1'], noRxFollowUpReason);
    expect(find.byType(CallCloseOutPage), findsNothing);
    expect(find.text('origin'), findsOneWidget);
  });

  testWidgets('Other reason must be explained', (tester) async {
    final repo = await _pump(tester, CallCloseOutPage(appointment: _apt()));
    await tester.tap(find.text('No prescription needed'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(find.text('Select a reason.'), findsOneWidget);
    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Other reason').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(find.text('Please explain the reason.'), findsOneWidget);
    expect(repo.noRx, isEmpty);
    await tester.enterText(
        find.byType(TextField), 'Patient already on effective treatment');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(repo.noRx['a1'], 'Patient already on effective treatment');
    expect(find.byType(CallCloseOutPage), findsNothing);
  });

  testWidgets('Refer or escalate records the outcome', (tester) async {
    final repo = await _pump(tester, CallCloseOutPage(appointment: _apt()));
    await tester.tap(find.text('Refer or escalate'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Refer patient'));
    await tester.pumpAndSettle();
    expect(repo.completed['a1'], 'REFERRED');
    expect(find.byType(CallCloseOutPage), findsNothing);
  });
}
