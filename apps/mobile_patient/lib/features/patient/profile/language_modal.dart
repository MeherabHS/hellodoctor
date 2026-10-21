// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';
import '../../../l10n/app_localizations.dart';

import '../../../core/l10n/locale_controller.dart';
import '../../../core/theme/tokens.dart';

/// `prototype/index.html`'s `#languageModal`, reached from the Profile
/// screen's Settings menu (`#patientLangMenuItem`). Unlike the prototype
/// (which only updates a cosmetic badge), selecting a language here calls
/// [LocaleController.setLocale] and really retranslates the app via
/// `AppLocalizations`/`MaterialApp.locale`.
Future<void> showLanguageModal(BuildContext context, LocaleController locale) {
  return showModalBottomSheet(
    context: context,
    builder: (context) => AnimatedBuilder(
      animation: locale,
      builder: (context, _) => Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(AppLocalizations.of(context).languageTitle,
                style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
            const SizedBox(height: 12),
            _LanguageOption(
              flag: '🇬🇧',
              title: AppLocalizations.of(context).languageEnglish,
              subtitle: AppLocalizations.of(context).languageEnglishDescription,
              selected: locale.locale.languageCode == 'en',
              onTap: () => locale.setLocale('en'),
            ),
            const SizedBox(height: 8),
            _LanguageOption(
              flag: '🇰🇪',
              title: AppLocalizations.of(context).languageSwahili,
              subtitle: AppLocalizations.of(context).languageSwahiliDescription,
              selected: locale.locale.languageCode == 'sw',
              onTap: () => locale.setLocale('sw'),
            ),
          ],
        ),
      ),
    ),
  );
}

class _LanguageOption extends StatelessWidget {
  const _LanguageOption({
    required this.flag,
    required this.title,
    required this.subtitle,
    required this.selected,
    required this.onTap,
  });
  final String flag, title, subtitle;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
        child: Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: selected
                ? PatientTokens.primaryLight
                : PatientTokens.surfaceSubtle,
            borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
            border: Border.all(
                color: selected ? PatientTokens.primary : PatientTokens.border),
          ),
          child: Row(children: [
            Text(flag, style: const TextStyle(fontSize: 22)),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title,
                      style: const TextStyle(
                          fontWeight: FontWeight.w700,
                          color: PatientTokens.text)),
                  Text(subtitle,
                      style: const TextStyle(
                          fontSize: 11, color: PatientTokens.textSub)),
                ],
              ),
            ),
            if (selected)
              const Icon(Icons.check_circle, color: PatientTokens.primary),
          ]),
        ),
      );
}
