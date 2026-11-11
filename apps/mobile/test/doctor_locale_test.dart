import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/l10n/doctor_locale.dart';
import 'notification_navigation_test.dart' show NavigationTokens;

class DelayedLocaleStore extends NavigationTokens {
  final delayed = Completer<String?>();
  @override
  Future<String?> read(String key) => delayed.future;
}

void main() {
  test('Doctor preference loads Kiswahili and falls back completely to English',
      () async {
    final store = NavigationTokens();
    await store.write('doctor_locale_v1', 'sw');
    final locale = DoctorLocaleController(store);
    addTearDown(locale.dispose);
    await locale.load();
    expect(locale.locale.languageCode, 'sw');
    await locale.setLanguage('unsupported');
    expect(locale.locale.languageCode, 'en');
  });
  test(
      'a late device read or stale backend preference cannot replace a newer choice',
      () async {
    final store = DelayedLocaleStore();
    final active = DoctorLocaleController(store);
    addTearDown(active.dispose);
    final load = active.load();
    final revision = active.revision;
    await active.setLanguage('sw');
    store.delayed.complete('en');
    await load;
    await active.setLanguage('en', expectedRevision: revision);
    expect(active.locale.languageCode, 'sw');
  });
  test('a delayed locale load after disposal has no listener side effect',
      () async {
    final store = DelayedLocaleStore();
    final active = DoctorLocaleController(store);
    var notifications = 0;
    active.addListener(() => notifications++);
    final load = active.load();
    active.dispose();
    store.delayed.complete('sw');
    await load;
    expect(notifications, 0);
  });
}
