// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get notificationsTitle => 'Notifications';

  @override
  String get notificationsSubtitle => 'Account & activity updates';

  @override
  String get notificationsMarkAllRead => 'Mark all read';

  @override
  String get notificationsLoadError =>
      'Notifications could not be loaded. Please retry.';

  @override
  String get notificationsRefreshError =>
      'Notifications could not be refreshed. Please retry.';

  @override
  String get notificationsEmpty => 'No notifications yet';

  @override
  String get notificationsLoadOlder => 'Load earlier notifications';

  @override
  String get notificationsLoadingOlder => 'Loading earlier notifications...';

  @override
  String get notificationDestinationError =>
      'This destination is unavailable or you no longer have access. Please try again from your inbox.';

  @override
  String get retryButton => 'Retry';

  @override
  String get appointmentTitle => 'Appointment';

  @override
  String get appointmentReference => 'Booking reference';

  @override
  String get refreshButton => 'Refresh';

  @override
  String get doctorLabel => 'Doctor';

  @override
  String get patientLabel => 'Patient';

  @override
  String get restoringSession => 'Restoring secure Doctor session...';

  @override
  String get notificationLanguage => 'Notification language';

  @override
  String get notificationSettingsError =>
      'Could not save notification settings.';

  @override
  String get appointmentStatusPendingPayment => 'Payment pending';

  @override
  String get appointmentStatusConfirmed => 'Confirmed';

  @override
  String get appointmentStatusWaiting => 'Waiting';

  @override
  String get appointmentStatusInConsultation => 'In consultation';

  @override
  String get appointmentStatusCompleted => 'Completed';

  @override
  String get appointmentStatusCancelled => 'Cancelled';

  @override
  String get appointmentStatusExpired => 'Expired';

  @override
  String get appointmentStatusRefunded => 'Refunded';

  @override
  String get appointmentStatusDisputed => 'Under review';

  @override
  String get appointmentStatusPrematureTermination => 'Interrupted';

  @override
  String get cacheOffline => 'Offline — showing saved data';

  @override
  String cacheLastUpdated(String time) {
    return 'Last updated $time';
  }

  @override
  String cacheOfflineUpdated(String time) {
    return 'Offline — showing saved data · Last updated $time';
  }

  @override
  String get settingsTitle => 'Notification settings';

  @override
  String get settingsLoadError => 'Unable to load notification settings.';

  @override
  String get settingsGreetings => 'Greetings';

  @override
  String get settingsGreetingsHelp =>
      'Good-morning, birthday and Kenyan holiday messages';

  @override
  String get settingsReligious => 'Religious holiday greetings';

  @override
  String get settingsReligiousHelp =>
      'Eid, Good Friday, Easter and Christmas messages';

  @override
  String get settingsRoutine => 'Routine updates';

  @override
  String get settingsRoutineHelp =>
      'Article status and other operational notices';

  @override
  String get settingsAnnouncements => 'Announcements';

  @override
  String get settingsAnnouncementsHelp =>
      'Optional engagement and announcement notices';

  @override
  String get settingsQuietHours => 'Quiet hours';

  @override
  String get settingsQuietHoursHelp =>
      'Hold non-urgent notifications during this window. Consultation reminders and time-sensitive alerts are never delayed.';

  @override
  String get settingsStartsAt => 'Starts at';

  @override
  String get settingsEndsAt => 'Ends at';
}
