import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/onboarding/account_onboarding_gate.dart';
import 'package:hellodoctor_mobile/features/onboarding/peer_onboarding_page.dart';
import 'package:hellodoctor_mobile/l10n/app_localizations.dart';

class _Store implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async => values[key] = value;
  @override
  Future<void> delete(String key) async => values.remove(key);
  @override
  Future<void> clear() async => values.clear();
}

Widget _app(Widget home, {Locale? locale}) => MaterialApp(
    theme: DoctorTokens.theme(),
    locale: locale,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: home);

Finder _key(String k) => find.byKey(ValueKey(k));

void main() {
  setUp(() {
    final view = TestWidgetsFlutterBinding.ensureInitialized()
        .platformDispatcher
        .views
        .first;
    view.physicalSize = const Size(1080, 2340);
    view.devicePixelRatio = 3;
  });
  tearDown(() {
    final view =
        TestWidgetsFlutterBinding.instance.platformDispatcher.views.first;
    view.resetPhysicalSize();
    view.resetDevicePixelRatio();
  });

  testWidgets('four slides naming the real peer features', (tester) async {
    var done = 0;
    await tester.pumpWidget(_app(PeerOnboardingPage(onDone: () => done++)));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('1 of 4'), findsOneWidget);
    expect(find.text('Welcome, Peer Navigator'), findsOneWidget);
    expect(find.text('Get Started'), findsOneWidget);
    expect(_key('peer-onboarding-back'), findsNothing);

    await tester.tap(_key('peer-onboarding-next'));
    await tester.pumpAndSettle();
    expect(find.text('Be a Supportive Peer'), findsOneWidget);
    await tester.tap(_key('peer-onboarding-back'));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('1 of 4'), findsOneWidget);

    for (var i = 2; i <= 4; i++) {
      await tester.tap(_key('peer-onboarding-next'));
      await tester.pumpAndSettle();
      expect(find.bySemanticsLabel('$i of 4'), findsOneWidget);
      if (i == 3) {
        // The peer app's own tabs, nothing it cannot do.
        expect(find.text('Support Requests'), findsOneWidget);
        expect(find.text('Chats'), findsOneWidget);
        expect(find.text('Schedule & Availability'), findsOneWidget);
        expect(find.textContaining('Activities'), findsNothing);
      }
    }
    expect(find.text('Ready to begin?'), findsOneWidget);
    expect(find.text('Go to My Dashboard'), findsOneWidget);
    expect(done, 0);
    await tester.tap(_key('peer-onboarding-finish'));
    expect(done, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Skip leaves from any slide', (tester) async {
    var done = 0;
    await tester.pumpWidget(_app(PeerOnboardingPage(onDone: () => done++)));
    await tester.pumpAndSettle();
    await tester.tap(_key('peer-onboarding-next'));
    await tester.pumpAndSettle();
    await tester.tap(_key('peer-onboarding-skip'));
    expect(done, 1);
  });

  testWidgets('Swahili copy and small screens without overflow',
      (tester) async {
    for (final size in const [Size(720, 1280), Size(1080, 2340)]) {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = size.width < 1000 ? 2 : 3;
      await tester.pumpWidget(_app(
          PeerOnboardingPage(key: UniqueKey(), onDone: () {}),
          locale: const Locale('sw')));
      await tester.pumpAndSettle();
      expect(find.text('Ruka'), findsOneWidget);
      expect(find.text('Karibu, Mwelekezi Rika'), findsOneWidget);
      for (var i = 0; i < 3; i++) {
        await tester.tap(_key('peer-onboarding-next'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull, reason: '$size slide ${i + 2}');
      }
      expect(find.text('Nenda kwenye Dashibodi Yangu'), findsOneWidget);
    }
  });

  testWidgets('gate: once per peer account', (tester) async {
    final store = _Store();
    Widget gate(String? account) => _app(PeerOnboardingGate(
        store: store,
        account: account,
        child: const Text('peer home', key: ValueKey('peer-home'))));
    await tester.pumpWidget(gate('peer-1'));
    await tester.pumpAndSettle();
    expect(find.byType(PeerOnboardingPage), findsOneWidget);
    await tester.tap(_key('peer-onboarding-skip'));
    await tester.pumpAndSettle();
    expect(_key('peer-home'), findsOneWidget);
    expect(store.values[onboardingSeenKey('peer', 'peer-1')], '1');
    await tester.pumpWidget(const SizedBox());
    await tester.pumpWidget(gate('peer-1'));
    await tester.pumpAndSettle();
    expect(find.byType(PeerOnboardingPage), findsNothing);
  });

  test('signing out keeps doctor and peer onboarding flags', () async {
    FlutterSecureStorage.setMockInitialValues({
      onboardingSeenKey('peer', 'peer-1'): '1',
      onboardingSeenKey('doctor', 'doc-1'): '1',
      'access_token': 'a',
    });
    final store = SecureTokenStore(const FlutterSecureStorage());
    await store.clear();
    expect(await store.read('access_token'), isNull);
    expect(await store.read(onboardingSeenKey('peer', 'peer-1')), '1');
    expect(await store.read(onboardingSeenKey('doctor', 'doc-1')), '1');
  });
}
