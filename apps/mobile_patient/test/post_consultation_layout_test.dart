import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/consultation/post_consultation_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

/// The completion screen shows only what the app actually receives about a
/// finished consultation, and it must fit any screen.
void main() {
  AppointmentSummaryDto appointment(String modality) => AppointmentSummaryDto(
        id: 'a',
        appointmentNumber: 'HD1',
        doctorId: 'd',
        doctorDisplayName: 'Dr. Wanjiku Kamau',
        primarySpecialty: 'General Medicine',
        modality: modality,
        status: 'COMPLETED',
        startTime: DateTime.utc(2026, 11, 12, 7),
        endTime: DateTime.utc(2026, 11, 12, 7, 25),
        fee: null,
        paymentPolicy: 'NOT_REQUIRED',
      );

  Future<void> pump(WidgetTester tester, Size size, double scale,
      {required String modality, required String? outcome}) async {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final api = ApiClient(dio: Dio());
    final family = FamilyProfilesCubit(FamilyProfilesRepository(api));
    addTearDown(family.close);
    await tester.pumpWidget(MaterialApp(
        theme: PatientTokens.theme(),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(scale)),
            child: child!),
        home: PostConsultationPage(
            appointment: appointment(modality),
            clinicalOutcome: outcome,
            completedAt: DateTime.utc(2026, 11, 12, 7, 25),
            repo: BookingRepository(api),
            familyProfilesCubit: family)));
    await tester.pumpAndSettle();
  }

  for (final size in const [Size(320, 568), Size(411, 923), Size(800, 1280)]) {
    for (final scale in [1.0, 1.3]) {
      testWidgets('fits at $size, text x$scale', (tester) async {
        await pump(tester, size, scale,
            modality: 'VIDEO', outcome: 'COMPLETED_NO_RX');
        expect(tester.takeException(), isNull);
        expect(find.text('Video Consultation Completed'), findsOneWidget);
        expect(find.text('Dr. Wanjiku Kamau'), findsOneWidget);
        final records =
            find.widgetWithText(FilledButton, 'View Health Records');
        await tester.ensureVisible(records);
        await tester.pumpAndSettle();
        expect(records.hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
      });
    }
  }

  testWidgets('shows no invented clinical summary or follow-up plan',
      (tester) async {
    await pump(tester, const Size(411, 923), 1.0,
        modality: 'VIDEO', outcome: 'COMPLETED_NO_RX');
    expect(find.text('Consultation outcome'), findsOneWidget);
    expect(find.textContaining('Consultation Summary'), findsNothing);
    expect(find.textContaining('Follow-up Plan'), findsNothing);
  });

  testWidgets('a chat consultation is titled as a chat', (tester) async {
    await pump(tester, const Size(411, 923), 1.0,
        modality: 'CHAT', outcome: 'COMPLETED_NO_RX');
    expect(find.text('Chat Consultation Completed'), findsOneWidget);
  });
}
