import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/pre_consult_intake_modal.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class _NoopStore implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

class _FakeRepo extends BookingRepository {
  _FakeRepo(this.uploads)
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));
  IntakeUploads uploads;
  final removed = <String>[];
  Object? removeError;

  @override
  Future<IntakeUploads> intakeUploads(String appointmentId) async => uploads;

  @override
  Future<void> removeIntake(String appointmentId, String documentId) async {
    if (removeError != null) throw removeError!;
    removed.add(documentId);
  }
}

Widget _host(BookingRepository repo) => MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: PreConsultIntakeModal(
            doctorName: 'Dr. Amina Odhiambo',
            appointmentId: 'apt-1',
            repo: repo),
      ),
    );

const _twoPages = IntakeUploads(editable: true, documents: [
  IntakeUploadDto(id: 'd1', pageNumber: 1, mimeType: 'image/jpeg'),
  IntakeUploadDto(id: 'd2', pageNumber: 2, mimeType: 'image/jpeg'),
]);

void main() {
  testWidgets('lists uploaded pages and removes one after confirmation',
      (tester) async {
    final repo = _FakeRepo(_twoPages);
    await tester.pumpWidget(_host(repo));
    await tester.pumpAndSettle();

    expect(find.text('Already uploaded'), findsOneWidget);
    expect(find.text('Page 1'), findsOneWidget);
    expect(find.text('Page 2'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('intake-remove-d2')));
    await tester.pumpAndSettle();
    expect(find.text('Remove this page?'), findsOneWidget);
    await tester.tap(find.widgetWithText(TextButton, 'Remove'));
    await tester.pumpAndSettle();

    expect(repo.removed, ['d2']);
    expect(find.text('Page 2'), findsNothing);
    expect(find.text('Page 1'), findsOneWidget);
  });

  testWidgets('cancelling the confirmation keeps the page', (tester) async {
    final repo = _FakeRepo(_twoPages);
    await tester.pumpWidget(_host(repo));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('intake-remove-d1')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
    await tester.pumpAndSettle();

    expect(repo.removed, isEmpty);
    expect(find.text('Page 1'), findsOneWidget);
  });

  testWidgets('a server refusal shows its message and keeps the page',
      (tester) async {
    final repo = _FakeRepo(_twoPages)
      ..removeError = const ApiException(
          409, 'Uploads can no longer be changed for this consultation.');
    await tester.pumpWidget(_host(repo));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('intake-remove-d1')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, 'Remove'));
    await tester.pumpAndSettle();

    expect(find.text('Uploads can no longer be changed for this consultation.'),
        findsOneWidget);
    expect(find.text('Page 1'), findsOneWidget);
  });

  testWidgets('a closed consultation shows pages read-only', (tester) async {
    final repo = _FakeRepo(const IntakeUploads(editable: false, documents: [
      IntakeUploadDto(id: 'd1', pageNumber: 1, mimeType: 'image/jpeg'),
    ]));
    await tester.pumpWidget(_host(repo));
    await tester.pumpAndSettle();

    expect(find.text('Page 1'), findsOneWidget);
    expect(find.byKey(const ValueKey('intake-remove-d1')), findsNothing);
    expect(find.text('Uploads for this consultation can no longer be changed.'),
        findsOneWidget);
  });

  test('IntakeUploads parses the server response', () {
    final parsed = IntakeUploads.fromJson({
      'editable': true,
      'documents': [
        {'id': 'a', 'page_number': 3, 'mime_type': 'application/pdf'},
        {'page_number': 4},
      ],
    });
    expect(parsed.editable, isTrue);
    expect(parsed.documents, hasLength(1));
    expect(parsed.documents.single.pageNumber, 3);
    expect(parsed.documents.single.isPdf, isTrue);
  });
}
