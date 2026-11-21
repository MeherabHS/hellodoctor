import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_sw.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
      : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
    delegate,
    GlobalMaterialLocalizations.delegate,
    GlobalCupertinoLocalizations.delegate,
    GlobalWidgetsLocalizations.delegate,
  ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('sw')
  ];

  /// No description provided for @batteryPromptTitle.
  ///
  /// In en, this message translates to:
  /// **'Get reminders on time'**
  String get batteryPromptTitle;

  /// No description provided for @batteryPromptBody.
  ///
  /// In en, this message translates to:
  /// **'Your phone may stop this app in the background to save battery. That can delay or block consultation reminders and messages. To get them on time, allow the app to run in the background.'**
  String get batteryPromptBody;

  /// No description provided for @batteryPromptLater.
  ///
  /// In en, this message translates to:
  /// **'Not now'**
  String get batteryPromptLater;

  /// No description provided for @batteryPromptOpenSettings.
  ///
  /// In en, this message translates to:
  /// **'Open settings'**
  String get batteryPromptOpenSettings;

  /// No description provided for @batteryStepsSamsung.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery, and choose Unrestricted. Also check that this app is not under Sleeping apps (Settings > Battery > Background usage limits).'**
  String get batteryStepsSamsung;

  /// No description provided for @batteryStepsTranssion.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery, and allow background running (turn battery optimisation off). In Phone Master, allow this app to auto-start.'**
  String get batteryStepsTranssion;

  /// No description provided for @batteryStepsXiaomi.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery saver, and choose No restrictions. Then turn on Autostart.'**
  String get batteryStepsXiaomi;

  /// No description provided for @batteryStepsOppo.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery, and turn on Allow background activity. Also allow Auto launch.'**
  String get batteryStepsOppo;

  /// No description provided for @batteryStepsVivo.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery, and allow high background power consumption. Also allow Auto-start.'**
  String get batteryStepsVivo;

  /// No description provided for @batteryStepsHuawei.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery, then App launch. Set this app to Manage manually and turn on all three switches.'**
  String get batteryStepsHuawei;

  /// No description provided for @batteryStepsOther.
  ///
  /// In en, this message translates to:
  /// **'Tap Open settings, then Battery, and choose Unrestricted (or Don\'t optimise).'**
  String get batteryStepsOther;

  /// No description provided for @chatReceiptSending.
  ///
  /// In en, this message translates to:
  /// **'Sending'**
  String get chatReceiptSending;

  /// No description provided for @chatReceiptSent.
  ///
  /// In en, this message translates to:
  /// **'Sent'**
  String get chatReceiptSent;

  /// No description provided for @chatReceiptRead.
  ///
  /// In en, this message translates to:
  /// **'Read'**
  String get chatReceiptRead;

  /// No description provided for @notificationsTitle.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notificationsTitle;

  /// No description provided for @notificationsSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Account & activity updates'**
  String get notificationsSubtitle;

  /// No description provided for @notificationsMarkAllRead.
  ///
  /// In en, this message translates to:
  /// **'Mark all read'**
  String get notificationsMarkAllRead;

  /// No description provided for @notificationsLoadError.
  ///
  /// In en, this message translates to:
  /// **'Notifications could not be loaded. Please retry.'**
  String get notificationsLoadError;

  /// No description provided for @notificationsRefreshError.
  ///
  /// In en, this message translates to:
  /// **'Notifications could not be refreshed. Please retry.'**
  String get notificationsRefreshError;

  /// No description provided for @notificationsEmpty.
  ///
  /// In en, this message translates to:
  /// **'No notifications yet'**
  String get notificationsEmpty;

  /// No description provided for @notificationsLoadOlder.
  ///
  /// In en, this message translates to:
  /// **'Load earlier notifications'**
  String get notificationsLoadOlder;

  /// No description provided for @notificationsLoadingOlder.
  ///
  /// In en, this message translates to:
  /// **'Loading earlier notifications...'**
  String get notificationsLoadingOlder;

  /// No description provided for @notificationDestinationError.
  ///
  /// In en, this message translates to:
  /// **'This destination is unavailable or you no longer have access. Please try again from your inbox.'**
  String get notificationDestinationError;

  /// No description provided for @retryButton.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retryButton;

  /// No description provided for @appointmentTitle.
  ///
  /// In en, this message translates to:
  /// **'Appointment'**
  String get appointmentTitle;

  /// No description provided for @appointmentReference.
  ///
  /// In en, this message translates to:
  /// **'Booking reference'**
  String get appointmentReference;

  /// No description provided for @refreshButton.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refreshButton;

  /// No description provided for @doctorLabel.
  ///
  /// In en, this message translates to:
  /// **'Doctor'**
  String get doctorLabel;

  /// No description provided for @patientLabel.
  ///
  /// In en, this message translates to:
  /// **'Patient'**
  String get patientLabel;

  /// No description provided for @restoringSession.
  ///
  /// In en, this message translates to:
  /// **'Restoring secure Doctor session...'**
  String get restoringSession;

  /// No description provided for @notificationLanguage.
  ///
  /// In en, this message translates to:
  /// **'Notification language'**
  String get notificationLanguage;

  /// No description provided for @notificationSettingsError.
  ///
  /// In en, this message translates to:
  /// **'Could not save notification settings.'**
  String get notificationSettingsError;

  /// No description provided for @appointmentStatusPendingPayment.
  ///
  /// In en, this message translates to:
  /// **'Payment pending'**
  String get appointmentStatusPendingPayment;

  /// No description provided for @appointmentStatusConfirmed.
  ///
  /// In en, this message translates to:
  /// **'Confirmed'**
  String get appointmentStatusConfirmed;

  /// No description provided for @appointmentStatusWaiting.
  ///
  /// In en, this message translates to:
  /// **'Waiting'**
  String get appointmentStatusWaiting;

  /// No description provided for @appointmentStatusInConsultation.
  ///
  /// In en, this message translates to:
  /// **'In consultation'**
  String get appointmentStatusInConsultation;

  /// No description provided for @appointmentStatusCompleted.
  ///
  /// In en, this message translates to:
  /// **'Completed'**
  String get appointmentStatusCompleted;

  /// No description provided for @appointmentStatusCancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get appointmentStatusCancelled;

  /// No description provided for @appointmentStatusLapsed.
  ///
  /// In en, this message translates to:
  /// **'Time passed'**
  String get appointmentStatusLapsed;

  /// No description provided for @appointmentStatusExpired.
  ///
  /// In en, this message translates to:
  /// **'Expired'**
  String get appointmentStatusExpired;

  /// No description provided for @appointmentStatusRefunded.
  ///
  /// In en, this message translates to:
  /// **'Refunded'**
  String get appointmentStatusRefunded;

  /// No description provided for @appointmentStatusDisputed.
  ///
  /// In en, this message translates to:
  /// **'Under review'**
  String get appointmentStatusDisputed;

  /// No description provided for @appointmentStatusPrematureTermination.
  ///
  /// In en, this message translates to:
  /// **'Interrupted'**
  String get appointmentStatusPrematureTermination;

  /// No description provided for @cacheOffline.
  ///
  /// In en, this message translates to:
  /// **'Offline — showing saved data'**
  String get cacheOffline;

  /// No description provided for @cacheLastUpdated.
  ///
  /// In en, this message translates to:
  /// **'Last updated {time}'**
  String cacheLastUpdated(String time);

  /// No description provided for @cacheOfflineUpdated.
  ///
  /// In en, this message translates to:
  /// **'Offline — showing saved data · Last updated {time}'**
  String cacheOfflineUpdated(String time);

  /// No description provided for @settingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Notification settings'**
  String get settingsTitle;

  /// No description provided for @settingsLoadError.
  ///
  /// In en, this message translates to:
  /// **'Unable to load notification settings.'**
  String get settingsLoadError;

  /// No description provided for @settingsGreetings.
  ///
  /// In en, this message translates to:
  /// **'Greetings'**
  String get settingsGreetings;

  /// No description provided for @settingsGreetingsHelp.
  ///
  /// In en, this message translates to:
  /// **'Good-morning, birthday and Kenyan holiday messages'**
  String get settingsGreetingsHelp;

  /// No description provided for @settingsReligious.
  ///
  /// In en, this message translates to:
  /// **'Religious holiday greetings'**
  String get settingsReligious;

  /// No description provided for @settingsReligiousHelp.
  ///
  /// In en, this message translates to:
  /// **'Eid, Good Friday, Easter and Christmas messages'**
  String get settingsReligiousHelp;

  /// No description provided for @settingsRoutine.
  ///
  /// In en, this message translates to:
  /// **'Routine updates'**
  String get settingsRoutine;

  /// No description provided for @settingsRoutineHelp.
  ///
  /// In en, this message translates to:
  /// **'Article status and other operational notices'**
  String get settingsRoutineHelp;

  /// No description provided for @settingsAnnouncements.
  ///
  /// In en, this message translates to:
  /// **'Announcements'**
  String get settingsAnnouncements;

  /// No description provided for @settingsAnnouncementsHelp.
  ///
  /// In en, this message translates to:
  /// **'Optional engagement and announcement notices'**
  String get settingsAnnouncementsHelp;

  /// No description provided for @settingsQuietHours.
  ///
  /// In en, this message translates to:
  /// **'Quiet hours'**
  String get settingsQuietHours;

  /// No description provided for @settingsQuietHoursHelp.
  ///
  /// In en, this message translates to:
  /// **'Hold non-urgent notifications during this window. Consultation reminders and time-sensitive alerts are never delayed.'**
  String get settingsQuietHoursHelp;

  /// No description provided for @settingsStartsAt.
  ///
  /// In en, this message translates to:
  /// **'Starts at'**
  String get settingsStartsAt;

  /// No description provided for @settingsEndsAt.
  ///
  /// In en, this message translates to:
  /// **'Ends at'**
  String get settingsEndsAt;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get notificationsToday;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'Yesterday'**
  String get notificationsYesterday;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'Earlier'**
  String get notificationsEarlier;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'All'**
  String get notificationsFilterAll;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'Visits'**
  String get notificationsFilterAppointments;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'Messages'**
  String get notificationsFilterMessages;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get notificationsFilterSystem;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'Just now'**
  String get notificationsJustNow;

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'{count} min ago'**
  String notificationsMinutesAgo(int count);

  /// Notification inbox (Peer Navigator design)
  ///
  /// In en, this message translates to:
  /// **'{count} h ago'**
  String notificationsHoursAgo(int count);

  /// Leaves the doctor onboarding
  ///
  /// In en, this message translates to:
  /// **'Skip'**
  String get doctorOnboardingSkip;

  /// Doctor onboarding: previous slide
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get doctorOnboardingBack;

  /// Doctor onboarding: next slide
  ///
  /// In en, this message translates to:
  /// **'Next'**
  String get doctorOnboardingNext;

  /// Doctor onboarding: slide 1 button
  ///
  /// In en, this message translates to:
  /// **'Get Started'**
  String get doctorOnboardingGetStarted;

  /// Doctor onboarding: last button; the doctor is already signed in
  ///
  /// In en, this message translates to:
  /// **'Go to Dashboard'**
  String get doctorOnboardingFinish;

  /// Page dots label for screen readers
  ///
  /// In en, this message translates to:
  /// **'{current} of {total}'**
  String doctorOnboardingCounter(int current, int total);

  /// Slide 1 title; the waving hand follows it
  ///
  /// In en, this message translates to:
  /// **'Welcome, Doctor!'**
  String get doctorOnboarding1Title;

  /// Slide 1 body; line breaks as on the board
  ///
  /// In en, this message translates to:
  /// **'Join AfyaMkononi to provide\nquality, confidential and\nyouth-friendly care.'**
  String get doctorOnboarding1Body;

  /// Slide 2 title
  ///
  /// In en, this message translates to:
  /// **'Provide Care Online'**
  String get doctorOnboarding2Title;

  /// Slide 2 body
  ///
  /// In en, this message translates to:
  /// **'Consult patients through\nsecure video or chat, give\nadvice, and support\nfollow-up care.'**
  String get doctorOnboarding2Body;

  /// Slide 3 title
  ///
  /// In en, this message translates to:
  /// **'Manage Appointments'**
  String get doctorOnboarding3Title;

  /// Slide 3 body
  ///
  /// In en, this message translates to:
  /// **'View and manage your\nconsultation schedule easily,\nwith timely notifications\nand reminders.'**
  String get doctorOnboarding3Body;

  /// Slide 4 title
  ///
  /// In en, this message translates to:
  /// **'You’re All Set!'**
  String get doctorOnboarding4Title;

  /// Slide 4 body (signed-in wording)
  ///
  /// In en, this message translates to:
  /// **'Start providing trusted,\nconfidential and\nadolescent-friendly care\non AfyaMkononi.'**
  String get doctorOnboarding4Body;

  /// AfyaMkononi tagline; parts separated by ·
  ///
  /// In en, this message translates to:
  /// **'Prevent · Test · Treat · Stay Healthy'**
  String get brandTagline;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Skip'**
  String get peerOnboardingSkip;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Next'**
  String get peerOnboardingNext;

  /// Peer onboarding: previous slide (screen-reader label of the back arrow)
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get peerOnboardingBack;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Get Started'**
  String get peerOnboardingGetStarted;

  /// Peer onboarding last button; opens the peer Home
  ///
  /// In en, this message translates to:
  /// **'Go to My Dashboard'**
  String get peerOnboardingFinish;

  /// Page dots label for screen readers
  ///
  /// In en, this message translates to:
  /// **'{current} of {total}'**
  String peerOnboardingCounter(int current, int total);

  /// Slide 1 speech bubble
  ///
  /// In en, this message translates to:
  /// **'Hi there! 👋\nWelcome to\nAfyaMkononi!'**
  String get peerOnboarding1Bubble;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Welcome, Peer Navigator'**
  String get peerOnboarding1Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'You are a trusted peer for young people in your community. Share accurate information, offer support and help them reach care in a safe and friendly way.'**
  String get peerOnboarding1Body;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Support\nYour Peers'**
  String get peerOnboarding1Tile1;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Share\nTrusted Info'**
  String get peerOnboarding1Tile2;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Encourage\nCare'**
  String get peerOnboarding1Tile3;

  /// Slide 2 speech bubble
  ///
  /// In en, this message translates to:
  /// **'You’re not alone.\nLet’s talk. I’m here\nto listen and support! 💚'**
  String get peerOnboarding2Bubble;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Be a Supportive Peer'**
  String get peerOnboarding2Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Chat with young people, offer encouragement and share positive experiences in a safe and respectful space.'**
  String get peerOnboarding2Body;

  /// Slide 3 speech bubble
  ///
  /// In en, this message translates to:
  /// **'Here’s what you can\ndo on your peer\ndashboard!'**
  String get peerOnboarding3Bubble;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Your Peer Dashboard'**
  String get peerOnboarding3Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Everything you need to support others, in one place.'**
  String get peerOnboarding3Body;

  /// Matches the Requests tab
  ///
  /// In en, this message translates to:
  /// **'Support Requests'**
  String get peerOnboarding3Row1Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Accept or decline requests for your support.'**
  String get peerOnboarding3Row1Body;

  /// Matches the Chats tab
  ///
  /// In en, this message translates to:
  /// **'Chats'**
  String get peerOnboarding3Row2Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Continue conversations with the people you support.'**
  String get peerOnboarding3Row2Body;

  /// Matches the Schedule tab
  ///
  /// In en, this message translates to:
  /// **'Schedule & Availability'**
  String get peerOnboarding3Row3Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Set when you are available and create support slots.'**
  String get peerOnboarding3Row3Body;

  /// Slide 4 speech bubble
  ///
  /// In en, this message translates to:
  /// **'You’re ready! 👋\nThank you for walking\nwith young people on\ntheir health journey.'**
  String get peerOnboarding4Bubble;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Be kind'**
  String get peerOnboarding4Row1Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Make every chat a safe and friendly space.'**
  String get peerOnboarding4Row1Body;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Be respectful'**
  String get peerOnboarding4Row2Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Respect each person’s choices and experiences.'**
  String get peerOnboarding4Row2Body;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Stay confidential'**
  String get peerOnboarding4Row3Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Keep what people share private and protected.'**
  String get peerOnboarding4Row3Body;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Ready to begin?'**
  String get peerOnboarding4Title;

  /// Peer Navigator onboarding
  ///
  /// In en, this message translates to:
  /// **'Set your availability so young people can request your support.'**
  String get peerOnboarding4Body;

  /// Opens Android's 'always run in background' dialog
  ///
  /// In en, this message translates to:
  /// **'Allow'**
  String get batteryPromptAllow;

  /// Notification and battery readiness
  ///
  /// In en, this message translates to:
  /// **'Notifications are off'**
  String get deliveryNotificationsOffTitle;

  /// Notification and battery readiness
  ///
  /// In en, this message translates to:
  /// **'Turn them on so you don\'t miss consultation reminders and messages.'**
  String get deliveryNotificationsOffBody;

  /// Notification and battery readiness
  ///
  /// In en, this message translates to:
  /// **'Some notification types are switched off, so you may miss reminders or messages.'**
  String get deliveryChannelsOffBody;

  /// Notification and battery readiness
  ///
  /// In en, this message translates to:
  /// **'Reminders may be delayed'**
  String get deliveryBatteryTitle;

  /// Notification and battery readiness
  ///
  /// In en, this message translates to:
  /// **'Battery saving can stop this app in the background. Allow it to run so reminders arrive on time.'**
  String get deliveryBatteryBody;

  /// Notification and battery readiness
  ///
  /// In en, this message translates to:
  /// **'Turn on'**
  String get deliveryFix;

  /// Label of the EN | SW switch beside Skip on onboarding.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get onboardingLanguage;

  /// Spoken name of the EN side of the onboarding language switch.
  ///
  /// In en, this message translates to:
  /// **'English'**
  String get onboardingLanguageEnglish;

  /// Spoken name of the SW side of the onboarding language switch.
  ///
  /// In en, this message translates to:
  /// **'Kiswahili'**
  String get onboardingLanguageSwahili;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'sw'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'sw':
      return AppLocalizationsSw();
  }

  throw FlutterError(
      'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
      'an issue with the localizations generation tool. Please file an issue '
      'on GitHub with a reproducible sample app and the gen-l10n configuration '
      'that was used.');
}
