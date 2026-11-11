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
