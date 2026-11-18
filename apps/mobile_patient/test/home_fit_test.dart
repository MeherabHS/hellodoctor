import 'dart:io';
import 'dart:ui' as ui;

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/core/widgets/afya_bottom_nav.dart';
import 'package:hellodoctor_mobile_patient/core/widgets/afya_widgets.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/patient_home_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/modules/patient_modules.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

/// Home without a search field (removed 2026-11-17): on ordinary phones all
/// nine modules fit on screen with no scrolling; a phone too small for
/// legible cards falls back to scrolling. Set SNAPSHOT_DIR to save renders.
/// Every request, whatever the method, is answered here, never on a network.
class _HomeApi extends ApiClient {
  _HomeApi() : super(dio: _offlineDio());
  static const _self = {
    'id': 'p1',
    'user_id': 'u1',
    'full_name': 'mdnd',
    'date_of_birth': '1996-01-01',
    'gender': 'MALE',
  };
  static Dio _offlineDio() => Dio(BaseOptions(baseUrl: 'https://example.test'))
    ..interceptors.add(InterceptorsWrapper(onRequest: (o, handler) {
      final path = o.uri.path;
      if (path.endsWith('/photo')) {
        // No profile photo: the avatar shows initials.
        handler.reject(DioException(
            requestOptions: o,
            response: Response(requestOptions: o, statusCode: 404)));
        return;
      }
      final Object data = switch (path) {
        '/patients/me' => _self,
        '/patients/me/profiles' => [_self],
        '/patients/me/appointments' => const [],
        '/notifications/unread-count' => const {'unread_count': 0},
        '/notifications' => const [],
        _ => const <String, dynamic>{},
      };
      handler.resolve(
          Response(requestOptions: o, statusCode: 200, data: {'data': data}));
    }));
}

class _Phone {
  const _Phone(this.name, this.size, this.textScale, this.top, this.bottom,
      {this.locale = const Locale('en'), required this.fits});
  final String name;
  final Size size;
  final double textScale, top, bottom;
  final Locale locale;
  final bool fits;
}

void main() {
  setUpAll(() async {
    await (FontLoader('Plus Jakarta Sans')
          ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf')))
        .load();
    await (FontLoader('MaterialIcons')
          ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf')))
        .load();
  });

  const phones = [
    // The product owner's phone, measured from their 2026-11-17 screenshot:
    // 1080 x 2352 px at 2.625x (411 x 896 dp), three-button navigation. The
    // old Home cut off its third row of modules.
    _Phone('owner-phone-en', Size(411, 896), 1.0, 28, 48, fits: true),
    _Phone('owner-phone-sw', Size(411, 896), 1.0, 28, 48,
        locale: Locale('sw'), fits: true),
    _Phone('owner-phone-text-1.1', Size(411, 896), 1.1, 28, 48, fits: true),
    _Phone('pixel-7', Size(412, 915), 1.0, 28, 24, fits: true),
    // Too short for legible cards (small phone, or large system text):
    // Home scrolls instead of shrinking or clipping anything.
    _Phone('small-phone-large-text', Size(360, 784), 1.15, 26, 48, fits: false),
    _Phone('small-phone', Size(360, 640), 1.0, 24, 48, fits: false),
  ];

  for (final phone in phones) {
    testWidgets('Home on ${phone.name}', (tester) async {
      const dpr = 3.0;
      tester.view.physicalSize = phone.size * dpr;
      tester.view.devicePixelRatio = dpr;
      tester.view.padding =
          FakeViewPadding(top: phone.top * dpr, bottom: phone.bottom * dpr);
      tester.platformDispatcher.textScaleFactorTestValue = phone.textScale;
      addTearDown(tester.view.reset);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);

      final api = _HomeApi();
      addTearDown(api.dispose);
      final home = HomeCubit(HomeRepository(api));
      final profile = ProfileCubit(ProfileRepository(api));
      final family = FamilyProfilesCubit(FamilyProfilesRepository(api));
      final notifications = NotificationCubit(NotificationRepository(api));
      for (final c in [home, profile, family, notifications]) {
        addTearDown(c.close);
      }
      final nav = PatientShellNav(
          openServices: () {},
          openMessages: () {},
          openProfile: () {},
          select: (_) {});

      await tester.pumpWidget(RepaintBoundary(
        key: const ValueKey('shot'),
        child: MultiBlocProvider(
          providers: [
            BlocProvider.value(value: home),
            BlocProvider.value(value: profile),
            BlocProvider.value(value: family),
            BlocProvider.value(value: notifications),
          ],
          child: MaterialApp(
            debugShowCheckedModeBanner: false,
            theme: PatientTokens.theme(),
            locale: phone.locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(
              backgroundColor: PatientTokens.bg,
              body: PatientHomePage(nav: nav),
              bottomNavigationBar: SafeArea(
                  top: false,
                  child: PatientBottomNav(selected: 0, onSelected: (_) {})),
            ),
          ),
        ),
      ));
      await tester.pumpAndSettle();
      final ctx = tester.element(find.byType(PatientHomePage));
      await tester.runAsync(() async {
        for (final asset in const [
          'assets/illustrations/home_hero_girl.webp',
          'assets/brand/afya_logo.webp',
        ]) {
          await precacheImage(AssetImage(asset), ctx);
        }
      });
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.textContaining('Search'), findsNothing);
      expect(find.textContaining('Tafuta'), findsNothing);
      final tiles = find.byType(AfyaServiceTile);
      expect(tiles, findsNWidgets(9));
      for (final p
          in tester.renderObjectList<RenderParagraph>(find.byType(RichText))) {
        expect(p.didExceedMaxLines, isFalse,
            reason: 'cut off: "${p.text.toPlainText()}"');
      }

      final scroll = tester.state<ScrollableState>(find.descendant(
          of: find.byType(PatientHomePage), matching: find.byType(Scrollable)));
      final navTop = tester.getTopLeft(find.byType(PatientBottomNav)).dy;
      if (phone.fits) {
        expect(scroll.position.maxScrollExtent, 0,
            reason: 'everything fits without scrolling');
        for (final e in tiles.evaluate()) {
          expect(tester.getRect(find.byWidget(e.widget)).bottom,
              lessThanOrEqualTo(navTop),
              reason: 'every module is fully above the navigation bar');
        }
      } else {
        expect(scroll.position.maxScrollExtent, greaterThan(0),
            reason: 'a too-small screen scrolls instead of shrinking cards');
      }

      final dir = Platform.environment['SNAPSHOT_DIR'];
      if (dir != null) {
        final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('shot')));
        final bytes = await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 2);
          return (await image.toByteData(format: ui.ImageByteFormat.png))!;
        });
        File('$dir/home-${phone.name}.png')
            .writeAsBytesSync(bytes!.buffer.asUint8List());
      }
    });
  }
}
