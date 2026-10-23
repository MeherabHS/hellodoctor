import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

/// Android notification channels used throughout the app. Fixed, meaningful
/// channels mirroring `apps/mobile`'s Doctor-side design — never one generic
/// channel, and never everything marked max-importance.
class NotificationChannels {
  const NotificationChannels._();

  static const consultationReminders = AndroidNotificationChannel(
    'CONSULTATION_REMINDERS',
    'Consultation reminders',
    description: 'Reminders shortly before a scheduled consultation starts.',
    importance: Importance.high,
  );
  static const messages = AndroidNotificationChannel(
    'MESSAGES',
    'Messages',
    description: 'New consultation chat messages.',
    importance: Importance.high,
  );
  static const appointments = AndroidNotificationChannel(
    'APPOINTMENTS',
    'Appointments',
    description: 'Booking confirmations and consultation outcomes.',
    importance: Importance.high,
  );
  static const grievance = AndroidNotificationChannel(
    'GRIEVANCE',
    'Grievance updates',
    description: 'Updates on a submitted grievance report.',
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
    messages,
    appointments,
    grievance,
    general,
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
  /// ordinary operational state, not an error.
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

  /// Schedules a future device notification — used for the client-side
  /// consult-reminder (view-2's "Trigger Consultation Reminder" demo, made
  /// real: scheduled off the appointment's own confirmed slot time, not a
  /// backend-pushed event, since the backend does not target patients with
  /// `CONSULTATION_REMINDER_15M` today — see `notification_ids.dart`).
  /// Deliberately uses [AndroidScheduleMode.inexactAllowWhileIdle], not an
  /// exact alarm: this is best-effort (Doze/battery optimization may defer
  /// it), and an exact alarm on Android 12+ additionally requires the
  /// special `SCHEDULE_EXACT_ALARM`/`USE_EXACT_ALARM` access, which this
  /// architecture deliberately avoids needing. If [whenUtc] is already in
  /// the past, the plugin fires it immediately.
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
  /// authoritative appointment list and cancel exactly the stale ones
  /// without ever touching ones that are still current.
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
