import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/core/widgets/afya_widgets.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profile_chip_row.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_models.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

/// Labels a phone set to large text used to cut or break mid-word: the
/// "Add Dependent" tile and the longer Swahili Home module names.
/// Set SNAPSHOT_DIR to also save what was rendered, for a visual check.
void main() {
  setUpAll(() async {
    await (FontLoader('Plus Jakarta Sans')
          ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf')))
        .load();
    await (FontLoader('MaterialIcons')
          ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf')))
        .load();
  });

  const self = PatientProfileDto(
      id: 'p1',
      userId: 'u1',
      fullName: 'mdnd',
      dateOfBirth: '1996-01-01',
      gender: 'MALE');

  Future<void> pump(WidgetTester tester, Locale locale, Widget child) async {
    tester.view.physicalSize = const Size(393, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: PatientTokens.theme(),
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: MediaQuery(
            data: const MediaQueryData(
                size: Size(393, 640), textScaler: TextScaler.linear(1.3)),
            child: Scaffold(
                body: RepaintBoundary(
                    key: const ValueKey('shot'),
                    child: Padding(
                        padding: const EdgeInsets.all(20), child: child))))));
    await tester.pumpAndSettle();
  }

  Future<void> snapshot(WidgetTester tester, String name) async {
    final dir = Platform.environment['SNAPSHOT_DIR'];
    if (dir == null) return;
    final boundary = tester.renderObject<RenderRepaintBoundary>(
        find.byKey(const ValueKey('shot')));
    final bytes = await tester.runAsync(() async {
      final image = await boundary.toImage();
      return (await image.toByteData(format: ui.ImageByteFormat.png))!;
    });
    File('$dir/$name.png').writeAsBytesSync(bytes!.buffer.asUint8List());
  }

  /// Every rendered Text fits its box: no clipped lines and no ellipsis.
  void expectNoCutText(WidgetTester tester) {
    for (final p
        in tester.renderObjectList<RenderParagraph>(find.byType(RichText))) {
      expect(p.didExceedMaxLines, isFalse,
          reason: 'cut off: "${p.text.toPlainText()}"');
    }
    expect(tester.takeException(), isNull);
  }

  for (final locale in const [Locale('en'), Locale('sw')]) {
    testWidgets(
        'Add Dependent stays whole at large text (${locale.languageCode})',
        (tester) async {
      await pump(
          tester,
          locale,
          FamilyProfileChipRow(
              profiles: const [self],
              activeProfileId: 'p1',
              onSelect: (_) {},
              onAddDependent: () {}));
      await snapshot(tester, 'family-row-${locale.languageCode}');
      expectNoCutText(tester);
      final label = find.byKey(const ValueKey('add-dependent-label'));
      final tile =
          find.ancestor(of: label, matching: find.byType(Container)).first;
      expect(tester.getRect(label).width,
          lessThanOrEqualTo(tester.getRect(tile).width));
    });
  }

  testWidgets('Swahili Home module names are shown in full at large text',
      (tester) async {
    late AppLocalizations l;
    await pump(tester, const Locale('sw'), Builder(builder: (context) {
      l = AppLocalizations.of(context);
      final labels = [
        l.moduleHivPrevention,
        l.moduleSupportChat,
        l.moduleBookConsultation,
        l.modulePeerNavigator,
        l.moduleReminders,
        l.moduleHealthRecords,
      ];
      return Column(children: [
        for (var row = 0; row < 2; row++)
          Padding(
            padding: const EdgeInsets.only(bottom: 10),
            child: IntrinsicHeight(
              child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (var col = 0; col < 3; col++) ...[
                      if (col > 0) const SizedBox(width: 10),
                      Expanded(
                          child: AfyaServiceTile(
                              icon: Icons.favorite_outline,
                              color: PatientTokens.primary,
                              background: PatientTokens.tileGreenBg,
                              label: labels[row * 3 + col],
                              labelWidth: (353 - 20) / 3 - 8,
                              onTap: () {})),
                    ],
                  ]),
            ),
          ),
      ]);
    }));
    await snapshot(tester, 'home-modules-sw');
    expectNoCutText(tester);
    expect(l.modulePeerNavigator, isNot(contains('Mwenzi')),
        reason: '"mwenzi" means a partner; a peer is "rika"');
  });
}
