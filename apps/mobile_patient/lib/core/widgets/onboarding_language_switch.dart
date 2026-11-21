import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../l10n/app_localizations.dart';
import '../l10n/locale_controller.dart';

/// EN | SW pill shown beside Skip on the patient onboarding screens
/// (product owner, 2026-11-21). Tapping a side switches the whole app at
/// once, onboarding included, and keeps the choice on this phone (and on
/// the account after sign-in, see [LocaleController.setLocale]).
class OnboardingLanguageSwitch extends StatelessWidget {
  const OnboardingLanguageSwitch({
    super.key,
    required this.fill,
    required this.active,
    required this.text,
    required this.fontSize,
  });

  final Color fill;
  final Color active;
  final Color text;
  final double fontSize;

  @override
  Widget build(BuildContext context) {
    LocaleController? controller;
    try {
      controller = context.watch<LocaleController>();
    } on ProviderNotFoundException {
      controller = null; // Widget tests without the app's provider.
    }
    final l = AppLocalizations.of(context);
    final current = controller?.locale.languageCode ??
        Localizations.localeOf(context).languageCode;
    Widget side(String code, String label, String name) {
      final selected = current == code;
      return Expanded(
        child: Semantics(
          button: true,
          selected: selected,
          label: name,
          excludeSemantics: true,
          child: GestureDetector(
            key: ValueKey('onboarding-language-$code'),
            behavior: HitTestBehavior.opaque,
            onTap: () => controller?.setLocale(code),
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 180),
              alignment: Alignment.center,
              decoration: ShapeDecoration(
                  color: selected ? active : Colors.transparent,
                  shape: const StadiumBorder()),
              child: FittedBox(
                fit: BoxFit.scaleDown,
                child: Text(label,
                    maxLines: 1,
                    style: TextStyle(
                        fontSize: fontSize,
                        height: 1,
                        fontWeight: FontWeight.w700,
                        fontVariations: const [FontVariation('wght', 700)],
                        letterSpacing: 0.4,
                        color: selected ? Colors.white : text)),
              ),
            ),
          ),
        ),
      );
    }

    return Semantics(
      container: true,
      label: l.languageTitle,
      child: DecoratedBox(
        decoration: ShapeDecoration(color: fill, shape: const StadiumBorder()),
        child: Padding(
          padding: EdgeInsets.all(fontSize * 0.18),
          child: Row(children: [
            side('en', 'EN', l.languageEnglish),
            side('sw', 'SW', l.languageSwahili),
          ]),
        ),
      ),
    );
  }
}
