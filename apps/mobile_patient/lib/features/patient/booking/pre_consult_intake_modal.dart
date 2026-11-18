import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';

import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/validation/upload_validation.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'booking_models.dart';
import 'booking_repository.dart';

/// `prototype/index.html`'s `#preConsultIntakeModal`. Copy is corrected from
/// the prototype's stale "Up to 5 Max" to the real, current
/// 1–20-photos-per-consultation business rule (`CLAUDE.md` §2.3,
/// `kPatientMaxImagesPerConsultation`) — plan §4's view-4/intake-modal row
/// calls this out explicitly as a fidelity fix, not a deviation. Server-side
/// per-image compression (600 KB ceiling) is the sole size authority; this
/// modal only enforces count and mime type client-side.
class PreConsultIntakeModal extends StatefulWidget {
  const PreConsultIntakeModal({
    super.key,
    required this.doctorName,
    required this.appointmentId,
    required this.repo,
  });
  final String doctorName, appointmentId;
  final BookingRepository repo;

  @override
  State<PreConsultIntakeModal> createState() => _PreConsultIntakeModalState();
}

class _PickedFile {
  _PickedFile(this.name, this.mimeType, this.bytes);
  final String name, mimeType;
  final Uint8List bytes;
}

/// Best-effort deletion of image_picker's own on-disk cache copy at [path].
/// CLAUDE.md requires prescription/lab PHI photos not be left in generic
/// disk caches; once an image_picker XFile's bytes are read into memory (or
/// picking is abandoned because the type is invalid), this app has no
/// further use for the plugin's temp file. Extracted as a standalone
/// function so it's directly unit-testable without driving image_picker's
/// platform channel. Failures are swallowed — cleanup must never surface to
/// the UI or block the pick flow. Also used by the consultation chat's
/// image attachment.
Future<void> deletePickedSourceFile(String path) async {
  try {
    final f = File(path);
    if (await f.exists()) await f.delete();
  } catch (_) {
    // best-effort only
  }
}

/// Best-effort cleanup after a file_picker pick attempt. Deliberately never
/// touches [path] directly with a raw delete — file_picker can in principle
/// hand back a path outside its own managed cache depending on
/// platform/options, so only the plugin's own `clearTemporaryFiles()` API
/// (injectable here for testing) is used, which only ever clears
/// file_picker's own managed temp files and never a user's external
/// original.
@visibleForTesting
Future<void> cleanupFilePickerTemp({
  Future<void> Function() clearFilePickerTemp = _clearFilePickerTempFiles,
}) async {
  try {
    await clearFilePickerTemp();
  } catch (_) {
    // best-effort only
  }
}

Future<void> _clearFilePickerTempFiles() async {
  await FilePicker.platform.clearTemporaryFiles();
}

class _PreConsultIntakeModalState extends State<PreConsultIntakeModal> {
  final List<_PickedFile> _files = [];
  bool _uploading = false;
  String? _error;
  bool _done = false;

  /// Pages already sent for this consultation. A failed load leaves this
  /// empty and editable, which is the pre-existing behaviour (the server
  /// still enforces the cap and the window on upload).
  List<IntakeUploadDto> _uploaded = const [];
  bool _editable = true;
  String? _removingId;

  int get _total => _uploaded.length + _files.length;
  bool get _hasRoom => _total < kPatientMaxImagesPerConsultation;

  @override
  void initState() {
    super.initState();
    _loadUploaded();
  }

  Future<void> _loadUploaded() async {
    try {
      final result = await widget.repo.intakeUploads(widget.appointmentId);
      if (!mounted) return;
      setState(() {
        _uploaded = result.documents;
        _editable = result.editable;
      });
    } catch (_) {
      // Non-blocking: the sheet still works as an upload-only form.
    }
  }

  Future<void> _removeUploaded(IntakeUploadDto doc) async {
    if (_removingId != null || _uploading) return;
    final l10n = AppLocalizations.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (c) => AlertDialog(
        title: Text(l10n.intakeRemoveUploadedTitle),
        content: Text(l10n.intakeRemoveUploadedBody),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(c).pop(false),
              child: Text(MaterialLocalizations.of(c).cancelButtonLabel)),
          TextButton(
              onPressed: () => Navigator.of(c).pop(true),
              child: Text(l10n.intakeRemoveConfirm,
                  style: const TextStyle(color: PatientTokens.danger))),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() {
      _removingId = doc.id;
      _error = null;
    });
    try {
      await widget.repo.removeIntake(widget.appointmentId, doc.id);
      if (!mounted) return;
      setState(
          () => _uploaded = _uploaded.where((d) => d.id != doc.id).toList());
    } catch (e) {
      if (!mounted) return;
      setState(() => _error = e is ApiException
          ? e.userMessage
          : AppLocalizations.of(context).intakeRemoveError);
      // A 409 means the window closed; refresh so the sheet locks.
      unawaited(_loadUploaded());
    } finally {
      if (mounted) setState(() => _removingId = null);
    }
  }

  String? _mimeFromPath(String path) {
    final lower = path.toLowerCase();
    if (lower.endsWith('.png')) return 'image/png';
    if (lower.endsWith('.jpg') || lower.endsWith('.jpeg')) return 'image/jpeg';
    if (lower.endsWith('.pdf')) return 'application/pdf';
    return null;
  }

  Future<void> _addXFile(XFile file) async {
    final mime = file.mimeType ?? _mimeFromPath(file.path);
    if (mime == null || !kAllowedIntakeMimeTypes.contains(mime)) {
      setState(
          () => _error = AppLocalizations.of(context).intakeInvalidFileType);
      unawaited(deletePickedSourceFile(file.path));
      return;
    }
    final bytes = await file.readAsBytes();
    // Bytes are now safely in memory; the plugin's on-disk cache copy is no
    // longer needed regardless of whether this file is accepted or rejected
    // for being over the per-consultation cap below.
    unawaited(deletePickedSourceFile(file.path));
    if (!mounted) return;
    setState(() {
      _error = null;
      if (_hasRoom) {
        _files.add(_PickedFile(file.name, mime, bytes));
      } else {
        _error = AppLocalizations.of(context)
            .intakeMaxFilesReached(kPatientMaxImagesPerConsultation);
      }
    });
  }

  Future<void> _pickGallery() async {
    final picked = await ImagePicker()
        .pickMultiImage(imageQuality: 85, maxWidth: 2000, maxHeight: 2000);
    for (final file in picked) {
      await _addXFile(file);
    }
  }

  Future<void> _pickCamera() async {
    final picked = await ImagePicker().pickImage(
        source: ImageSource.camera,
        imageQuality: 85,
        maxWidth: 2000,
        maxHeight: 2000);
    if (picked != null) await _addXFile(picked);
  }

  Future<void> _pickPdf() async {
    final result = await FilePicker.platform.pickFiles(
        type: FileType.custom,
        allowedExtensions: const ['pdf'],
        withData: true);
    final file = result?.files.single;
    // file_picker's own temp/cache copy (if any was created for this pick)
    // is never needed again once `bytes` has been read into memory here.
    unawaited(cleanupFilePickerTemp());
    if (file?.bytes == null) return;
    setState(() {
      if (_hasRoom) {
        _files.add(_PickedFile(file!.name, 'application/pdf', file.bytes!));
      } else {
        _error = AppLocalizations.of(context)
            .intakeMaxFilesReached(kPatientMaxImagesPerConsultation);
      }
    });
  }

  void _remove(int index) => setState(() => _files.removeAt(index));

  Future<void> _submit() async {
    if (_files.isEmpty || _uploading) return;
    setState(() {
      _uploading = true;
      _error = null;
    });
    try {
      final payload = _files
          .map((f) => encodeIntakeFile(mimeType: f.mimeType, bytes: f.bytes))
          .toList();
      await widget.repo.uploadIntake(widget.appointmentId, payload);
      if (!mounted) return;
      setState(() {
        _uploading = false;
        _done = true;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _uploading = false;
        _error = e is ApiException
            ? e.userMessage
            : AppLocalizations.of(context).intakeUploadError;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Padding(
      padding: EdgeInsets.only(
          left: 18,
          right: 18,
          top: 16,
          bottom: MediaQuery.of(context).viewInsets.bottom + 16),
      child: ConstrainedBox(
        constraints: BoxConstraints(
            maxHeight: MediaQuery.of(context).size.height * 0.88),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(children: [
              Expanded(
                child: Text(l10n.bookingAttachRecords,
                    style: const TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w800,
                        color: PatientTokens.text)),
              ),
              IconButton(
                  onPressed: () => Navigator.of(context).maybePop(),
                  icon: const Icon(Icons.close, size: 18)),
            ]),
            Text(l10n.intakeInstructions(_shortName(widget.doctorName)),
                style: const TextStyle(
                    fontSize: 11.5, color: PatientTokens.textSub, height: 1.4)),
            const SizedBox(height: 12),
            if (_done)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 20),
                child: Column(children: [
                  const Icon(Icons.check_circle,
                      color: PatientTokens.primary, size: 36),
                  const SizedBox(height: 8),
                  Text(l10n.intakeSuccessMessage,
                      style: const TextStyle(
                          fontWeight: FontWeight.w700,
                          color: PatientTokens.text)),
                ]),
              )
            else
              Flexible(
                child: SingleChildScrollView(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        InkWell(
                          onTap: _editable ? _pickGallery : null,
                          borderRadius:
                              BorderRadius.circular(PatientTokens.cardRadius),
                          child: Container(
                            width: double.infinity,
                            padding: const EdgeInsets.all(18),
                            decoration: BoxDecoration(
                              color: PatientTokens.surfaceAlt,
                              borderRadius: BorderRadius.circular(
                                  PatientTokens.cardRadius),
                              border: Border.all(
                                  color: PatientTokens.border,
                                  style: BorderStyle.solid),
                            ),
                            child: Column(children: [
                              const Icon(Icons.upload_file,
                                  color: PatientTokens.primaryDark, size: 26),
                              const SizedBox(height: 8),
                              Text(
                                  l10n.intakeUploadPrompt(
                                      kPatientMaxImagesPerConsultation),
                                  textAlign: TextAlign.center,
                                  style: const TextStyle(
                                      fontWeight: FontWeight.w700,
                                      color: PatientTokens.text,
                                      fontSize: 12.5)),
                            ]),
                          ),
                        ),
                        const SizedBox(height: 10),
                        Row(children: [
                          Expanded(
                            child: OutlinedButton.icon(
                              onPressed: _editable ? _pickCamera : null,
                              icon: const Icon(Icons.camera_alt_outlined,
                                  size: 16),
                              label: Text(l10n.cameraButton),
                            ),
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: OutlinedButton.icon(
                              onPressed: _editable ? _pickPdf : null,
                              icon: const Icon(Icons.picture_as_pdf_outlined,
                                  size: 16),
                              label: Text(l10n.pdfButton),
                            ),
                          ),
                        ]),
                        if (_uploaded.isNotEmpty) ...[
                          const SizedBox(height: 14),
                          Text(l10n.intakeUploadedPages,
                              style: const TextStyle(
                                  fontSize: 11,
                                  fontWeight: FontWeight.w800,
                                  color: PatientTokens.textSub)),
                          const SizedBox(height: 8),
                          for (final doc in _uploaded)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 6),
                              child: PatientCard(
                                padding: const EdgeInsets.symmetric(
                                    horizontal: 12, vertical: 8),
                                child: Row(children: [
                                  Icon(
                                      doc.isPdf
                                          ? Icons.picture_as_pdf
                                          : Icons.image_outlined,
                                      size: 18,
                                      color: PatientTokens.primaryDark),
                                  const SizedBox(width: 8),
                                  Expanded(
                                    child: Text(
                                        l10n.intakeUploadedPage(doc.pageNumber),
                                        style: const TextStyle(fontSize: 12)),
                                  ),
                                  if (_editable)
                                    _removingId == doc.id
                                        ? const SizedBox(
                                            width: 16,
                                            height: 16,
                                            child: CircularProgressIndicator(
                                                strokeWidth: 2))
                                        : IconButton(
                                            key: ValueKey(
                                                'intake-remove-${doc.id}'),
                                            tooltip: l10n.intakeRemoveConfirm,
                                            onPressed: () =>
                                                _removeUploaded(doc),
                                            icon: const Icon(
                                                Icons.delete_outline,
                                                size: 18,
                                                color: PatientTokens.danger),
                                            padding: EdgeInsets.zero,
                                            constraints:
                                                const BoxConstraints()),
                                ]),
                              ),
                            ),
                        ],
                        if (!_editable) ...[
                          const SizedBox(height: 8),
                          Text(l10n.intakeUploadsLocked,
                              style: const TextStyle(
                                  fontSize: 11.5,
                                  color: PatientTokens.textSub)),
                        ],
                        if (_files.isNotEmpty) ...[
                          const SizedBox(height: 14),
                          Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                Text(l10n.intakeAttachedPages,
                                    style: const TextStyle(
                                        fontSize: 11,
                                        fontWeight: FontWeight.w800,
                                        color: PatientTokens.textSub)),
                                StatusChip(l10n.intakeFileCountChip(
                                    _total, kPatientMaxImagesPerConsultation)),
                              ]),
                          const SizedBox(height: 8),
                          for (var i = 0; i < _files.length; i++)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 6),
                              child: PatientCard(
                                padding: const EdgeInsets.symmetric(
                                    horizontal: 12, vertical: 8),
                                child: Row(children: [
                                  Icon(
                                      _files[i].mimeType == 'application/pdf'
                                          ? Icons.picture_as_pdf
                                          : Icons.image_outlined,
                                      size: 18,
                                      color: PatientTokens.primaryDark),
                                  const SizedBox(width: 8),
                                  Expanded(
                                    child: Text(_files[i].name,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: const TextStyle(fontSize: 12)),
                                  ),
                                  IconButton(
                                      onPressed: () => _remove(i),
                                      icon: const Icon(Icons.close, size: 16),
                                      padding: EdgeInsets.zero,
                                      constraints: const BoxConstraints()),
                                ]),
                              ),
                            ),
                        ],
                        if (_error != null) ...[
                          const SizedBox(height: 8),
                          InlineError(_error!),
                        ],
                      ]),
                ),
              ),
            const SizedBox(height: 14),
            if (_done)
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: () => Navigator.of(context).maybePop(),
                  style: FilledButton.styleFrom(
                      backgroundColor: PatientTokens.primary),
                  child: Text(l10n.doneButton),
                ),
              )
            else
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: _editable &&
                          _files.isNotEmpty &&
                          isValidIntakeImageCount(_total) &&
                          !_uploading &&
                          _removingId == null
                      ? _submit
                      : null,
                  style: FilledButton.styleFrom(
                      backgroundColor: PatientTokens.primary),
                  child: _uploading
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: Colors.white))
                      : Text(l10n.intakeAttachContinue),
                ),
              ),
            if (!_done)
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Center(
                  child: TextButton(
                    onPressed: () => Navigator.of(context).maybePop(),
                    child: Text(l10n.intakeSkipPrompt,
                        style: const TextStyle(
                            fontSize: 11, color: PatientTokens.textSub)),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }

  String _shortName(String fullName) {
    final parts = fullName.replaceAll('Dr.', '').trim().split(RegExp(r'\s+'));
    return parts.isEmpty ? fullName : parts.first;
  }
}
