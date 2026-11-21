import 'notification_taps.dart';
import 'reminder_alarm.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

/// The app's two notification sounds (product owner, 2026-11-17: every
/// notification plays one of these, no exceptions). Files are in
/// `android/app/src/main/res/raw/`, kept from resource shrinking by
/// `res/raw/keep.xml`.
class NotificationSounds {
  const NotificationSounds._();

  /// Looping urgent tone: personal reminders and the 15-minute consultation
  /// reminder. Played on the alarm stream, so it rings on silent/vibrate.
  static const urgent =
      RawResourceAndroidNotificationSound('afya_reminder_urgent');

  /// Short tone: every other notification, on the normal notification stream.
  static const quick = RawResourceAndroidNotificationSound('afya_notify_quick');
}

/// Android notification channels used throughout the app.
///
/// A channel's sound and importance are fixed by Android once the channel
/// exists, so the forced-sound channels (2026-11-17) use new `_V2` ids and
/// [legacyIds] are deleted at start-up. The backend names these same ids in
/// its pushes (`notification_transport.rs::android_channel_id`).
class NotificationChannels {
  const NotificationChannels._();

  static const consultationReminders = AndroidNotificationChannel(
    'CONSULTATION_REMINDERS_V2',
    'Consultation reminders',
    description: 'Reminders shortly before a scheduled consultation starts.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.urgent,
    audioAttributesUsage: AudioAttributesUsage.alarm,
    enableVibration: true,
  );
  static const messages = AndroidNotificationChannel(
    'MESSAGES_V2',
    'Messages',
    description: 'New consultation chat messages.',
    importance: Importance.high,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const appointments = AndroidNotificationChannel(
    'APPOINTMENTS_V2',
    'Appointments',
    description: 'Booking confirmations and consultation outcomes.',
    importance: Importance.high,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const grievance = AndroidNotificationChannel(
    'GRIEVANCE_V2',
    'Grievance updates',
    description: 'Updates on a submitted grievance report.',
    importance: Importance.high,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const general = AndroidNotificationChannel(
    'GENERAL_V2',
    'General',
    description: 'Other account and app notifications.',
    importance: Importance.high,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );

  /// Admin-published announcements (ADMIN_CAMPAIGN), always at maximum
  /// importance (product owner, 2026-11-21). The backend names this id in
  /// its pushes for every role (`notification_transport.rs`).
  static const announcements = AndroidNotificationChannel(
    'ANNOUNCEMENTS_V3',
    'Announcements',
    description: 'Messages from the AfyaMkononi team.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );

  static const all = [
    consultationReminders,
    messages,
    appointments,
    grievance,
    general,
    announcements,
  ];

  /// Channels replaced by the forced-sound set; deleted at start-up so they
  /// no longer show in the phone's notification settings.
  static const legacyIds = [
    'CONSULTATION_REMINDERS',
    'MESSAGES',
    'APPOINTMENTS',
    'GRIEVANCE',
    'GENERAL',
    'patient_reminder_alarms_v1',
  ];

  /// Maps a backend `notification_type` to the channel it should display on.
  /// Falls through to [general] for any type not explicitly listed here —
  /// a new backend notification kind must never crash delivery, just land
  /// somewhere reasonable until this map is extended.
  ///
  /// As of this writing, direct backend source inspection
  /// (`services/backend/src/services/notification_scheduler.rs` and
  /// `notification_service.rs`) confirms exactly 7 real notification_type
  /// values exist, and every one of them currently targets a Doctor's
  /// `user_id` only (`CONSULTATION_REMINDER_15M`, `DAILY_CONSULTATION_SUMMARY`,
  /// `ARTICLE_STATUS_CHANGED`, `REGISTRATION_VERIFIED`, `REGISTRATION_REJECTED`,
  /// `EARNING_CREDITED`, `PAYOUT_PAID`) — none are patient-targeted today.
  /// `GET /notifications` is a real, generic, patient-callable endpoint, so
  /// this mapping stays forward-compatible for when patient-facing types
  /// (e.g. appointment confirmed, prescription issued, grievance resolved)
  /// are added; booking-hold events below are the first patient-targeted
  /// notification family.
  static AndroidNotificationChannel forType(String notificationType) {
    switch (notificationType) {
      case 'CONSULTATION_REMINDER_15M':
        return consultationReminders;
      case 'NEW_CHAT_MESSAGE':
        return messages;
      case 'BOOKING_HOLD_CREATED':
      case 'BOOKING_HOLD_ONE_MINUTE':
      case 'BOOKING_HOLD_EXPIRED':
        return appointments;
      case 'ADMIN_CAMPAIGN':
        return announcements;
      default:
        return general;
    }
  }
}

/// Thin wrapper around `flutter_local_notifications` + `timezone` — the only
/// place in the app that calls either package directly, so every other
/// module (realtime bridging, background worker, schedule reconciler)
/// depends on this single, testable seam instead of the plugin APIs
/// directly.
class LocalNotificationService {
  const LocalNotificationService._();

  static final FlutterLocalNotificationsPlugin _plugin =
      FlutterLocalNotificationsPlugin();
  static bool _initialized = false;
  static Future<void>? _initializing;

  /// Kenya does not observe daylight saving, so a fixed `Africa/Nairobi`
  /// location (rather than the device's own configured timezone) is
  /// deliberately used for every scheduled notification, matching the
  /// Doctor app's convention — a patient whose phone happens to be set to a
  /// different timezone (e.g. while traveling) still gets a consult reminder
  /// anchored to the booked Nairobi-time slot, not local-device time.
  static const String _nairobiLocationName = 'Africa/Nairobi';

  /// Call once at app startup, before any other method here. Idempotent —
  /// a second call is a no-op, so callers (app init, background isolate
  /// entry point) don't need to coordinate who calls it first.
  static Future<void> init() {
    if (_initialized) return Future<void>.value();
    return _initializing ??=
        _initialize().whenComplete(() => _initializing = null);
  }

  static Future<void> _initialize() async {
    tzdata.initializeTimeZones();
    tz.setLocalLocation(tz.getLocation(_nairobiLocationName));
    const androidInit = AndroidInitializationSettings('@mipmap/ic_launcher');
    const initSettings = InitializationSettings(
        android: androidInit,
        iOS: DarwinInitializationSettings(
            requestAlertPermission: false,
            requestBadgePermission: false,
            requestSoundPermission: false));
    await _plugin.initialize(initSettings,
        onDidReceiveNotificationResponse: (response) {
      // Personal reminder alarms are local and never routed to the
      // server-notification destinations (that would open the inbox).
      if (ReminderAlarm.isReminderPayload(response.payload)) {
        handleReminderAlarmAction(response);
        return;
      }
      NotificationTaps.receive(response.payload);
    }, onDidReceiveBackgroundNotificationResponse: handleReminderAlarmAction);
    final launch = await _plugin.getNotificationAppLaunchDetails();
    if (launch?.didNotificationLaunchApp == true &&
        !ReminderAlarm.isReminderPayload(
            launch?.notificationResponse?.payload)) {
      NotificationTaps.receive(launch?.notificationResponse?.payload);
    }
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    for (final id in NotificationChannels.legacyIds) {
      try {
        await androidImpl?.deleteNotificationChannel(id);
      } catch (_) {
        // Already gone (or never created on this device).
      }
    }
    for (final channel in NotificationChannels.all) {
      await androidImpl?.createNotificationChannel(channel);
    }
    _initialized = true;
  }

  /// Android 13+ (API 33) requires this runtime permission before any
  /// notification can be shown. Returns true if granted (or not required on
  /// this OS version/already granted); never throws — a denial is an
  /// ordinary operational state, not an error.
  static Future<bool> requestPermission() async {
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    final granted = await androidImpl?.requestNotificationsPermission() ?? true;
    return granted;
  }

  /// Whether exact alarm-clock scheduling is allowed; asks once (Android 12+
  /// shows the system "Alarms & reminders" setting) when it is not.
  static Future<bool> requestExactAlarms() async {
    if (await ReminderAlarm.exactAlarmsAllowed(_plugin)) return true;
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    await androidImpl?.requestExactAlarmsPermission();
    return ReminderAlarm.exactAlarmsAllowed(_plugin);
  }

  static Future<bool> areNotificationsEnabled() async {
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    return await androidImpl?.areNotificationsEnabled() ?? true;
  }

  /// Displays an immediate device notification. [id] must be one of the
  /// deterministic ids from `notification_ids.dart` so a duplicate discovery
  /// of the same event can never show twice.
  static Future<void> showNow({
    required int id,
    required AndroidNotificationChannel channel,
    required String title,
    required String body,
    String? payload,
  }) async {
    await _plugin.show(
      id,
      title,
      body,
      NotificationDetails(
        // The channel's own sound, repeated here because Android before 8.0
        // (no channels) takes the sound from each notification.
        android: AndroidNotificationDetails(
          channel.id,
          channel.name,
          channelDescription: channel.description,
          importance: channel.importance,
          priority: channel.importance == Importance.max
              ? Priority.max
              : Priority.high,
          playSound: true,
          sound: channel.sound,
          audioAttributesUsage: channel.audioAttributesUsage,
          enableVibration: true,
          // A notice shown on its push and again by the inbox poll (same id)
          // sounds once.
          onlyAlertOnce: true,
        ),
      ),
      payload: payload,
    );
  }
}
