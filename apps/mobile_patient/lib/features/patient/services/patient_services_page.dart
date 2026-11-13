import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import '../modules/patient_modules.dart';

/// Services tab: a directory of the nine approved modules, in the approved
/// order. Entries and destinations come from [patientModules], the same list
/// the Home grid renders.
class PatientServicesPage extends StatelessWidget {
  const PatientServicesPage({super.key, required this.nav});
  final PatientShellNav nav;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final modules = patientModules(context, nav);
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
          children: [
            Text(l10n.navServices,
                style: const TextStyle(
                    fontSize: 22,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
            const SizedBox(height: 2),
            Text(l10n.servicesDirectorySubtitle,
                style: const TextStyle(
                    fontSize: 13, color: PatientTokens.greetingSub)),
            const SizedBox(height: 16),
            for (final m in modules)
              AfyaModuleRow(
                  icon: m.icon,
                  color: m.color,
                  background: m.background,
                  title: m.label,
                  subtitle: m.description,
                  onTap: () => m.open(context)),
          ],
        ),
      ),
    );
  }
}
