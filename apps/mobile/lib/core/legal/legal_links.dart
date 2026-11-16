import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

/// Public website holding the full legal documents.
const afyaMkononiWebsite = 'https://afyamkononi.app';

/// A public page of the AfyaMkononi website.
class LegalWebPage {
  const LegalWebPage(this.title, this.path);
  final String title;

  /// Path on [afyaMkononiWebsite], e.g. `/privacy`.
  final String path;

  /// Shown to the reader as `afyamkononi.app/privacy`.
  String get displayUrl => 'afyamkononi.app$path';
  Uri get uri => Uri.parse('$afyaMkononiWebsite$path');
}

const _terms = LegalWebPage('Terms of Service', '/terms');
const _privacy = LegalWebPage('Privacy Policy', '/privacy');
const _retention = LegalWebPage('Data Retention Policy', '/data-retention');
const _consent = LegalWebPage('Patient Consent', '/patient-consent');
const _deletion = LegalWebPage('Account Deletion', '/account-deletion');
const _doctorTerms = LegalWebPage('Doctor Terms', '/doctor-terms');
const _doctorDeclaration =
    LegalWebPage('Doctor Terms', '/doctor-terms#professional-declaration');
const _doctorConfidentiality =
    LegalWebPage('Doctor Terms', '/doctor-terms#confidentiality');
const _peerPrivacy =
    LegalWebPage('Peer Navigator Privacy Notice', '/peer-navigator-privacy');

/// The website pages matching a legal document from `GET /legal/documents`.
/// The ticked document is still the versioned text the server records; these
/// are its full public versions. Empty for documents that exist only in the
/// app (Peer Terms, Peer Conduct & Safeguarding, marketing preferences).
List<LegalWebPage> legalWebPagesFor(String documentType) =>
    switch (documentType) {
      'PATIENT_TERMS' => const [_terms],
      'PATIENT_PRIVACY_NOTICE' => const [_privacy, _retention],
      'PATIENT_DATA_SHARING_CONSENT' => const [_consent],
      'PATIENT_DATA_RIGHTS' => const [_deletion, _retention],
      'DOCTOR_TERMS' => const [_doctorTerms],
      'PRIVACY_NOTICE' => const [_privacy, _retention],
      'PROFESSIONAL_VERIFICATION_DECLARATION' => const [_doctorDeclaration],
      'TELEMEDICINE_PRACTITIONER_POLICY' => const [_doctorConfidentiality],
      'PEER_PRIVACY_NOTICE' => const [_peerPrivacy, _retention],
      _ => const [],
    };

/// Opens [page] in the phone's browser. A failure leaves the form usable.
Future<bool> openLegalWebPage(LegalWebPage page) async {
  try {
    return await launchUrl(page.uri, mode: LaunchMode.externalApplication);
  } catch (_) {
    return false;
  }
}

/// Tappable `afyamkononi.app/...` links under a consent checkbox.
class LegalWebLinks extends StatelessWidget {
  const LegalWebLinks(
      {super.key, required this.pages, required this.color, this.onOpen});
  final List<LegalWebPage> pages;
  final Color color;

  /// Replaces the browser launch (tests).
  final void Function(LegalWebPage page)? onOpen;

  @override
  Widget build(BuildContext context) {
    if (pages.isEmpty) return const SizedBox.shrink();
    return Wrap(spacing: 12, runSpacing: 2, children: [
      for (final page in pages)
        Semantics(
          link: true,
          label: '${page.title}, ${page.displayUrl}',
          child: InkWell(
            onTap: () =>
                onOpen != null ? onOpen!(page) : openLegalWebPage(page),
            borderRadius: BorderRadius.circular(6),
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 6),
              child: Row(mainAxisSize: MainAxisSize.min, children: [
                Icon(Icons.open_in_new_rounded, size: 14, color: color),
                const SizedBox(width: 4),
                Flexible(
                  child: Text(page.displayUrl,
                      style: TextStyle(
                          fontSize: 12.5,
                          fontWeight: FontWeight.w700,
                          color: color,
                          decoration: TextDecoration.underline,
                          decorationColor: color)),
                ),
              ]),
            ),
          ),
        ),
    ]);
  }
}
