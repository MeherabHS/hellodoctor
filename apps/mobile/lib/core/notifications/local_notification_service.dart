import 'notification_taps.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

/// The app's two notification sounds (product owner, 2026-11-17: every
/// notification plays one of these, no exceptions). Files are in
/// `android/app/src/main/res/raw/`, kept from resource shrinking by
/// `res/raw/keep.xml`.
class NotificationSounds {
  const NotificationSounds._();

  /// Urgent tone: the 15-minute consultation reminder. Played on the alarm
  /// stream, so it rings on silent/vibrate.
  static const urgent =
      RawResourceAndroidNotificationSound('afya_reminder_urgent');

  /// Short tone: every other notification, on the normal notification stream.
  static const quick = RawResourceAndroidNotificationSound('afya_notify_quick');
}

/// Android notification channels used throughout the app, per
/// docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md.
///
/// A channel's sound and importance are fixed by Android once the channel
/// exists. The forced-sound channels (2026-11-17) became `_V2`; on
/// 2026-11-20 every channel was raised to max importance (product-owner
/// instruction), hence the `_V3` ids. [legacyIds] are deleted at start-up.
/// The backend names these same ids in its pushes
/// (`notification_transport.rs::android_channel_id`).
class NotificationChannels {
  const NotificationChannels._();

  static const consultationReminders = AndroidNotificationChannel(
    'CONSULTATION_REMINDERS_V3',
    'Consultation reminders',
    description: 'Reminders shortly before a scheduled consultation starts.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.urgent,
    audioAttributesUsage: AudioAttributesUsage.alarm,
    enableVibration: true,
  );
  static const dailySchedule = AndroidNotificationChannel(
    'DAILY_SCHEDULE_V3',
    'Daily schedule summary',
    description: 'Your consultation count at the start of each day.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const messages = AndroidNotificationChannel(
    'MESSAGES_V3',
    'Messages',
    description: 'New consultation chat messages.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const account = AndroidNotificationChannel(
    'ACCOUNT_V3',
    'Account',
    description: 'Registration and verification updates.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const financial = AndroidNotificationChannel(
    'FINANCIAL_V3',
    'Earnings & payouts',
    description: 'Earning credited and payout confirmations.',
    importance: Importance.max,
    playSound: true,
    sound: NotificationSounds.quick,
    enableVibration: true,
  );
  static const general = AndroidNotificationChannel(
    'GENERAL_V3',
    'General',
    description: 'Other account and app notifications.',
    importance: Importance.max,
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
    dailySchedule,
    messages,
    account,
    financial,
    general,
    announcements,
  ];

  /// Channels replaced by the forced-sound set; deleted at start-up so they
  /// no longer show in the phone's notification settings.
  static const legacyIds = [
    'CONSULTATION_REMINDERS_V2',
    'DAILY_SCHEDULE_V2',
    'MESSAGES_V2',
    'ACCOUNT_V2',
    'FINANCIAL_V2',
    'GENERAL_V2',
    'CONSULTATION_REMINDERS',
    'DAILY_SCHEDULE',
    'MESSAGES',
    'ACCOUNT',
    'FINANCIAL',
    'GENERAL',
  ];

  /// Maps a backend `notification_type` (see
  /// `services/backend/src/services/notification_service.rs` and the
  /// scheduler's `DAILY_CONSULTATION_SUMMARY`/`CONSULTATION_REMINDER_15M`)
  /// to the channel it should display on. Falls through to [general] for any
  /// type not explicitly listed here, rather than failing — a new backend
  /// notification kind must never crash delivery, just land somewhere
  /// reasonable until this map is extended.
  static AndroidNotificationChannel forType(String notificationType) {
    switch (notificationType) {
      case 'CONSULTATION_REMINDER_15M':
        return consultationReminders;
      case 'DAILY_CONSULTATION_SUMMARY':
        return dailySchedule;
      case 'NEW_CHAT_MESSAGE':
        return messages;
      case 'REGISTRATION_VERIFIED':
      case 'REGISTRATION_REJECTED':
      case 'ARTICLE_STATUS_CHANGED':
        return account;
      case 'EARNING_CREDITED':
      case 'PAYOUT_PAID':
        return financial;
      case 'ADMIN_CAMPAIGN':
        return announcements;
      default:
        return general;
    }
  }
}

/// Thin wrapper around `flutter_local_notifications` + `timezone` — the only
/// place in the app that calls either package directly, so every other
/// Layer 1/2 module (realtime bridging, background worker) depends on this
/// single, testable seam instead of the plugin APIs directly.
class LocalNotificationService {
  const LocalNotificationService._();

  static final FlutterLocalNotificationsPlugin _plugin =
      FlutterLocalNotificationsPlugin();
  static bool _initialized = false;
  static Future<void>? _initializing;

  /// Kenya does not observe daylight saving, so a fixed `Africa/Nairobi`
  /// location (rather than the device's own configured timezone) is
  /// deliberately used for every scheduled notification — matching the
  /// backend scheduler's own fixed-UTC+3 `nairobi_offset()` exactly, so a
  /// doctor whose phone happens to be set to a different timezone (e.g.
  /// while traveling) still gets the daily summary at 7am Nairobi time, not
  /// 7am local-device time.
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
        onDidReceiveNotificationResponse: (response) =>
            NotificationTaps.receive(response.payload));
    final launch = await _plugin.getNotificationAppLaunchDetails();
    if (launch?.didNotificationLaunchApp == true) {
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
  /// ordinary operational state (see spec §18: "no crash, explain in
  /// Settings"), not an error.
  static Future<bool> requestPermission() async {
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    final granted = await androidImpl?.requestNotificationsPermission() ?? true;
    return granted;
  }

  static Future<bool> areNotificationsEnabled() async {
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    return await androidImpl?.areNotificationsEnabled() ?? true;
  }

  /// Displays an immediate device notification — used for Layer 1/2's
  /// "a new backend event was just discovered" case. [id] must be one of the
  /// deterministic ids from `notification_ids.dart` so a duplicate
  /// discovery of the same event can never show twice.
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
          priority: Priority.max,
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
