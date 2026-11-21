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

  @override
  String get notificationsToday => 'Leo';

  @override
  String get notificationsYesterday => 'Jana';

  @override
  String get notificationsEarlier => 'Awali';

  @override
  String get notificationsFilterAll => 'Zote';

  @override
  String get notificationsFilterAppointments => 'Miadi';

  @override
  String get notificationsFilterMessages => 'Ujumbe';

  @override
  String get notificationsFilterSystem => 'Mfumo';

  @override
  String get notificationsJustNow => 'Sasa hivi';

  @override
  String notificationsMinutesAgo(int count) {
    return 'Dakika $count zilizopita';
  }

  @override
  String notificationsHoursAgo(int count) {
    return 'Saa $count zilizopita';
  }

  @override
  String get doctorOnboardingSkip => 'Ruka';

  @override
  String get doctorOnboardingBack => 'Rudi';

  @override
  String get doctorOnboardingNext => 'Endelea';

  @override
  String get doctorOnboardingGetStarted => 'Anza';

  @override
  String get doctorOnboardingFinish => 'Nenda kwenye Dashibodi';

  @override
  String doctorOnboardingCounter(int current, int total) {
    return '$current kati ya $total';
  }

  @override
  String get doctorOnboarding1Title => 'Karibu, Daktari!';

  @override
  String get doctorOnboarding1Body =>
      'Jiunge na AfyaMkononi kutoa\nhuduma bora, ya siri na\nrafiki kwa vijana.';

  @override
  String get doctorOnboarding2Title => 'Toa Huduma Mtandaoni';

  @override
  String get doctorOnboarding2Body =>
      'Hudumia wagonjwa kupitia\nvideo au gumzo salama, toa\nushauri, na usaidie\nufuatiliaji wa huduma.';

  @override
  String get doctorOnboarding3Title => 'Simamia Miadi';

  @override
  String get doctorOnboarding3Body =>
      'Ona na usimamie ratiba\nyako ya mashauriano kwa\nurahisi, ukipata arifa na\nvikumbusho kwa wakati.';

  @override
  String get doctorOnboarding4Title => 'Uko Tayari!';

  @override
  String get doctorOnboarding4Body =>
      'Anza kutoa huduma\nya kuaminika, ya siri na\nrafiki kwa vijana\nkupitia AfyaMkononi.';

  @override
  String get brandTagline => 'Kinga · Pima · Tibu · Dumisha Afya';

  @override
  String get peerOnboardingSkip => 'Ruka';

  @override
  String get peerOnboardingNext => 'Endelea';

  @override
  String get peerOnboardingBack => 'Rudi';

  @override
  String get peerOnboardingGetStarted => 'Anza';

  @override
  String get peerOnboardingFinish => 'Nenda kwenye Dashibodi Yangu';

  @override
  String peerOnboardingCounter(int current, int total) {
    return '$current kati ya $total';
  }

  @override
  String get peerOnboarding1Bubble => 'Habari! 👋\nKaribu\nAfyaMkononi!';

  @override
  String get peerOnboarding1Title => 'Karibu, Mwelekezi Rika';

  @override
  String get peerOnboarding1Body =>
      'Wewe ni rika anayeaminika kwa vijana katika jamii yako. Shiriki taarifa sahihi, toa msaada na uwasaidie kupata huduma kwa njia salama na ya kirafiki.';

  @override
  String get peerOnboarding1Tile1 => 'Saidia\nRika Wako';

  @override
  String get peerOnboarding1Tile2 => 'Shiriki Taarifa\nza Kuaminika';

  @override
  String get peerOnboarding1Tile3 => 'Himiza\nHuduma';

  @override
  String get peerOnboarding2Bubble =>
      'Hauko peke yako.\nTuzungumze. Niko hapa\nkusikiliza na kusaidia! 💚';

  @override
  String get peerOnboarding2Title => 'Kuwa Rika Msaidizi';

  @override
  String get peerOnboarding2Body =>
      'Zungumza na vijana, toa faraja na shiriki uzoefu chanya katika mazingira salama na ya heshima.';

  @override
  String get peerOnboarding3Bubble =>
      'Haya ndiyo unayoweza\nkufanya kwenye dashibodi\nyako!';

  @override
  String get peerOnboarding3Title => 'Dashibodi Yako';

  @override
  String get peerOnboarding3Body =>
      'Kila unachohitaji kusaidia wengine, mahali pamoja.';

  @override
  String get peerOnboarding3Row1Title => 'Maombi ya Msaada';

  @override
  String get peerOnboarding3Row1Body =>
      'Kubali au kataa maombi ya msaada wako.';

  @override
  String get peerOnboarding3Row2Title => 'Mazungumzo';

  @override
  String get peerOnboarding3Row2Body =>
      'Endelea na mazungumzo na unaowasaidia.';

  @override
  String get peerOnboarding3Row3Title => 'Ratiba na Upatikanaji';

  @override
  String get peerOnboarding3Row3Body =>
      'Weka muda unaopatikana na uunde nafasi za msaada.';

  @override
  String get peerOnboarding4Bubble =>
      'Uko tayari! 👋\nAsante kwa kutembea\nna vijana katika\nsafari yao ya afya.';

  @override
  String get peerOnboarding4Row1Title => 'Kuwa mpole';

  @override
  String get peerOnboarding4Row1Body =>
      'Fanya kila mazungumzo yawe salama na ya kirafiki.';

  @override
  String get peerOnboarding4Row2Title => 'Kuwa na heshima';

  @override
  String get peerOnboarding4Row2Body => 'Heshimu chaguo na uzoefu wa kila mtu.';

  @override
  String get peerOnboarding4Row3Title => 'Tunza siri';

  @override
  String get peerOnboarding4Row3Body =>
      'Weka siri yale ambayo watu wanashiriki.';

  @override
  String get peerOnboarding4Title => 'Uko tayari kuanza?';

  @override
  String get peerOnboarding4Body =>
      'Weka upatikanaji wako ili vijana waweze kuomba msaada wako.';

  @override
  String get batteryPromptAllow => 'Ruhusu';

  @override
  String get deliveryNotificationsOffTitle => 'Arifa zimezimwa';

  @override
  String get deliveryNotificationsOffBody =>
      'Ziwashe ili usikose vikumbusho vya mashauriano na ujumbe.';

  @override
  String get deliveryChannelsOffBody =>
      'Baadhi ya aina za arifa zimezimwa, hivyo unaweza kukosa vikumbusho au ujumbe.';

  @override
  String get deliveryBatteryTitle => 'Vikumbusho vinaweza kuchelewa';

  @override
  String get deliveryBatteryBody =>
      'Uokoaji wa betri unaweza kusimamisha programu hii ikiwa nyuma. Iruhusu iendelee ili vikumbusho vifike kwa wakati.';

  @override
  String get deliveryFix => 'Washa';

  @override
  String get onboardingLanguage => 'Lugha';

  @override
  String get onboardingLanguageEnglish => 'Kiingereza';

  @override
  String get onboardingLanguageSwahili => 'Kiswahili';
}
