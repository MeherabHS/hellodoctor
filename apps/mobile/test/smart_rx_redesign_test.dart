import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

// Smart Rx redesign (2026-11-24, docs/design/doctor-ui/smart-rx-dispatch-flow.png):
// pick a consultation (All / Live / Late Rx / Locked), then capture and
// dispatch. The rules are unchanged: a prescription decision is final.

class _QuietNotifications extends NotificationCubit {
  _QuietNotifications(super.repo);
  @override
  Future<void> load() async {}
}

AppointmentDto _apt(String id, String name, String status,
        {String? outcome,
        DateTime? completed,
        DateTime? start,
        bool chat = false}) =>
    AppointmentDto(
        id: id,
        number: 'AM-$id',
        status: status,
        lifecycle: status,
        patientName: name,
        modality: chat ? 'CHAT' : 'VIDEO',
        clinicalOutcome: outcome,
        completedAt: completed?.toIso8601String(),
        end: completed,
        start: start ?? DateTime.now().toUtc());

final _now = DateTime.now().toUtc();
final _queue = [
  _apt('jm', 'John Mwangi', 'IN_CONSULTATION'),
  _apt('an', 'Amina Njeri', 'WAITING', chat: true),
  _apt('pk', 'Peter Kariuki', 'CONFIRMED'),
  // Completed 2 hours ago, no decision yet: the late-Rx window is open.
  _apt('sn', 'Sarah Nditiru', 'COMPLETED',
      outcome: 'REFERRED', completed: _now.subtract(const Duration(hours: 2))),
  // Prescription already sent yesterday: shown, but locked.
  _apt('dk', 'David Kimani', 'COMPLETED',
      outcome: 'COMPLETED_WITH_RX',
      completed: _now.subtract(const Duration(hours: 20))),
  // Decided long ago: not shown at all.
  _apt('old', 'Old Patient', 'COMPLETED',
      outcome: 'COMPLETED_NO_RX',
      completed: _now.subtract(const Duration(days: 5))),
];

Future<void> _pump(WidgetTester tester, Widget home,
    {Size size = const Size(412, 1800)}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final repo = StubDoctorRepository()
    ..queueResponse = (Completer<List<AppointmentDto>>()..complete(_queue));
  final cubit = DoctorCubit(repo);
  addTearDown(cubit.close);
  await cubit.loadAll();
  final notifications =
      _QuietNotifications(NotificationRepository(ApiClient(dio: Dio())));
  addTearDown(notifications.close);
  await tester.pumpWidget(MultiBlocProvider(
      providers: [
        BlocProvider<DoctorCubit>.value(value: cubit),
        BlocProvider<NotificationCubit>.value(value: notifications),
      ],
      child: MaterialApp(
          home: Builder(
              builder: (c) => Scaffold(
                  body: Center(
                      child: TextButton(
                          onPressed: () => Navigator.push(
                              c, MaterialPageRoute(builder: (_) => home)),
                          child: const Text('origin'))))))));
  await tester.tap(find.text('origin'));
  await tester.pumpAndSettle();
}

void main() {
  test('the list holds completed consultations only', () {
    final now = DateTime.now();
    // Not completed: never in the list (prescribed from the consultation).
    expect(rxKindName(_queue[0], now), isNull);
    expect(rxKindName(_queue[1], now), isNull);
    expect(rxKindName(_queue[2], now), isNull);
    expect(rxKindName(_queue[3], now), 'awaiting');
    expect(rxKindName(_queue[4], now), 'locked');
    expect(rxKindName(_queue[5], now), isNull);
  });

  test('a prescription only after the consultation has started', () {
    final now = DateTime.now().toUtc();
    AppointmentDto c(String status, {bool chat = false, Duration? startIn}) =>
        _apt('x', 'X', status,
            chat: chat, start: startIn == null ? null : now.add(startIn));
    // Video: someone has joined (IN_CONSULTATION); nobody yet: no.
    expect(rxDispatchAllowed(c('IN_CONSULTATION'), now), isTrue);
    expect(rxDispatchAllowed(c('CONFIRMED'), now), isFalse);
    expect(rxDispatchAllowed(c('WAITING'), now), isFalse);
    // Chat: once its window opens (15 minutes before the start).
    expect(
        rxDispatchAllowed(
            c('CONFIRMED', chat: true, startIn: const Duration(minutes: 10)),
            now),
        isTrue);
    expect(
        rxDispatchAllowed(
            c('CONFIRMED', chat: true, startIn: const Duration(hours: 2)), now),
        isFalse);
    // Completed: within 24 hours, no decision yet.
    expect(rxDispatchAllowed(_queue[3], now), isTrue);
    expect(rxDispatchAllowed(_queue[4], now), isFalse);
    expect(rxHoursLeft(_queue[3], now), 22);
  });

  test('times read as Today / Yesterday in Nairobi time', () {
    final now = DateTime.utc(2026, 11, 24, 9); // 12:00 in Nairobi
    expect(
        rxWhen(DateTime.utc(2026, 11, 24, 7, 30), now: now), 'Today, 10:30 AM');
    expect(rxWhen(DateTime.utc(2026, 11, 23, 13, 15), now: now),
        'Yesterday, 04:15 PM');
    expect(rxWhen(DateTime.utc(2026, 11, 20, 6), now: now), '20 Nov, 09:00 AM');
    expect(rxWhen(null, now: now), '');
  });

  testWidgets('picker: filters, locked rows and Continue', (tester) async {
    await _pump(tester, const ScannerPage());
    expect(tester.takeException(), isNull);
    expect(find.text('Select Consultation'), findsOneWidget);
    expect(find.text('All (2)'), findsOneWidget);
    expect(find.text('Awaiting Rx (1)'), findsOneWidget);
    expect(find.text('Locked (1)'), findsOneWidget);
    // Completed consultations only.
    for (final name in [
      'John Mwangi',
      'Amina Njeri',
      'Peter Kariuki',
      'Old Patient'
    ]) {
      expect(find.text(name), findsNothing, reason: name);
    }
    expect(find.text('Sarah Nditiru'), findsOneWidget);
    expect(find.text('22 h left'), findsOneWidget);
    expect(
        find.text('Already recorded and cannot be changed.'), findsOneWidget);

    FilledButton continueButton() => tester
        .widget<FilledButton>(find.widgetWithText(FilledButton, 'Continue'));
    expect(continueButton().onPressed, isNull, reason: 'nothing picked yet');

    // A locked consultation cannot be picked.
    await tester.tap(find.text('David Kimani'));
    await tester.pump();
    expect(continueButton().onPressed, isNull);

    // Filters narrow the list.
    await tester.ensureVisible(find.text('Awaiting Rx (1)'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Awaiting Rx (1)'));
    await tester.pumpAndSettle();
    expect(find.text('Sarah Nditiru'), findsOneWidget);
    expect(find.text('David Kimani'), findsNothing);
    await tester.ensureVisible(find.text('All (2)'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('All (2)'));
    await tester.pumpAndSettle();

    // Search by name.
    await tester.enterText(find.byType(TextField), 'sarah');
    await tester.pumpAndSettle();
    expect(find.text('Sarah Nditiru'), findsOneWidget);
    expect(find.text('David Kimani'), findsNothing);
    await tester.enterText(find.byType(TextField), '');
    await tester.pumpAndSettle();

    // Pick the completed consultation and continue to the capture step.
    await tester.tap(find.text('Sarah Nditiru'));
    await tester.pump();
    expect(continueButton().onPressed, isNotNull);
    await tester.tap(find.widgetWithText(FilledButton, 'Continue'));
    await tester.pumpAndSettle();
    expect(find.text('Capture Pages (Max 5)'), findsOneWidget);
    expect(find.text('#AM-sn'), findsOneWidget);
    expect(find.text('Patient Case History'), findsOneWidget);
    expect(find.text('Add page'), findsNWidgets(5));
    expect(find.text('0/5 pages'), findsOneWidget);
    expect(find.text('Dispatch Prescription to Patient'), findsOneWidget);
    expect(
        find.text(
            'Unsent photos are removed from this device when you leave this screen.'),
        findsOneWidget);

    // The advice counter follows the text.
    await tester.enterText(find.byType(TextField), 'Take after meals.');
    await tester.pump();
    expect(find.text('17/1000'), findsOneWidget);

    // Back returns to the picker (pages and notes cleared), not to origin.
    await tester.tap(find.byTooltip('Back'));
    await tester.pumpAndSettle();
    expect(find.text('Select Consultation'), findsOneWidget);
    expect(find.text('origin'), findsNothing);
  });

  testWidgets('dispatching with no pages asks for a page first',
      (tester) async {
    await _pump(tester, ScannerPage(initialAppointment: _queue[0]));
    expect(find.text('Capture Pages (Max 5)'), findsOneWidget,
        reason: 'opened for one consultation: straight to capture');
    await tester.tap(find.text('Dispatch Prescription to Patient'));
    await tester.pump();
    expect(find.text('Capture or attach a prescription page first.'),
        findsOneWidget);
    // Opened for one consultation, back leaves the screen.
    await tester.pumpAndSettle(const Duration(seconds: 5));
    await tester.tap(find.byTooltip('Back'));
    await tester.pumpAndSettle();
    expect(find.text('origin'), findsOneWidget);
  });

  testWidgets('opened for a consultation that has not started, it explains why',
      (tester) async {
    await _pump(tester, ScannerPage(initialAppointment: _queue[2]));
    expect(
        find.text(
            'A prescription for Peter Kariuki can be sent once the consultation has started.'),
        findsOneWidget);
    expect(find.text('Capture Pages (Max 5)'), findsNothing);
  });

  for (final size in const [Size(320, 568), Size(412, 915)]) {
    testWidgets('both steps lay out without overflow at $size', (tester) async {
      await _pump(tester, const ScannerPage(), size: size);
      expect(tester.takeException(), isNull);
      // Centre the row (clear of the fixed Continue bar) before tapping.
      await tester.runAsync(() => Scrollable.ensureVisible(
          tester.element(find.text('Sarah Nditiru')),
          alignment: .5));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Sarah Nditiru'));
      await tester.pump();
      await tester.tap(find.widgetWithText(FilledButton, 'Continue'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Patient Case History'), findsOneWidget);
      // Below the fold on small phones: scroll down like a doctor would.
      await tester.scrollUntilVisible(find.text('Add page').last, 200,
          scrollable: find.byType(Scrollable).first);
      expect(tester.takeException(), isNull);
      expect(find.text('Capture Pages (Max 5)'), findsOneWidget);
    });
  }
}
