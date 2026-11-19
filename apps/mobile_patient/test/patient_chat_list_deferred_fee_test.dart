import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/patient_chat_list_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

/// Answers every request with 404: the list's doctor-photo lookups find no
/// photo, so the cards keep their initials and no request is left open.
class _NoPhotos implements HttpClientAdapter {
  @override
  Future<ResponseBody> fetch(RequestOptions options,
          Stream<Uint8List>? requestStream, Future<void>? cancelFuture) async =>
      ResponseBody.fromString('{"success":false}', 404, headers: {
        Headers.contentTypeHeader: ['application/json']
      });
  @override
  void close({bool force = false}) {}
}

class _FixedChatRepository extends ChatRepository {
  _FixedChatRepository(this.fixed)
      : super(ApiClient(dio: Dio()..httpClientAdapter = _NoPhotos()));
  final List<AppointmentSummaryDto> fixed;
  @override
  Future<List<AppointmentSummaryDto>> consultationPage(
          {String? profileId, int offset = 0}) async =>
      offset == 0 ? fixed : [];
}

AppointmentSummaryDto _appointment(
        {required String paymentPolicy, required String? fee}) =>
    AppointmentSummaryDto(
      id: 'a',
      appointmentNumber: 'HD1',
      doctorId: 'd',
      doctorDisplayName: 'Dr. Wanjiku',
      primarySpecialty: 'General Medicine',
      modality: 'CHAT',
      status: 'CONFIRMED',
      startTime: DateTime(2026, 11, 1),
      endTime: DateTime(2026, 11, 1, 1),
      fee: fee,
      paymentPolicy: paymentPolicy,
    );

void main() {
  testWidgets('deferred consultation never shows a fee amount in the chat list',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final repo = _FixedChatRepository([
      _appointment(paymentPolicy: 'NOT_REQUIRED', fee: '500'),
    ]);
    final family =
        FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
    await tester.pumpWidget(MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: PatientChatListPage(
            repo: repo, familyProfilesCubit: family, onBook: (_) {})));
    await tester.pumpAndSettle();
    // Let the stubbed doctor-photo lookups finish (Dio runs each step on a
    // zero-length timer).
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 20));
    }
    expect(find.textContaining('KES'), findsNothing,
        reason: 'a deferred (NOT_REQUIRED) consultation must never show a '
            'fee amount, even though the backend still stores a nominal '
            'consultation_fee for modality-availability purposes');
    await family.close();
  });

  testWidgets(
      'a legacy paid consultation shows no fee either (payment-free app)',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final repo = _FixedChatRepository([
      _appointment(paymentPolicy: 'REQUIRED', fee: '500'),
    ]);
    final family =
        FamilyProfilesCubit(FamilyProfilesRepository(ApiClient(dio: Dio())));
    await tester.pumpWidget(MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: PatientChatListPage(
            repo: repo, familyProfilesCubit: family, onBook: (_) {})));
    await tester.pumpAndSettle();
    // Let the stubbed doctor-photo lookups finish (Dio runs each step on a
    // zero-length timer).
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 20));
    }
    expect(find.textContaining('KES'), findsNothing);
    expect(find.textContaining('500'), findsNothing);
    await family.close();
  });
}
