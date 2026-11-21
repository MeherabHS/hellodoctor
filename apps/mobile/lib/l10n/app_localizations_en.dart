// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get batteryPromptTitle => 'Get reminders on time';

  @override
  String get batteryPromptBody =>
      'Your phone may stop this app in the background to save battery. That can delay or block consultation reminders and messages. To get them on time, allow the app to run in the background.';

  @override
  String get batteryPromptLater => 'Not now';

  @override
  String get batteryPromptOpenSettings => 'Open settings';

  @override
  String get batteryStepsSamsung =>
      'Tap Open settings, then Battery, and choose Unrestricted. Also check that this app is not under Sleeping apps (Settings > Battery > Background usage limits).';

  @override
  String get batteryStepsTranssion =>
      'Tap Open settings, then Battery, and allow background running (turn battery optimisation off). In Phone Master, allow this app to auto-start.';

  @override
  String get batteryStepsXiaomi =>
      'Tap Open settings, then Battery saver, and choose No restrictions. Then turn on Autostart.';

  @override
  String get batteryStepsOppo =>
      'Tap Open settings, then Battery, and turn on Allow background activity. Also allow Auto launch.';

  @override
  String get batteryStepsVivo =>
      'Tap Open settings, then Battery, and allow high background power consumption. Also allow Auto-start.';

  @override
  String get batteryStepsHuawei =>
      'Tap Open settings, then Battery, then App launch. Set this app to Manage manually and turn on all three switches.';

  @override
  String get batteryStepsOther =>
      'Tap Open settings, then Battery, and choose Unrestricted (or Don\'t optimise).';

  @override
  String get chatReceiptSending => 'Sending';

  @override
  String get chatReceiptSent => 'Sent';

  @override
  String get chatReceiptRead => 'Read';

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
  String get appointmentStatusLapsed => 'Time passed';

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

  @override
  String get notificationsToday => 'Today';

  @override
  String get notificationsYesterday => 'Yesterday';

  @override
  String get notificationsEarlier => 'Earlier';

  @override
  String get notificationsFilterAll => 'All';

  @override
  String get notificationsFilterAppointments => 'Visits';

  @override
  String get notificationsFilterMessages => 'Messages';

  @override
  String get notificationsFilterSystem => 'System';

  @override
  String get notificationsJustNow => 'Just now';

  @override
  String notificationsMinutesAgo(int count) {
    return '$count min ago';
  }

  @override
  String notificationsHoursAgo(int count) {
    return '$count h ago';
  }

  @override
  String get doctorOnboardingSkip => 'Skip';

  @override
  String get doctorOnboardingBack => 'Back';

  @override
  String get doctorOnboardingNext => 'Next';

  @override
  String get doctorOnboardingGetStarted => 'Get Started';

  @override
  String get doctorOnboardingFinish => 'Go to Dashboard';

  @override
  String doctorOnboardingCounter(int current, int total) {
    return '$current of $total';
  }

  @override
  String get doctorOnboarding1Title => 'Welcome, Doctor!';

  @override
  String get doctorOnboarding1Body =>
      'Join AfyaMkononi to provide\nquality, confidential and\nyouth-friendly care.';

  @override
  String get doctorOnboarding2Title => 'Provide Care Online';

  @override
  String get doctorOnboarding2Body =>
      'Consult patients through\nsecure video or chat, give\nadvice, and support\nfollow-up care.';

  @override
  String get doctorOnboarding3Title => 'Manage Appointments';

  @override
  String get doctorOnboarding3Body =>
      'View and manage your\nconsultation schedule easily,\nwith timely notifications\nand reminders.';

  @override
  String get doctorOnboarding4Title => 'You’re All Set!';

  @override
  String get doctorOnboarding4Body =>
      'Start providing trusted,\nconfidential and\nadolescent-friendly care\non AfyaMkononi.';

  @override
  String get brandTagline => 'Prevent · Test · Treat · Stay Healthy';

  @override
  String get peerOnboardingSkip => 'Skip';

  @override
  String get peerOnboardingNext => 'Next';

  @override
  String get peerOnboardingBack => 'Back';

  @override
  String get peerOnboardingGetStarted => 'Get Started';

  @override
  String get peerOnboardingFinish => 'Go to My Dashboard';

  @override
  String peerOnboardingCounter(int current, int total) {
    return '$current of $total';
  }

  @override
  String get peerOnboarding1Bubble => 'Hi there! 👋\nWelcome to\nAfyaMkononi!';

  @override
  String get peerOnboarding1Title => 'Welcome, Peer Navigator';

  @override
  String get peerOnboarding1Body =>
      'You are a trusted peer for young people in your community. Share accurate information, offer support and help them reach care in a safe and friendly way.';

  @override
  String get peerOnboarding1Tile1 => 'Support\nYour Peers';

  @override
  String get peerOnboarding1Tile2 => 'Share\nTrusted Info';

  @override
  String get peerOnboarding1Tile3 => 'Encourage\nCare';

  @override
  String get peerOnboarding2Bubble =>
      'You’re not alone.\nLet’s talk. I’m here\nto listen and support! 💚';

  @override
  String get peerOnboarding2Title => 'Be a Supportive Peer';

  @override
  String get peerOnboarding2Body =>
      'Chat with young people, offer encouragement and share positive experiences in a safe and respectful space.';

  @override
  String get peerOnboarding3Bubble =>
      'Here’s what you can\ndo on your peer\ndashboard!';

  @override
  String get peerOnboarding3Title => 'Your Peer Dashboard';

  @override
  String get peerOnboarding3Body =>
      'Everything you need to support others, in one place.';

  @override
  String get peerOnboarding3Row1Title => 'Support Requests';

  @override
  String get peerOnboarding3Row1Body =>
      'Accept or decline requests for your support.';

  @override
  String get peerOnboarding3Row2Title => 'Chats';

  @override
  String get peerOnboarding3Row2Body =>
      'Continue conversations with the people you support.';

  @override
  String get peerOnboarding3Row3Title => 'Schedule & Availability';

  @override
  String get peerOnboarding3Row3Body =>
      'Set when you are available and create support slots.';

  @override
  String get peerOnboarding4Bubble =>
      'You’re ready! 👋\nThank you for walking\nwith young people on\ntheir health journey.';

  @override
  String get peerOnboarding4Row1Title => 'Be kind';

  @override
  String get peerOnboarding4Row1Body =>
      'Make every chat a safe and friendly space.';

  @override
  String get peerOnboarding4Row2Title => 'Be respectful';

  @override
  String get peerOnboarding4Row2Body =>
      'Respect each person’s choices and experiences.';

  @override
  String get peerOnboarding4Row3Title => 'Stay confidential';

  @override
  String get peerOnboarding4Row3Body =>
      'Keep what people share private and protected.';

  @override
  String get peerOnboarding4Title => 'Ready to begin?';

  @override
  String get peerOnboarding4Body =>
      'Set your availability so young people can request your support.';

  @override
  String get batteryPromptAllow => 'Allow';

  @override
  String get deliveryNotificationsOffTitle => 'Notifications are off';

  @override
  String get deliveryNotificationsOffBody =>
      'Turn them on so you don\'t miss consultation reminders and messages.';

  @override
  String get deliveryChannelsOffBody =>
      'Some notification types are switched off, so you may miss reminders or messages.';

  @override
  String get deliveryBatteryTitle => 'Reminders may be delayed';

  @override
  String get deliveryBatteryBody =>
      'Battery saving can stop this app in the background. Allow it to run so reminders arrive on time.';

  @override
  String get deliveryFix => 'Turn on';

  @override
  String get onboardingLanguage => 'Language';

  @override
  String get onboardingLanguageEnglish => 'English';

  @override
  String get onboardingLanguageSwahili => 'Kiswahili';
}
