import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/legal/legal_links.dart';
import '../../core/network/api_client.dart';
import '../../core/theme/tokens.dart';
import '../../app.dart';
import 'auth_cubit.dart';

/// Everything already collected on the registration form, carried forward to
/// this screen so the account is only actually created once every current
/// required document has been acknowledged here.
class DoctorRegistrationDraft {
  const DoctorRegistrationDraft({
    required this.fullName,
    required this.licenseNumber,
    required this.phoneNumber,
    required this.email,
    required this.password,
    required this.specialty,
    required this.experienceYears,
    required this.hospital,
    required this.qualifications,
    required this.address,
  });
  final String fullName;
  final String licenseNumber;
  final String phoneNumber;
  final String email;
  final String password;
  final String specialty;
  final String experienceYears;
  final String hospital;
  final String qualifications;
  final String address;
}

/// The exact document_type values a Doctor registration applies to.
/// GET /legal/documents is a single shared, role-agnostic registry (no
/// audience filter exists server-side), so it also returns Patient-only
/// document types that must never reach a Doctor registration screen.
/// An explicit allow-list, not a Patient-type exclude-list, so a future new
/// Patient-only document type is excluded by default rather than requiring
/// this file to be remembered and updated -- mirrors
/// apps/doctor-web/src/lib/legalConsent.ts::DOCTOR_APPLICABLE_DOCUMENT_TYPES.
/// Previously only excluded 'PATIENT_DATA_SHARING_CONSENT' by name, silently
/// still showing the other three Patient-only types
/// (PATIENT_TERMS/PATIENT_PRIVACY_NOTICE/PATIENT_DATA_RIGHTS) on this screen.
const _doctorApplicableDocumentTypes = {
  'DOCTOR_TERMS',
  'PRIVACY_NOTICE',
  'PROFESSIONAL_VERIFICATION_DECLARATION',
  'TELEMEDICINE_PRACTITIONER_POLICY',
  'MARKETING_COMMUNICATIONS',
};

/// Extracted as a top-level, Flutter-independent pure function so it is
/// directly unit-testable without driving this page's API client/widget
/// tree -- see test/legal_consent_filter_test.dart.
bool isDoctorApplicableDocumentType(String documentType) =>
    _doctorApplicableDocumentTypes.contains(documentType);

class _LegalDocument {
  _LegalDocument({
    required this.type,
    required this.version,
    required this.title,
    required this.body,
    required this.required,
  });
  final String type;
  final String version;
  final String title;
  final String body;
  final bool required;
}

/// Fetches the current set of legal/professional documents from
/// `GET /legal/documents` (the server's registry — never duplicated as
/// static text here) and requires every required one to be checked before
/// the Doctor account can actually be created.
class LegalConsentPage extends StatefulWidget {
  const LegalConsentPage({super.key, required this.auth, required this.draft});
  final AuthCubit auth;
  final DoctorRegistrationDraft draft;
  @override
  State<LegalConsentPage> createState() => _LegalConsentState();
}

class _LegalConsentState extends State<LegalConsentPage> {
  List<_LegalDocument>? _documents;
  String? _loadError;
  final Set<String> _checked = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final response = await widget.auth.api.get('/legal/documents');
      final documents = (response.data['data'] as List)
          .map((d) => _LegalDocument(
                type: d['document_type'] as String,
                version: d['version'] as String,
                title: d['title'] as String,
                body: d['body'] as String,
                required: d['required'] as bool,
              ))
          .toList();
      documents.removeWhere(
          (document) => !isDoctorApplicableDocumentType(document.type));
      if (mounted) setState(() => _documents = documents);
    } on ApiException catch (e) {
      if (mounted) setState(() => _loadError = e.userMessage);
    } catch (_) {
      if (mounted) {
        setState(() =>
            _loadError = 'Unable to load the legal agreements. Please retry.');
      }
    }
  }

  bool get _allRequiredChecked {
    final documents = _documents;
    if (documents == null) return false;
    return documents
        .where((d) => d.required)
        .every((d) => _checked.contains(d.type));
  }

  void _openDocument(_LegalDocument doc) => Navigator.push(
      context,
      MaterialPageRoute(
          builder: (_) => Scaffold(
              appBar: AppBar(title: Text(doc.title)),
              body: SingleChildScrollView(
                  padding: const EdgeInsets.all(20), child: Text(doc.body)))));

  Widget _checkboxRow(_LegalDocument doc) => Container(
      margin: const EdgeInsets.only(bottom: 10),
      decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: DoctorTokens.fieldBorder)),
      child: CheckboxListTile(
          value: _checked.contains(doc.type),
          activeColor: DoctorTokens.primary,
          onChanged: (value) => setState(() {
                if (value ?? false) {
                  _checked.add(doc.type);
                } else {
                  _checked.remove(doc.type);
                }
              }),
          controlAffinity: ListTileControlAffinity.leading,
          contentPadding: const EdgeInsets.fromLTRB(4, 2, 12, 0),
          title: Text(doc.title,
              style: const TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w700,
                  color: DoctorTokens.navy)),
          subtitle:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            // The full public version on afyamkononi.app.
            LegalWebLinks(
                pages: legalWebPagesFor(doc.type), color: DoctorTokens.primary),
            TextButton(
                onPressed: () => _openDocument(doc),
                style: TextButton.styleFrom(padding: EdgeInsets.zero),
                child: const Text('View →')),
          ])));

  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.screen,
      appBar: AppBar(title: const Text('Agreements')),
      body: BlocBuilder<AuthCubit, AuthState>(
          bloc: widget.auth,
          builder: (_, state) {
            if (state is RegistrationSubmitted) {
              return RegistrationSubmittedView(
                  onDone: () =>
                      Navigator.popUntil(context, (route) => route.isFirst));
            }
            if (_loadError != null) {
              return Center(
                  child: Padding(
                      padding: const EdgeInsets.all(24),
                      child: Column(mainAxisSize: MainAxisSize.min, children: [
                        Text(_loadError!,
                            style: const TextStyle(color: DoctorTokens.danger)),
                        const SizedBox(height: 12),
                        FilledButton(
                            onPressed: () {
                              setState(() => _loadError = null);
                              _load();
                            },
                            child: const Text('Retry'))
                      ])));
            }
            final documents = _documents;
            if (documents == null) {
              return const Center(child: CircularProgressIndicator());
            }
            final required = documents.where((d) => d.required).toList();
            final optional = documents.where((d) => !d.required).toList();
            final busy = state is AuthBusy;
            return ListView(padding: const EdgeInsets.all(24), children: [
              const Text('Legal & Professional Agreements',
                  style: TextStyle(
                      fontSize: 21,
                      fontWeight: FontWeight.w800,
                      color: DoctorTokens.navy)),
              const SizedBox(height: 6),
              const Text(
                  'Before submitting your application, review and accept the following requirements.',
                  style: TextStyle(
                      fontSize: 15, height: 1.4, color: DoctorTokens.slate)),
              const SizedBox(height: 18),
              for (final doc in required) _checkboxRow(doc),
              if (optional.isNotEmpty) ...[
                const Padding(
                    padding: EdgeInsets.symmetric(vertical: 12),
                    child: Divider()),
                const Text('Optional',
                    style: TextStyle(
                        color: DoctorTokens.textSub,
                        fontWeight: FontWeight.w700)),
                for (final doc in optional) _checkboxRow(doc),
              ],
              const SizedBox(height: 8),
              FilledButton(
                  onPressed: (busy || !_allRequiredChecked)
                      ? null
                      : () {
                          final accepted = documents
                              .where((d) => _checked.contains(d.type))
                              .map(
                                  (d) => {'type': d.type, 'version': d.version})
                              .toList();
                          widget.auth.registerDoctor(
                            fullName: widget.draft.fullName,
                            licenseNumber: widget.draft.licenseNumber,
                            phoneNumber: widget.draft.phoneNumber,
                            email: widget.draft.email,
                            password: widget.draft.password,
                            specialty: widget.draft.specialty,
                            experienceYears: widget.draft.experienceYears,
                            hospital: widget.draft.hospital,
                            qualifications: widget.draft.qualifications,
                            address: widget.draft.address,
                            acceptedDocuments: accepted,
                          );
                        },
                  child: busy
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Submit application')),
              if (state is AuthFailure)
                Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: Text(state.message,
                        style: const TextStyle(color: DoctorTokens.danger)))
            ]);
          }));
}
