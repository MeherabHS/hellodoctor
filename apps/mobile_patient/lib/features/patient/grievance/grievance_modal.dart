import 'package:flutter/material.dart';

import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'grievance_repository.dart';

/// `prototype/index.html`'s `#patientGrievanceModal` — "Report Consultation
/// Issue", made real against `POST /grievances`. Deliberately omits the
/// prototype's "System Auto-Collected Telemetry" inspector panel: that
/// section is entirely hardcoded demo numbers (a fixed "2m 14s" duration, a
/// fixed audit timeline, a fixed device string) with no backing endpoint a
/// patient can call — showing fabricated telemetry here would be a worse
/// violation of CLAUDE.md's "No Phantom Completions" rule than omitting it.
/// Real telemetry is captured server-side (see `agora_placeholder_page.dart`)
/// for Admin's own adjudication review, which this app has no corresponding
/// patient-facing read for.
Future<void> showGrievanceModal(
  BuildContext context, {
  required String appointmentId,
  required String doctorName,
  required GrievanceRepository repo,
}) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    builder: (_) => _GrievanceModal(
      appointmentId: appointmentId,
      doctorName: doctorName,
      repo: repo,
    ),
  );
}

/// Short category keys sent as `category` (the backend caps it at 150
/// characters) — always English, a stable data value, not UI chrome, so it
/// is never localized. [_categoryDisplay] resolves the matching localized
/// display label shown in the dropdown.
const _categoryKeys = <String>[
  'Rushed Consultation / Ended Abruptly',
  'Refused e-Prescription',
  'Unprofessional Conduct',
  'Clinical Dispute / Wrong Medication',
  'Doctor Was Absent / Late Join',
];

String _categoryDisplay(AppLocalizations l10n, String key) => switch (key) {
      'Rushed Consultation / Ended Abruptly' => l10n.grievanceCategoryRushed,
      'Refused e-Prescription' => l10n.grievanceCategoryRefusedRx,
      'Unprofessional Conduct' => l10n.grievanceCategoryUnprofessional,
      'Clinical Dispute / Wrong Medication' =>
        l10n.grievanceCategoryClinicalDispute,
      'Doctor Was Absent / Late Join' => l10n.grievanceCategoryAbsentDoctor,
      _ => key,
    };

class _GrievanceModal extends StatefulWidget {
  const _GrievanceModal({
    required this.appointmentId,
    required this.doctorName,
    required this.repo,
  });
  final String appointmentId, doctorName;
  final GrievanceRepository repo;

  @override
  State<_GrievanceModal> createState() => _GrievanceModalState();
}

class _GrievanceModalState extends State<_GrievanceModal> {
  String _target = 'DOCTOR';
  String _category = _categoryKeys.first;
  final _summaryController = TextEditingController();
  bool _submitting = false;
  String? _error;
  Map<String, dynamic>? _result;

  @override
  void dispose() {
    _summaryController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final summary = _summaryController.text.trim();
    if (summary.isEmpty) {
      setState(() =>
          _error = AppLocalizations.of(context).grievanceDescribeWhatHappened);
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final consultationId =
          await widget.repo.resolveConsultationId(widget.appointmentId);
      final result = await widget.repo.submit(
        consultationId: consultationId,
        target: _target,
        category: _category,
        claimSummary: summary,
      );
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _result = result;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = e.status == 404
            ? AppLocalizations.of(context).grievanceNoSession
            : e.userMessage;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = AppLocalizations.of(context).grievanceSubmitError;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final result = _result;
    return Padding(
      padding: EdgeInsets.only(
          left: 18,
          right: 18,
          top: 16,
          bottom: MediaQuery.of(context).viewInsets.bottom + 16),
      child: ConstrainedBox(
        constraints:
            BoxConstraints(maxHeight: MediaQuery.of(context).size.height * 0.9),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: result != null ? _successContent(result) : _formContent(),
          ),
        ),
      ),
    );
  }

  List<Widget> _successContent(Map<String, dynamic> result) {
    final l10n = AppLocalizations.of(context);
    return [
      const Icon(Icons.check_circle, color: PatientTokens.primary, size: 40),
      const SizedBox(height: 10),
      Text(l10n.grievanceReportSubmitted,
          style: const TextStyle(
              fontSize: 17,
              fontWeight: FontWeight.w800,
              color: PatientTokens.text)),
      const SizedBox(height: 6),
      Text(l10n.grievanceReference('${result['grievance_number']}'),
          style: const TextStyle(fontSize: 12, color: PatientTokens.textSub)),
      const SizedBox(height: 10),
      Text(l10n.grievanceSuccessNotice,
          style: const TextStyle(
              fontSize: 12, color: PatientTokens.textSub, height: 1.45)),
      const SizedBox(height: 16),
      SizedBox(
        width: double.infinity,
        child: FilledButton(
          onPressed: () => Navigator.of(context).pop(),
          style: FilledButton.styleFrom(backgroundColor: PatientTokens.primary),
          child: Text(l10n.doneButton),
        ),
      ),
    ];
  }

  List<Widget> _formContent() {
    final l10n = AppLocalizations.of(context);
    return [
      Row(children: [
        Container(
          width: 34,
          height: 34,
          decoration: BoxDecoration(
              color: const Color(0xFFFEE2E2),
              borderRadius: BorderRadius.circular(10)),
          alignment: Alignment.center,
          child: const Text('⚖️', style: TextStyle(fontSize: 16)),
        ),
        const SizedBox(width: 10),
        Expanded(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(l10n.grievanceTitle,
                style: const TextStyle(
                    fontSize: 14.5,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
            Text(l10n.grievanceAuditSubtitle,
                style: const TextStyle(
                    fontSize: 10.5, color: PatientTokens.textSub)),
          ]),
        ),
        IconButton(
            onPressed: () => Navigator.of(context).maybePop(),
            icon: const Icon(Icons.close, size: 18)),
      ]),
      const SizedBox(height: 10),
      Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
            color: PatientTokens.surfaceAlt,
            borderRadius: BorderRadius.circular(12)),
        child: Text(widget.doctorName,
            style: const TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w800,
                color: PatientTokens.text)),
      ),
      const SizedBox(height: 14),
      Text(l10n.grievanceTargetQuestion,
          style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              color: PatientTokens.textSub)),
      const SizedBox(height: 6),
      Row(children: [
        Expanded(
            child: _TargetOption(
                label: l10n.grievanceAgainstDoctor,
                description: l10n.grievanceAgainstDoctorDesc,
                selected: _target == 'DOCTOR',
                onTap: () => setState(() => _target = 'DOCTOR'))),
        const SizedBox(width: 8),
        Expanded(
            child: _TargetOption(
                label: l10n.grievanceAgainstSystem,
                description: l10n.grievanceAgainstSystemDesc,
                selected: _target == 'SYSTEM',
                onTap: () => setState(() => _target = 'SYSTEM'))),
      ]),
      const SizedBox(height: 14),
      Text(l10n.grievanceCategoryLabel,
          style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              color: PatientTokens.textSub)),
      const SizedBox(height: 6),
      Container(
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
            border: Border.all(color: PatientTokens.border),
            borderRadius: BorderRadius.circular(10)),
        child: DropdownButtonHideUnderline(
          child: DropdownButton<String>(
            isExpanded: true,
            value: _category,
            items: [
              for (final key in _categoryKeys)
                DropdownMenuItem(
                    value: key,
                    child: Text(_categoryDisplay(l10n, key),
                        style: const TextStyle(fontSize: 11.5))),
            ],
            onChanged: (v) => setState(() => _category = v ?? _category),
          ),
        ),
      ),
      const SizedBox(height: 14),
      Text(l10n.grievanceExplainLabel,
          style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              color: PatientTokens.textSub)),
      const SizedBox(height: 6),
      TextField(
        controller: _summaryController,
        maxLines: 3,
        maxLength: 1000,
        decoration: InputDecoration(hintText: l10n.grievanceExplainHint),
      ),
      Container(
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
            color: PatientTokens.surfaceAlt,
            borderRadius: BorderRadius.circular(10)),
        child: Text(l10n.grievanceDisclaimer,
            style: const TextStyle(
                fontSize: 10, color: PatientTokens.textSub, height: 1.4)),
      ),
      if (_error != null) ...[
        const SizedBox(height: 8),
        InlineError(_error!),
      ],
      const SizedBox(height: 14),
      SizedBox(
        width: double.infinity,
        child: FilledButton(
          onPressed: _submitting ? null : _submit,
          style: FilledButton.styleFrom(
              backgroundColor: PatientTokens.danger,
              padding: const EdgeInsets.symmetric(vertical: 13)),
          child: _submitting
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(
                      strokeWidth: 2, color: Colors.white))
              : Text(l10n.grievanceSubmitButton,
                  style: const TextStyle(fontWeight: FontWeight.w800)),
        ),
      ),
      const SizedBox(height: 6),
      Center(
        child: TextButton(
          onPressed: () => Navigator.of(context).maybePop(),
          child: Text(l10n.grievanceCancelReturn,
              style:
                  const TextStyle(fontSize: 11, color: PatientTokens.textSub)),
        ),
      ),
    ];
  }
}

class _TargetOption extends StatelessWidget {
  const _TargetOption({
    required this.label,
    required this.description,
    required this.selected,
    required this.onTap,
  });
  final String label, description;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(10),
        child: Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color:
                selected ? const Color(0xFFFEF2F2) : PatientTokens.surfaceAlt,
            borderRadius: BorderRadius.circular(10),
            border: Border.all(
                color: selected ? PatientTokens.danger : PatientTokens.border),
          ),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(label,
                style: TextStyle(
                    fontSize: 11.5,
                    fontWeight: FontWeight.w800,
                    color:
                        selected ? PatientTokens.danger : PatientTokens.text)),
            const SizedBox(height: 2),
            Text(description,
                style:
                    const TextStyle(fontSize: 9, color: PatientTokens.textSub)),
          ]),
        ),
      );
}
