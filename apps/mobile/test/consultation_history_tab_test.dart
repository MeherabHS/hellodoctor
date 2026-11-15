import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/home/consultation_history_tab.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

AppointmentDto _a(
        String id, String patient, String name, String modality, DateTime start,
        {String status = 'COMPLETED'}) =>
    AppointmentDto(
        id: id,
        status: status,
        lifecycle: status,
        patientId: patient,
        patientName: name,
        modality: modality,
        start: start,
        end: start.add(const Duration(minutes: 10)));

class HistoryRepository extends StubDoctorRepository {
  HistoryRepository(this.rows);
  final List<AppointmentDto> rows;
  @override
  Future<List<AppointmentDto>> consultationHistory({int offset = 0}) async =>
      offset == 0 ? rows : [];
  @override
  Future<Json> summary(String id) async => {'chief_complaint': 'Follow-up'};
}

final _piled = [
  _a('a3', 'p1', 'Grace Wanjiku', 'VIDEO', DateTime.utc(2026, 11, 13, 5, 33)),
  _a('j1', 'p2', 'John Ondimu', 'CHAT', DateTime.utc(2026, 11, 11, 13, 20)),
  _a('a2', 'p1', 'Grace Wanjiku', 'CHAT', DateTime.utc(2026, 11, 10, 8)),
  _a('a1', 'p1', 'Grace Wanjiku', 'VIDEO', DateTime.utc(2026, 11, 1, 8)),
  // Booked but never completed: not part of the completed history.
  _a('x1', 'p2', 'John Ondimu', 'CHAT', DateTime.utc(2026, 11, 13, 6),
      status: 'CONFIRMED'),
];

int appointmentsOpened = 0;

Future<void> _mount(WidgetTester tester, List<AppointmentDto> rows) async {
  tester.view.physicalSize = const Size(412, 2400);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final cubit = DoctorCubit(HistoryRepository(rows));
  addTearDown(cubit.close);
  await tester.pumpWidget(BlocProvider<DoctorCubit>.value(
      value: cubit,
      child: MaterialApp(
          home: ConsultationHistoryTab(
              onBack: () {}, onOpenAppointments: () => appointmentsOpened++))));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('empty history shows the reference empty state', (tester) async {
    appointmentsOpened = 0;
    await _mount(tester, [
      _a('x1', 'p2', 'John Ondimu', 'CHAT', DateTime.utc(2026, 11, 13, 6),
          status: 'CONFIRMED')
    ]);
    expect(find.text('Consultation History'), findsOneWidget);
    expect(find.text('Review completed patient consultations'), findsOneWidget);
    expect(find.text('No consultation history yet'), findsOneWidget);
    expect(
        find.text(
            'Completed consultations will appear here once you finish a session.'),
        findsOneWidget);
    expect(find.byType(Image), findsOneWidget);
    expect(find.byType(HistoryCard), findsNothing);
    await tester.tap(find.text('Go to appointments'));
    expect(appointmentsOpened, 1);
  });

  testWidgets('completed consultations pile up as reference cards',
      (tester) async {
    await _mount(tester, _piled);
    expect(find.byType(HistoryCard), findsNWidgets(4));
    expect(find.text('Completed'), findsNWidgets(4));
    expect(find.text('Video consultation · Completed'), findsNWidgets(2));
    expect(find.text('Chat consultation · Completed'), findsNWidgets(2));
    // EAT: 05:33 UTC is 8:33 AM in Nairobi.
    expect(find.text('Nov 13 · 8:33 AM'), findsOneWidget);
    expect(
        find.text('Clinical attachments and prescriptions'), findsNWidgets(4));
    // Newest Grace card has two earlier completed consultations; John has none.
    expect(find.text('Previous consultations: 2'), findsOneWidget);
    expect(find.text('Previous consultations: 1'), findsOneWidget);
    expect(find.text('View details'), findsNWidgets(4));

    await tester.tap(find.text('Chat'));
    await tester.pumpAndSettle();
    expect(find.byType(HistoryCard), findsNWidgets(2));
    await tester.tap(find.text('Video'));
    await tester.pumpAndSettle();
    expect(find.byType(HistoryCard), findsNWidgets(2));
  });

  testWidgets('View details opens the consultation with earlier ones',
      (tester) async {
    await _mount(tester, _piled);
    await tester.tap(find.text('View details').first);
    await tester.pumpAndSettle();
    expect(find.byType(EncounterHistoryPage), findsOneWidget);
    expect(
        find.text('Previous consultations with this patient'), findsOneWidget);
    expect(find.textContaining('Chat consultation · Nov 10'), findsOneWidget);
    expect(find.textContaining('Video consultation · Nov 1 '), findsOneWidget);
  });
}
