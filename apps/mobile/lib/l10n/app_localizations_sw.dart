// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Swahili (`sw`).
class AppLocalizationsSw extends AppLocalizations {
  AppLocalizationsSw([String locale = 'sw']) : super(locale);

  @override
  String get notificationsTitle => 'Arifa';

  @override
  String get notificationsSubtitle =>
      'Vikumbusho vya miadi na taarifa muhimu za akaunti.';

  @override
  String get notificationsMarkAllRead => 'Tia alama zote zimesomwa';

  @override
  String get notificationsLoadError =>
      'Arifa hazikuweza kupakiwa. Tafadhali jaribu tena.';

  @override
  String get notificationsRefreshError =>
      'Arifa hazikuweza kusasishwa. Tafadhali jaribu tena.';

  @override
  String get notificationsEmpty => 'Bado hakuna arifa';

  @override
  String get notificationsLoadOlder => 'Pakia arifa za awali';

  @override
  String get notificationsLoadingOlder => 'Inapakia arifa za awali...';

  @override
  String get notificationDestinationError =>
      'Ukurasa huu haupatikani au huna ruhusa tena. Tafadhali jaribu tena kutoka kwenye arifa zako.';

  @override
  String get retryButton => 'Jaribu tena';

  @override
  String get appointmentTitle => 'Miadi';

  @override
  String get appointmentReference => 'Nambari ya miadi';

  @override
  String get refreshButton => 'Sasisha';

  @override
  String get doctorLabel => 'Daktari';

  @override
  String get patientLabel => 'Mgonjwa';

  @override
  String get restoringSession => 'Inarejesha kipindi salama cha daktari...';

  @override
  String get notificationLanguage => 'Lugha ya arifa';

  @override
  String get notificationSettingsError =>
      'Imeshindwa kuhifadhi mipangilio ya arifa.';

  @override
  String get appointmentStatusPendingPayment => 'Inasubiri malipo';

  @override
  String get appointmentStatusConfirmed => 'Imethibitishwa';

  @override
  String get appointmentStatusWaiting => 'Inasubiri';

  @override
  String get appointmentStatusInConsultation => 'Katika ushauri';

  @override
  String get appointmentStatusCompleted => 'Imekamilika';

  @override
  String get appointmentStatusCancelled => 'Imeghairiwa';

  @override
  String get appointmentStatusExpired => 'Muda umeisha';

  @override
  String get appointmentStatusRefunded => 'Malipo yamerejeshwa';

  @override
  String get appointmentStatusDisputed => 'Inakaguliwa';

  @override
  String get appointmentStatusPrematureTermination => 'Imekatizwa';

  @override
  String get cacheOffline => 'Nje ya mtandao — inaonyesha data iliyohifadhiwa';

  @override
  String cacheLastUpdated(String time) {
    return 'Ilisasishwa $time';
  }

  @override
  String cacheOfflineUpdated(String time) {
    return 'Nje ya mtandao — inaonyesha data iliyohifadhiwa · Ilisasishwa $time';
  }

  @override
  String get settingsTitle => 'Mipangilio ya arifa';

  @override
  String get settingsLoadError => 'Imeshindwa kupakia mipangilio ya arifa.';

  @override
  String get settingsGreetings => 'Salamu';

  @override
  String get settingsGreetingsHelp =>
      'Salamu za asubuhi, siku za kuzaliwa na sikukuu za Kenya';

  @override
  String get settingsReligious => 'Salamu za sikukuu za kidini';

  @override
  String get settingsReligiousHelp =>
      'Salamu za Iddi, Ijumaa Kuu, Pasaka na Krismasi';

  @override
  String get settingsRoutine => 'Taarifa za kawaida';

  @override
  String get settingsRoutineHelp =>
      'Hali ya makala na arifa nyingine za shughuli';

  @override
  String get settingsAnnouncements => 'Matangazo';

  @override
  String get settingsAnnouncementsHelp =>
      'Arifa za hiari za ushiriki na matangazo';

  @override
  String get settingsQuietHours => 'Saa za utulivu';

  @override
  String get settingsQuietHoursHelp =>
      'Subirisha arifa zisizo za dharura katika kipindi hiki. Vikumbusho vya ushauri na arifa za muda maalumu havitacheleweshwa.';

  @override
  String get settingsStartsAt => 'Huanza saa';

  @override
  String get settingsEndsAt => 'Huisha saa';
}
