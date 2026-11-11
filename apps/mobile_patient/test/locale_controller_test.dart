import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/l10n/locale_controller.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class _MemoryStore implements TokenStore {
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

void main() {
  test(
      'locale preference survives controller recreation and rejects unknown codes',
      () async {
    final store = _MemoryStore();
    final first = LocaleController(store);

    await first.setLocale('sw');
    expect(first.locale.languageCode, 'sw');

    final restored = LocaleController(store);
    await restored.load();
    expect(restored.locale.languageCode, 'sw');

    await restored.setLocale('fr');
    expect(restored.locale.languageCode, 'sw');
  });

  test('English and Swahili provide translated M10 patient labels', () {
    final english = lookupAppLocalizations(const Locale('en'));
    final swahili = lookupAppLocalizations(const Locale('sw'));

    expect(english.qnaTitle, 'Ask Free Health Q&A');
    expect(swahili.qnaTitle, 'Uliza Maswali ya Afya Bure');
    expect(english.vaultTitle, 'Health Vault');
    expect(swahili.vaultTitle, 'Hifadhi ya Afya');
    expect(english.postConsultHeader, 'Consultation Completed');
    expect(swahili.postConsultHeader, isNot(english.postConsultHeader));
  });

  test(
      'Milestone 3 notification-settings labels are translated in both locales',
      () {
    final english = lookupAppLocalizations(const Locale('en'));
    final swahili = lookupAppLocalizations(const Locale('sw'));
    expect(english.notificationSettingsPageTitle, 'Notification settings');
    expect(swahili.notificationSettingsPageTitle, 'Mipangilio ya arifa');
    expect(swahili.notificationSettingsQuietHoursTitle,
        isNot(english.notificationSettingsQuietHoursTitle));
  });

  test('setLocale calls the bound backend-sync hook with the new language code',
      () async {
    final store = _MemoryStore();
    final controller = LocaleController(store);
    final syncedLanguages = <String>[];
    controller.bindBackendSync((languageCode) async {
      syncedLanguages.add(languageCode);
    });

    await controller.setLocale('sw');
    expect(controller.locale.languageCode, 'sw');
    expect(syncedLanguages, ['sw']);
  });

  test('setLocale never awaits or fails on a failing backend-sync hook',
      () async {
    final store = _MemoryStore();
    final controller = LocaleController(store);
    controller.bindBackendSync((_) async {
      throw Exception('network unavailable');
    });

    // Must complete normally — a failing best-effort sync must never
    // surface as an error from the local, synchronous language switch.
    await controller.setLocale('sw');
    expect(controller.locale.languageCode, 'sw');
  });

  test('setLocale is a no-op with no sync hook bound (unbound by default)',
      () async {
    final store = _MemoryStore();
    final controller = LocaleController(store);
    await controller.setLocale('sw');
    expect(controller.locale.languageCode, 'sw');
  });
}
