import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:pdfx/pdfx.dart';

/// Remediation E2: in-app clinical PDF viewer — opens a clinical
/// attachment/prescription PDF without ever handing its bytes to an
/// external application via `launchUrl`.
///
/// `pdfx` (verified directly against its published source,
/// https://github.com/ScerIO/packages.flutter, package version 2.11.0) opens
/// a document straight from an in-memory `Uint8List` via
/// `PdfDocument.openData` and renders each page to an in-memory raster image
/// via pdfium over a platform channel (`PdfPage.render`) — neither call
/// writes a temporary file to disk; the rendered page image is just another
/// `ImageProvider` held in Flutter's own in-memory image cache for this
/// route's lifetime, the same category of in-memory-only caching already
/// accepted for `ClinicalImageViewer` and `Image.network`. The resolved
/// native document handle is explicitly closed in [dispose] (and before
/// retrying), since `PdfController.dispose()` alone does not release it.
class ClinicalPdfViewer extends StatefulWidget {
  const ClinicalPdfViewer({super.key, required this.load});

  /// Mints a fresh, single-use download grant and returns the document
  /// bytes. Called once on open and again on every retry — never reused,
  /// since the underlying grant is one-time.
  final Future<Uint8List> Function() load;

  @override
  State<ClinicalPdfViewer> createState() => _ClinicalPdfViewerState();
}

class _ClinicalPdfViewerState extends State<ClinicalPdfViewer> {
  late PdfController _controller;
  PdfDocument? _openDocument;
  String? _documentError;

  @override
  void initState() {
    super.initState();
    _controller = _newController();
  }

  PdfController _newController() {
    final documentFuture = PdfDocument.openData(widget.load());
    // Fire-and-forget capture of the resolved document purely so dispose()
    // can close it; a failed open leaves this null and there is nothing to
    // close. The `catchError` here only prevents an "unhandled future"
    // warning — the real error still surfaces to the UI via
    // PdfView.onDocumentError.
    documentFuture.then((doc) {
      _openDocument = doc;
    }).catchError((_) {});
    return PdfController(document: documentFuture);
  }

  void _retry() {
    _openDocument?.close();
    _openDocument = null;
    _controller.dispose();
    setState(() {
      _documentError = null;
      _controller = _newController();
    });
  }

  @override
  void dispose() {
    _openDocument?.close();
    _controller.dispose();
    super.dispose();
  }

  Widget _errorView(String message) => Center(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
        Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: Text(message, textAlign: TextAlign.center)),
        const SizedBox(height: 8),
        TextButton(onPressed: _retry, child: const Text('Retry'))
      ]));

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Clinical document')),
      body: SafeArea(
          child: _documentError != null
              ? _errorView(_documentError!)
              : PdfView(
                  key: ValueKey(_controller),
                  controller: _controller,
                  onDocumentError: (_) {
                    if (!mounted) return;
                    setState(
                        () => _documentError = 'Unable to open this document.');
                  },
                  builders: PdfViewBuilders<DefaultBuilderOptions>(
                    options: const DefaultBuilderOptions(),
                    documentLoaderBuilder: (_) =>
                        const Center(child: CircularProgressIndicator()),
                    pageLoaderBuilder: (_) =>
                        const Center(child: CircularProgressIndicator()),
                    // Never surface the raw exception text (pdfx's own
                    // default does) — matches this app's existing discipline
                    // of only ever showing safe, generic error copy.
                    errorBuilder: (_, __) =>
                        _errorView('Unable to open this document.'),
                  ),
                )));
}
