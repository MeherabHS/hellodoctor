import 'package:flutter/material.dart';

import '../network/api_client.dart';

/// Persists and broadcasts the patient's chosen app language (English /
/// Swahili — `prototype/index.html`'s `#languageModal`, surfaced in
/// `features/patient/profile/language_modal.dart`).
///
/// Deliberately a plain [ChangeNotifier], not a cubit: this is pure UI-chrome
/// state (no network call, no business logic), so the lighter-weight tool is
/// the right fit, per the implementation plan's explicit call on this.
///
/// Stored directly via [TokenStore] under its own device-level key — NOT
/// through `PatientCacheStore`'s JWT-subject-scoped cache, since a language
/// preference is a device/UI preference that must survive logout and account
/// switching, unlike the account-scoped data `PatientCacheStore` exists to
/// wipe on sign-out.
class LocaleController extends ChangeNotifier {
  LocaleController(this._store);

  static const _storageKey = 'patient_locale_v1';
  static const supportedLanguageCodes = {'en', 'sw'};

  final TokenStore _store;
  Locale _locale = const Locale('en');

  Locale get locale => _locale;

  Future<void> load() async {
    final saved = await _store.read(_storageKey);
    if (saved != null && supportedLanguageCodes.contains(saved)) {
      _locale = Locale(saved);
      notifyListeners();
    }
  }

  Future<void> setLocale(String languageCode) async {
    if (!supportedLanguageCodes.contains(languageCode)) return;
    if (_locale.languageCode == languageCode) return;
    _locale = Locale(languageCode);
    notifyListeners();
    await _store.write(_storageKey, languageCode);
  }
}
