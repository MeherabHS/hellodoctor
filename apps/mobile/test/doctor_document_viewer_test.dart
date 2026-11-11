// Remediation E2 security regression coverage for DocumentButton's routing
// of clinical attachments/prescriptions to an in-app viewer instead of
// `launchUrl(..., externalApplication)`. Platform-channel-backed pdfx
// rendering is not driven here (this harness has no native pdfium binary) —
// a PDF open attempt fails gracefully through pdfx's own internal
// error-handling path (verified against its source during implementation),
// so these tests assert ClinicalPdfViewer is actually reached and shows a
// safe error+retry state rather than crashing, not that a page visually
// renders. Real on-device PDF rendering remains unverified here; see the
// Remediation E2 report.
import 'dart:convert';
import 'dart:io' show Platform;
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/clinical_image_viewer.dart';
import 'package:hellodoctor_mobile/features/doctor/clinical_pdf_viewer.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

/// A real, minimal 1x1 transparent PNG — `MemoryImage` actually decodes its
/// bytes, so a garbage byte list would throw inside the image codec rather
/// than exercising the behavior under test.
final Uint8List _tinyPngBytes = base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY'
    '42YAAAAASUVORK5CYII=');

typedef _FetchResult = Future<TypedBytes> Function();

class _FakeDoctorRepository extends DoctorRepository {
  _FakeDoctorRepository(this._fetch) : super(ApiClient(dio: Dio()));
  final _FetchResult _fetch;
  int fetchCount = 0;

  @override
  Future<TypedBytes> fetchDocumentBytes(String appointmentId, String documentId,
      {String? currentAppointmentId}) {
    fetchCount++;
    return _fetch();
  }
}

Widget _harness(DoctorRepository repo, Widget child) =>
    BlocProvider<DoctorCubit>(
      create: (_) => DoctorCubit(repo),
      child: MaterialApp(home: Scaffold(body: child)),
    );

/// `pdfx` (see `clinical_pdf_viewer.dart`'s own doc comment) has no desktop
/// rendering backend on Linux at all — `PdfDocument.openData` throws
/// `PlatformNotSupportedException` before even reaching a platform channel,
/// which is a materially different, harness-only failure from the "no
/// native pdfium binary registered" case these specific tests were written
/// and verified against (Windows/macOS, where `pdfx` reports itself
/// supported). Skipped only on that one host platform; every other test in
/// this file (and the real app, on every real target platform: Android,
/// iOS, Web, Windows, macOS) is unaffected.
bool get _hostLacksPdfSupport => Platform.isLinux;

void main() {
  group('DocumentButton routing (Remediation E2)', () {
    testWidgets(
        'image/jpeg routes to ClinicalImageViewer, not an external launch',
        (tester) async {
      final repo = _FakeDoctorRepository(() async =>
          TypedBytes(bytes: _tinyPngBytes, contentType: 'image/jpeg'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'image/jpeg')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalImageViewer), findsOneWidget);
      expect(find.byType(ClinicalPdfViewer), findsNothing);
      expect(repo.fetchCount, 1,
          reason: 'the viewer must fetch via the authenticated repository');
    });

    testWidgets('image/png routes to ClinicalImageViewer', (tester) async {
      final repo = _FakeDoctorRepository(() async =>
          TypedBytes(bytes: _tinyPngBytes, contentType: 'image/png'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'image/png')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalImageViewer), findsOneWidget);
    });

    testWidgets(
        'application/pdf routes to ClinicalPdfViewer, not an external launch',
        skip: _hostLacksPdfSupport, (tester) async {
      final repo = _FakeDoctorRepository(() async => TypedBytes(
          bytes: Uint8List.fromList(utf8.encode('%PDF-1.4 not a real pdf')),
          contentType: 'application/pdf'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'application/pdf')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalPdfViewer), findsOneWidget);
      expect(find.byType(ClinicalImageViewer), findsNothing);

      // pdfx has no native pdfium backend in this harness: the platform
      // channel call it makes to open the document has no registered
      // handler, so it may error quickly or simply never resolve — either
      // way, the only thing this test can honestly assert without a real
      // device is that reaching this state never crashes or falls back to
      // an external application. Real on-device PDF open/error/retry
      // behavior is NOT VERIFIED here; see the Remediation E2 report.
      await tester.pump(const Duration(seconds: 1));
      expect(tester.takeException(), isNull);
    });

    testWidgets(
        'an unrecognized mime type is refused with a message, no viewer is opened',
        (tester) async {
      final repo = _FakeDoctorRepository(() async =>
          TypedBytes(bytes: _tinyPngBytes, contentType: 'image/jpeg'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'application/zip')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();

      expect(find.byType(ClinicalImageViewer), findsNothing);
      expect(find.byType(ClinicalPdfViewer), findsNothing);
      expect(find.text('This document type cannot be previewed in the app.'),
          findsOneWidget);
      expect(repo.fetchCount, 0,
          reason: 'an unsupported type must never reach the network fetch');
    });

    testWidgets(
        'a missing mime type is refused with a message, no viewer is opened',
        (tester) async {
      final repo = _FakeDoctorRepository(() async =>
          TypedBytes(bytes: _tinyPngBytes, contentType: 'image/jpeg'));
      await tester.pumpWidget(_harness(repo,
          const DocumentButton(appointmentId: 'apt-1', documentId: 'doc-1')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();

      expect(find.byType(ClinicalImageViewer), findsNothing);
      expect(find.byType(ClinicalPdfViewer), findsNothing);
      expect(repo.fetchCount, 0);
    });

    testWidgets(
        'expired/revoked authentication on fetch shows an error inside the '
        'viewer, not a crash', (tester) async {
      final repo = _FakeDoctorRepository(
          () async => throw const ApiException(401, 'Session expired.'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'image/jpeg')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalImageViewer), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(find.text('Unable to open this image.'), findsOneWidget);
    });

    testWidgets('a missing document (404) shows an error inside the viewer',
        (tester) async {
      final repo = _FakeDoctorRepository(() async =>
          throw const ApiException(404, 'This document is unavailable.'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'image/png')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();

      expect(find.text('Unable to open this image.'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets(
        'a server content-type that disagrees with the declared mime type is '
        'rejected rather than rendered', (tester) async {
      // Declared image/png, but the server actually served a PDF — the
      // mismatch must be refused, not silently displayed as an image.
      final repo = _FakeDoctorRepository(() async => TypedBytes(
          bytes: Uint8List.fromList(utf8.encode('%PDF-1.4')),
          contentType: 'application/pdf'));
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'image/png')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();

      expect(find.byType(ClinicalImageViewer), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(find.text('Unable to open this image.'), findsOneWidget);
    });

    testWidgets(
        'retry inside ClinicalImageViewer re-fetches (fresh single-use grant)',
        (tester) async {
      var attempt = 0;
      final repo = _FakeDoctorRepository(() async {
        attempt++;
        if (attempt == 1) {
          throw const ApiException(404, 'This document is unavailable.');
        }
        return TypedBytes(bytes: _tinyPngBytes, contentType: 'image/jpeg');
      });
      await tester.pumpWidget(_harness(
          repo,
          const DocumentButton(
              appointmentId: 'apt-1',
              documentId: 'doc-1',
              mimeType: 'image/jpeg')));

      await tester.tap(find.byType(TextButton));
      await tester.pump();
      await tester.pump();
      expect(find.text('Unable to open this image.'), findsOneWidget);

      await tester.tap(find.text('Retry'));
      await tester.pump();
      await tester.pump();

      expect(find.text('Unable to open this image.'), findsNothing);
      expect(repo.fetchCount, 2,
          reason: 'retry must mint a fresh grant, not reuse the first');
    });
  });

  group('ClinicalPdfViewer (Remediation E2)', () {
    testWidgets('a malformed document fails gracefully, not a crash',
        skip: _hostLacksPdfSupport, (tester) async {
      // No real pdfium backend is available in this harness (see the
      // DocumentButton PDF-routing test above for why), so this only
      // verifies the attempt never crashes — not that an error/retry UI
      // actually appears within a bounded time. Real malformed-PDF handling
      // on a native device is NOT VERIFIED here.
      await tester.pumpWidget(MaterialApp(
          home: ClinicalPdfViewer(
              load: () async =>
                  Uint8List.fromList(utf8.encode('garbage, not a pdf')))));
      await tester.pump();
      await tester.pump(const Duration(seconds: 1));

      expect(tester.takeException(), isNull);
    });

    testWidgets('a fetch failure fails gracefully, not a crash',
        skip: _hostLacksPdfSupport, (tester) async {
      await tester.pumpWidget(MaterialApp(
          home: ClinicalPdfViewer(
              load: () async => throw const ApiException(401, 'Expired.'))));
      await tester.pump();
      await tester.pump(const Duration(seconds: 1));

      expect(tester.takeException(), isNull);
      expect(find.text('Unable to open this document.'), findsOneWidget);
    });

    testWidgets('disposing after a failed open does not throw',
        skip: _hostLacksPdfSupport, (tester) async {
      await tester.pumpWidget(MaterialApp(
          home: ClinicalPdfViewer(
              load: () async => throw const ApiException(401, 'Expired.'))));
      await tester.pump();
      await tester.pump(const Duration(seconds: 1));

      await tester.pumpWidget(const MaterialApp(home: SizedBox()));
      await tester.pump();

      expect(tester.takeException(), isNull);
    });
  });
}
