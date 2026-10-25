import 'dart:async';

import 'package:flutter/material.dart';

import '../network/api_client.dart';

/// Persists and broadcasts the patient's chosen app language (English /
/// Swahili — `prototype/index.html`'s `#languageModal`, surfaced in
/// `features/patient/profile/language_modal.dart`).
///
/// Deliberately a plain [ChangeNotifier], not a cubit: this is pure UI-chrome
/// state, so the lighter-weight tool is the right fit, per the implementation
/// plan's explicit call on this. Milestone 3 adds one narrow exception to
/// "no network call": a best-effort backend sync of the chosen language via
/// [bindBackendSync], so `users.preferred_language` (previously never
/// actually written anywhere in the backend) reflects what the Patient
/// actually sees, for notification-content localization
/// (`NotificationService::render`). That sync is fire-and-forget and never
/// blocks or fails the local, synchronous language switch below.
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
  Future<void> Function(String languageCode)? _backendSync;

  Locale get locale => _locale;

  Future<void> load() async {
    final saved = await _store.read(_storageKey);
    if (saved != null && supportedLanguageCodes.contains(saved)) {
      _locale = Locale(saved);
      notifyListeners();
    }
  }

  /// Wires the best-effort backend sync call (`NotificationRepository.
  /// syncLanguage`), set once a signed-in session's `api`/repository exists
  /// — construction of this controller itself happens earlier, before login,
  /// per `app.dart`.
  void bindBackendSync(Future<void> Function(String languageCode) sync) {
    _backendSync = sync;
  }

  Future<void> setLocale(String languageCode) async {
    if (!supportedLanguageCodes.contains(languageCode)) return;
    if (_locale.languageCode == languageCode) return;
    _locale = Locale(languageCode);
    notifyListeners();
    await _store.write(_storageKey, languageCode);
    final sync = _backendSync;
    if (sync != null) {
      unawaited(sync(languageCode).catchError((_) {}));
    }
  }
}
