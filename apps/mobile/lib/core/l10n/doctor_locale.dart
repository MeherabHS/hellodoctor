import 'package:flutter/material.dart';
import '../../l10n/app_localizations.dart';
import '../../l10n/app_localizations_en.dart';
import '../network/api_client.dart';

AppLocalizations doctorL10n(BuildContext context) =>
    Localizations.of<AppLocalizations>(context, AppLocalizations) ??
    AppLocalizationsEn();

/// Device UI preference only; no clinical data or notification payloads are stored.
class DoctorLocaleController extends ChangeNotifier {
  DoctorLocaleController(this.store);
  final TokenStore store;
  Locale locale = const Locale('en');
  int _generation = 0;
  int get revision => _generation;
  bool _disposed = false;
  Future<void> load() async {
    final generation = _generation;
    try {
      final code = await store.read('doctor_locale_v1');
      if (_disposed || generation != _generation) return;
      locale = Locale(code == 'sw' ? 'sw' : 'en');
      notifyListeners();
    } catch (_) {/* Complete English fallback. */}
  }

  Future<void> setLanguage(String code, {int? expectedRevision}) async {
    if (_disposed ||
        expectedRevision != null && expectedRevision != _generation) {
      return;
    }
    _generation++;
    locale = Locale(code == 'sw' ? 'sw' : 'en');
    if (!_disposed) notifyListeners();
    try {
      await store.write('doctor_locale_v1', locale.languageCode);
    } catch (_) {}
  }

  Future<void> Function(String code)? _serverSync;

  /// The save started by the last [chooseLanguage]; screens that load the
  /// server's language wait for it so they never undo a fresh choice.
  Future<void> serverSync = Future<void>.value();

  void bindServerSync(Future<void> Function(String code) sync) {
    _serverSync = sync;
  }

  /// A language picked by the user (onboarding switch): applied at once,
  /// kept on this phone and saved to the account (best effort) so the
  /// signed-in shell, which reads the account's language, keeps it.
  Future<void> chooseLanguage(String code) async {
    if (_disposed || locale.languageCode == code) return;
    await setLanguage(code);
    final sync = _serverSync;
    if (sync != null) {
      serverSync = sync(locale.languageCode).catchError((_) {});
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    super.dispose();
  }
}

class DoctorLocaleScope extends InheritedNotifier<DoctorLocaleController> {
  const DoctorLocaleScope(
      {super.key,
      required DoctorLocaleController controller,
      required super.child})
      : super(notifier: controller);
  static DoctorLocaleController? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DoctorLocaleScope>()?.notifier;
}
