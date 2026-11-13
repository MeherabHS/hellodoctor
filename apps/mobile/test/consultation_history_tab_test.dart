import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/home/home_pages.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

AppointmentDto _a(String id, String patient, String name, String modality,
        DateTime start) =>
    AppointmentDto(
        id: id,
        status: 'COMPLETED',
        lifecycle: 'COMPLETED',
        patientId: patient,
        patientName: name,
        modality: modality,
        start: start,
        end: start.add(const Duration(minutes: 10)));

class HistoryRepository extends StubDoctorRepository {
  final rows = [
    _a('a3', 'p1', 'Amina Otieno', 'CHAT', DateTime.utc(2026, 11, 12, 8)),
    _a('b1', 'p2', 'Brian Mwangi', 'VIDEO', DateTime.utc(2026, 11, 11, 8)),
    _a('a2', 'p1', 'Amina Otieno', 'VIDEO', DateTime.utc(2026, 11, 10, 8)),
    _a('a1', 'p1', 'Amina Otieno', 'CHAT', DateTime.utc(2026, 11, 1, 8)),
  ];
  @override
  Future<List<AppointmentDto>> consultationHistory({int offset = 0}) async =>
      offset == 0 ? rows : [];
  @override
  Future<Json> summary(String id) async => {'chief_complaint': 'Follow-up'};
}

Future<void> _mount(WidgetTester tester, DoctorCubit cubit) async {
  tester.view.physicalSize = const Size(412, 1400);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(BlocProvider<DoctorCubit>.value(
      value: cubit,
      child: MaterialApp(home: ConsultationHistoryTab(onBack: () {}))));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('History tab lists past consultations, filters and searches',
      (tester) async {
    final cubit = DoctorCubit(HistoryRepository());
    addTearDown(cubit.close);
    await _mount(tester, cubit);

    expect(find.text('Consultation History'), findsOneWidget);
    expect(find.text('Patient records are not available yet'), findsNothing);
    expect(find.byType(AppointmentRow), findsNWidgets(4));

    await tester.tap(find.text('Chat'));
    await tester.pumpAndSettle();
    expect(find.byType(AppointmentRow), findsNWidgets(2));

    await tester.tap(find.text('All'));
    await tester.enterText(find.byType(TextField), 'brian');
    await tester.pumpAndSettle();
    expect(find.byType(AppointmentRow), findsOneWidget);
    expect(find.text('Brian Mwangi'), findsOneWidget);
  });

  testWidgets(
      'tapping a consultation shows the earlier consultations with that patient',
      (tester) async {
    final cubit = DoctorCubit(HistoryRepository());
    addTearDown(cubit.close);
    await _mount(tester, cubit);

    // Newest Amina consultation (12 Nov, chat).
    await tester.tap(find.byType(AppointmentRow).first);
    await tester.pumpAndSettle();
    expect(find.byType(EncounterHistoryPage), findsOneWidget);
    expect(find.text('Previous consultations with this patient'),
        findsOneWidget);
    // Two earlier consultations with Amina; Brian's is not listed.
    final previous = find.descendant(
        of: find.byType(Card), matching: find.byType(ListTile));
    expect(previous, findsNWidgets(2));
    expect(find.textContaining('Video consultation · Nov 10'), findsOneWidget);
    expect(find.textContaining('Chat consultation · Nov 1'), findsOneWidget);

    // Opening the oldest one shows no earlier consultations.
    await tester.tap(find.textContaining('Chat consultation · Nov 1'));
    await tester.pumpAndSettle();
    expect(find.text('No previous consultations.'), findsOneWidget);
  });
}
