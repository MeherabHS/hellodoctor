import '../../l10n/app_localizations.dart';

String patientSpecialtyLabel(AppLocalizations l10n, String value) =>
    switch (value) {
      'Internal Medicine' => l10n.specialtyInternalMedicine,
      'Cardiology' => l10n.specialtyCardiology,
      'Pediatrics' => l10n.specialtyPediatrics,
      'Gynecology' => l10n.specialtyGynecology,
      'Dermatology' => l10n.specialtyDermatology,
      _ => value,
    };
