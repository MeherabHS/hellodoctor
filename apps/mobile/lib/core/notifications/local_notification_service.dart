import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

/// Android notification channels used throughout the app. Fixed, meaningful
/// channels per docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md — never one
/// generic channel, and never everything marked max-importance.
class NotificationChannels {
  const NotificationChannels._();

  static const consultationReminders = AndroidNotificationChannel(
    'CONSULTATION_REMINDERS',
    'Consultation reminders',
    description: 'Reminders shortly before a scheduled consultation starts.',
    importance: Importance.high,
  );
  static const dailySchedule = AndroidNotificationChannel(
    'DAILY_SCHEDULE',
    'Daily schedule summary',
    description: 'Your consultation count at the start of each day.',
    importance: Importance.defaultImportance,
  );
  static const messages = AndroidNotificationChannel(
    'MESSAGES',
    'Messages',
    description: 'New consultation chat messages.',
    importance: Importance.high,
  );
  static const account = AndroidNotificationChannel(
    'ACCOUNT',
    'Account',
    description: 'Registration and verification updates.',
    importance: Importance.defaultImportance,
  );
  static const financial = AndroidNotificationChannel(
    'FINANCIAL',
    'Earnings & payouts',
    description: 'Earning credited and payout confirmations.',
    importance: Importance.defaultImportance,
  );
  static const general = AndroidNotificationChannel(
    'GENERAL',
    'General',
    description: 'Other account and app notifications.',
    importance: Importance.low,
  );

  static const all = [
    consultationReminders,
    dailySchedule,
    messages,
    account,
    financial,
    general,
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
      default:
        return general;
    }
  }
}

/// Thin wrapper around `flutter_local_notifications` + `timezone` — the only
/// place in the app that calls either package directly, so every other
/// Layer 1/2/3 module (realtime bridging, background worker, schedule
/// reconciler) depends on this single, testable seam instead of the plugin
/// APIs directly.
class LocalNotificationService {
  const LocalNotificationService._();

  static final FlutterLocalNotificationsPlugin _plugin =
      FlutterLocalNotificationsPlugin();
  static bool _initialized = false;

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
  static Future<void> init() async {
    if (_initialized) return;
    tzdata.initializeTimeZones();
    tz.setLocalLocation(tz.getLocation(_nairobiLocationName));
    const androidInit = AndroidInitializationSettings('@mipmap/ic_launcher');
    const initSettings = InitializationSettings(android: androidInit);
    await _plugin.initialize(initSettings);
    final androidImpl = _plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
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
        android: AndroidNotificationDetails(
          channel.id,
          channel.name,
          channelDescription: channel.description,
          importance: channel.importance,
          priority: channel.importance == Importance.high
              ? Priority.high
              : Priority.defaultPriority,
        ),
      ),
      payload: payload,
    );
  }

  /// Schedules a future device notification — used for Layer 3's daily
  /// summary and T-15 reminders. Deliberately uses
  /// [AndroidScheduleMode.inexactAllowWhileIdle], not an exact alarm: the
  /// spec itself requires treating this as best-effort ("do NOT claim an
  /// exact execution time"; Doze/battery optimization may defer it), and an
  /// exact alarm on Android 12+ additionally requires the special
  /// `SCHEDULE_EXACT_ALARM`/`USE_EXACT_ALARM` access, which this
  /// architecture deliberately avoids needing. If [whenUtc] is already in
  /// the past, the plugin fires it immediately — acceptable for a
  /// reconciliation pass that runs slightly late.
  static Future<void> scheduleAt({
    required int id,
    required AndroidNotificationChannel channel,
    required DateTime whenUtc,
    required String title,
    required String body,
    String? payload,
  }) async {
    await _plugin.zonedSchedule(
      id,
      title,
      body,
      tz.TZDateTime.from(whenUtc, tz.local),
      NotificationDetails(
        android: AndroidNotificationDetails(
          channel.id,
          channel.name,
          channelDescription: channel.description,
          importance: channel.importance,
          priority: channel.importance == Importance.high
              ? Priority.high
              : Priority.defaultPriority,
        ),
      ),
      androidScheduleMode: AndroidScheduleMode.inexactAllowWhileIdle,
      uiLocalNotificationDateInterpretation:
          UILocalNotificationDateInterpretation.absoluteTime,
      payload: payload,
    );
  }

  static Future<void> cancel(int id) => _plugin.cancel(id);

  /// The set of ids currently scheduled-but-not-yet-fired — the schedule
  /// reconciler's "what already exists" input, so it can diff against the
  /// authoritative backend schedule and cancel exactly the stale ones
  /// (RESCHEDULED/CANCELLED/NO LONGER AUTHORIZED/COMPLETED) without ever
  /// touching ones that are still current.
  static Future<Set<int>> pendingIds() async {
    final pending = await _plugin.pendingNotificationRequests();
    return pending.map((p) => p.id).toSet();
  }

  /// Same as [pendingIds], but keyed by each request's own `payload` (the
  /// original deterministic string key passed to [scheduleAt]) rather than
  /// its hashed int id. A hashed id can't be reversed back into the key that
  /// produced it, so this is what lets the reconciler find and cancel a
  /// pending request whose *key* no longer belongs to the current
  /// should-exist set (e.g. a stale `consultation_t15:<old-appointment-id>`
  /// left over from a cancelled appointment) — not just ones it already
  /// knows the id for. A request scheduled by something other than this
  /// service (or on a plugin version where [payload] came back null) is
  /// skipped rather than guessed at.
  static Future<Map<int, String>> pendingByPayload() async {
    final pending = await _plugin.pendingNotificationRequests();
    return {
      for (final p in pending)
        if (p.payload != null) p.id: p.payload!,
    };
  }
}
