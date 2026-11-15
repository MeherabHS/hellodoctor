import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/features/patient/modules/module_screens.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

Widget _host(Widget child, {Locale locale = const Locale('en')}) => MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: child),
    );

void main() {
  testWidgets('no number configured shows the coming-soon placeholder',
      (tester) async {
    await tester.pumpWidget(_host(const SupportWhatsAppCard(number: '')));
    expect(find.text('Need immediate assistance?'), findsOneWidget);
    expect(find.text('Text us on WhatsApp'), findsOneWidget);
    expect(find.text('WhatsApp number coming soon'), findsOneWidget);
    expect(find.byIcon(Icons.chevron_right_rounded), findsNothing);
  });

  testWidgets('a configured number is shown beneath the prompt',
      (tester) async {
    await tester.pumpWidget(
        _host(const SupportWhatsAppCard(number: '+254 700 000 000')));
    expect(find.text('+254 700 000 000'), findsOneWidget);
    expect(find.text('WhatsApp number coming soon'), findsNothing);
    expect(find.byIcon(Icons.chevron_right_rounded), findsOneWidget);
  });

  testWidgets('Swahili copy', (tester) async {
    await tester.pumpWidget(_host(const SupportWhatsAppCard(number: ''),
        locale: const Locale('sw')));
    expect(find.text('Unahitaji msaada wa haraka?'), findsOneWidget);
    expect(find.text('Namba ya WhatsApp itapatikana hivi karibuni'),
        findsOneWidget);
  });
}
