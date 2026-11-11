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
