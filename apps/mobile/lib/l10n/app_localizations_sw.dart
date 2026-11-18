// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Swahili (`sw`).
class AppLocalizationsSw extends AppLocalizations {
  AppLocalizationsSw([String locale = 'sw']) : super(locale);

  @override
  String get batteryPromptTitle => 'Pokea vikumbusho kwa wakati';

  @override
  String get batteryPromptBody =>
      'Simu yako inaweza kusimamisha programu hii chinichini ili kuokoa betri. Hilo linaweza kuchelewesha au kuzuia vikumbusho vya mashauriano na ujumbe. Ili uvipokee kwa wakati, ruhusu programu iendelee kufanya kazi chinichini.';

  @override
  String get batteryPromptLater => 'Si sasa';

  @override
  String get batteryPromptOpenSettings => 'Fungua mipangilio';

  @override
  String get batteryStepsSamsung =>
      'Gusa Fungua mipangilio, kisha Battery, na uchague Unrestricted. Pia hakikisha programu hii haiko kwenye Sleeping apps (Settings > Battery > Background usage limits).';

  @override
  String get batteryStepsTranssion =>
      'Gusa Fungua mipangilio, kisha Battery, na uruhusu programu ifanye kazi chinichini (zima battery optimisation). Kwenye Phone Master, ruhusu programu hii ijiwashe yenyewe (auto-start).';

  @override
  String get batteryStepsXiaomi =>
      'Gusa Fungua mipangilio, kisha Battery saver, na uchague No restrictions. Kisha washa Autostart.';

  @override
  String get batteryStepsOppo =>
      'Gusa Fungua mipangilio, kisha Battery, na uwashe Allow background activity. Pia ruhusu Auto launch.';

  @override
  String get batteryStepsVivo =>
      'Gusa Fungua mipangilio, kisha Battery, na uruhusu matumizi makubwa ya betri chinichini. Pia ruhusu Auto-start.';

  @override
  String get batteryStepsHuawei =>
      'Gusa Fungua mipangilio, kisha Battery, kisha App launch. Weka programu hii kwenye Manage manually na uwashe swichi zote tatu.';

  @override
  String get batteryStepsOther =>
      'Gusa Fungua mipangilio, kisha Battery, na uchague Unrestricted (au Don\'t optimise).';

  @override
  String get chatReceiptSending => 'Inatumwa';

  @override
  String get chatReceiptSent => 'Imetumwa';

  @override
  String get chatReceiptRead => 'Imesomwa';

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
  String get appointmentStatusLapsed => 'Muda umepita';

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
