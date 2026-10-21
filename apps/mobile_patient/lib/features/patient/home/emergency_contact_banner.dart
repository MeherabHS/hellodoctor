import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../core/config/emergency_contact.dart';
import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';

/// `prototype/index.html`'s `#patientEmergencyHelplineStrip` (view-0).
///
/// CLAUDE.md business rule, directly confirmed against the prototype's
/// rendered text (not just its HTML comment, which says "24/7..." as
/// internal shorthand only): the visible label must read exactly
/// "Emergency Contact" — never "24/7" — and this widget must be placed
/// directly above the Upcoming Consultation section on the Home screen.
/// Both of those are enforced by this widget's own regression test
/// (`emergency_contact_banner_golden_test.dart`), not left to a reviewer to
/// remember to check visually.
class EmergencyContactBanner extends StatelessWidget {
  const EmergencyContactBanner({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Container(
      margin: const EdgeInsets.fromLTRB(18, 14, 18, 6),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        gradient: const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [Color(0xFF064E3B), PatientTokens.primaryDark],
        ),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Row(
        children: [
          Container(
            width: 30,
            height: 30,
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: 0.22),
              shape: BoxShape.circle,
            ),
            alignment: Alignment.center,
            child: const Text('📞', style: TextStyle(fontSize: 14)),
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.emergencyContactLabel,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w800,
                    color: Colors.white,
                  ),
                ),
                Text(
                  l10n.emergencyContactSubtitle,
                  style: const TextStyle(fontSize: 9.5, color: Colors.white70),
                ),
              ],
            ),
          ),
          FilledButton(
            onPressed: () => launchUrl(Uri.parse(EmergencyContact.kenyaTelUri)),
            style: FilledButton.styleFrom(
              backgroundColor: Colors.white,
              foregroundColor: const Color(0xFF064E3B),
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
              textStyle:
                  const TextStyle(fontSize: 11, fontWeight: FontWeight.w800),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            child: Text(
                l10n.emergencyContactCallButton(EmergencyContact.kenyaNumber)),
          ),
        ],
      ),
    );
  }
}
