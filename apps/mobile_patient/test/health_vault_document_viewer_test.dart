// Remediation E2 security regression coverage for the Health Vault's
// document viewing: clinical images stay on their existing safe in-app path,
// and clinical PDFs — previously handed to an external application via
// `launchUrl` — now route to the in-app ClinicalPdfViewer instead.
// Platform-channel-backed pdfx rendering is not driven here (this harness
// has no native pdfium binary) — see the Doctor app's equivalent test file
// and the Remediation E2 report for why only graceful-failure, not real
// on-device rendering, is verified here.
import 'dart:convert';
import 'dart:io' show Platform;
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/widgets/patient_widgets.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/clinical_pdf_viewer.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/health_vault_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/vault_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/vault_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

const _pdfDoc = {
  'kind': 'INTAKE',
  'id': 'doc-pdf-1',
  'appointment_id': 'apt-1',
  'page_number': 1,
  'mime_type': 'application/pdf',
  'created_at': '2026-10-09T08:00:00Z',
};

const _imageDoc = {
  'kind': 'PRESCRIPTION',
  'id': 'doc-img-1',
  'appointment_id': 'apt-1',
  'rx_number': 'RX-20261010-ABCD',
  'page_number': 1,
  'mime_type': 'image/jpeg',
  'created_at': '2026-10-10T08:00:00Z',
};

const _unsupportedDoc = {
  'kind': 'INTAKE',
  'id': 'doc-zip-1',
  'appointment_id': 'apt-1',
  'page_number': 1,
  'mime_type': 'application/zip',
  'created_at': '2026-10-11T08:00:00Z',
};

typedef _FetchResult = Future<TypedBytes> Function();

class _FakeVaultRepository extends VaultRepository {
  _FakeVaultRepository(this._documents, {_FetchResult? fetch})
      : _fetch = fetch,
        super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));
  final List<PatientDocumentDto> _documents;
  final _FetchResult? _fetch;
  int fetchCount = 0;

  @override
  Future<List<PatientDocumentDto>> documents({String? profileId}) async =>
      _documents;

  @override
  Future<List<PatientDocumentDto>> cachedDocuments() async => const [];

  @override
  Future<Uri> resolveDownloadUrl(
          String appointmentId, String documentId) async =>
      Uri.parse('https://example.test/not-a-real-host/$documentId');

  @override
  Future<TypedBytes> fetchDocumentBytes(
      String appointmentId, String documentId) {
    fetchCount++;
    if (_fetch == null) {
      throw StateError('fetchDocumentBytes was not expected to be called');
    }
    return _fetch();
  }
}

class _FakeFamilyProfilesRepository extends FamilyProfilesRepository {
  _FakeFamilyProfilesRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));
  @override
  Future<List<PatientProfileDto>> list() async => const [];
  @override
  Future<List<PatientProfileDto>> cachedList() async => const [];
}

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

Widget _harness(VaultRepository repo) => MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: HealthVaultPage(
        repo: repo,
        familyProfilesCubit:
            FamilyProfilesCubit(_FakeFamilyProfilesRepository()),
      ),
    );

/// `pdfx` (see the Doctor app's equivalent test file for the full
/// explanation) has no desktop rendering backend on Linux at all —
/// `PdfDocument.openData` throws `PlatformNotSupportedException` before
/// even reaching a platform channel, as soon as `ClinicalPdfViewer` is
/// constructed, regardless of what its own `load` callback does. Skipped
/// only on that one host platform; every other test in this file (and the
/// real app, on every real target platform) is unaffected.
bool get _hostLacksPdfSupport => Platform.isLinux;

void main() {
  group('Health Vault document viewing (Remediation E2)', () {
    testWidgets(
        'a PDF document routes to the in-app ClinicalPdfViewer, not an '
        'external launch',
        skip: _hostLacksPdfSupport, (tester) async {
      final repo = _FakeVaultRepository(
        [PatientDocumentDto.fromJson(_pdfDoc)],
        fetch: () async => TypedBytes(
            bytes: Uint8List.fromList(utf8.encode('%PDF-1.4')),
            contentType: 'application/pdf'),
      );
      await tester.pumpWidget(_harness(repo));
      await tester.pumpAndSettle();

      await tester.tap(find
          .ancestor(
              of: find.byType(PatientCard), matching: find.byType(InkWell))
          .first);
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalPdfViewer), findsOneWidget);
      expect(repo.fetchCount, 1);

      // No native pdfium backend is available in this harness; only "does
      // not crash" is verifiable here, not real on-device rendering.
      await tester.pump(const Duration(seconds: 1));
      expect(tester.takeException(), isNull);
    });

    testWidgets('an image document still uses the existing in-app image viewer',
        (tester) async {
      final repo =
          _FakeVaultRepository([PatientDocumentDto.fromJson(_imageDoc)]);
      await tester.pumpWidget(_harness(repo));
      await tester.pumpAndSettle();

      await tester.tap(find
          .ancestor(
              of: find.byType(PatientCard), matching: find.byType(InkWell))
          .first);
      await tester.pump();
      await tester.pump();

      // The existing image path never calls fetchDocumentBytes — it still
      // resolves a URL and uses Image.network directly, unchanged.
      expect(repo.fetchCount, 0);
      expect(find.byType(ClinicalPdfViewer), findsNothing);
      // The pre-existing _ImageViewerPage is private to health_vault_page.dart;
      // its presence is confirmed by the Scaffold it pushes having a black
      // background, matching its own implementation.
      expect(
          find.byWidgetPredicate(
              (w) => w is Scaffold && w.backgroundColor == Colors.black),
          findsOneWidget);
    });

    testWidgets(
        'an unsupported mime type shows a safe message, no external launch, '
        'no fetch', (tester) async {
      final repo =
          _FakeVaultRepository([PatientDocumentDto.fromJson(_unsupportedDoc)]);
      await tester.pumpWidget(_harness(repo));
      await tester.pumpAndSettle();

      await tester.tap(find
          .ancestor(
              of: find.byType(PatientCard), matching: find.byType(InkWell))
          .first);
      await tester.pump();

      expect(find.byType(ClinicalPdfViewer), findsNothing);
      expect(repo.fetchCount, 0,
          reason: 'an unsupported type must never reach the network fetch');
      expect(find.byType(SnackBar), findsOneWidget);
    });

    testWidgets(
        'a server content-type that disagrees with the declared PDF mime '
        'type is rejected rather than rendered',
        skip: _hostLacksPdfSupport, (tester) async {
      final repo = _FakeVaultRepository(
        [PatientDocumentDto.fromJson(_pdfDoc)],
        // Declared application/pdf, but the server actually served an image.
        fetch: () async => TypedBytes(
            bytes: Uint8List.fromList([0x89, 0x50, 0x4E, 0x47]),
            contentType: 'image/png'),
      );
      await tester.pumpWidget(_harness(repo));
      await tester.pumpAndSettle();

      await tester.tap(find
          .ancestor(
              of: find.byType(PatientCard), matching: find.byType(InkWell))
          .first);
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalPdfViewer), findsOneWidget);
      // The mismatch throws inside `load()`; pdfx's own document-error path
      // must handle it gracefully.
      await tester.pump(const Duration(seconds: 1));
      expect(tester.takeException(), isNull);
    });

    testWidgets('expired/revoked authentication on PDF fetch does not crash',
        skip: _hostLacksPdfSupport, (tester) async {
      final repo = _FakeVaultRepository(
        [PatientDocumentDto.fromJson(_pdfDoc)],
        fetch: () async => throw const ApiException(401, 'Session expired.'),
      );
      await tester.pumpWidget(_harness(repo));
      await tester.pumpAndSettle();

      await tester.tap(find
          .ancestor(
              of: find.byType(PatientCard), matching: find.byType(InkWell))
          .first);
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalPdfViewer), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(find.text('Unable to open this document.'), findsOneWidget);
    });
  });
}
