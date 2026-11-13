import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:timezone/timezone.dart' as tz;

import '../../../core/notifications/local_notification_service.dart';
import '../../../core/notifications/reminder_alarm.dart';
import 'reminder_models.dart';

/// Where a patient's reminders are kept.
abstract class ReminderStore {
  Future<List<Reminder>> load(String userId);
  Future<void> save(String userId, List<Reminder> reminders);
}

/// Encrypted, on-device storage. Reminder titles can name a treatment, so
/// they are kept in the platform keystore-backed secure storage, never in
/// plain preferences or a generic cache.
///
/// It uses its own storage file, separate from the session tokens: signing
/// out clears the token file, and a patient's reminders should still be
/// there when the same account signs back in. Each account has its own key,
/// so one account never sees another's reminders.
class SecureReminderStore implements ReminderStore {
  SecureReminderStore([FlutterSecureStorage? storage])
      : _storage = storage ??
            const FlutterSecureStorage(
                aOptions: AndroidOptions(
                    sharedPreferencesName: reminderSecureStorageFile));

  /// Named on every call; see `sessionSecureStorage` in api_client.dart for
  /// why an unnamed store would read the wrong file.
  static const reminderSecureStorageFile = 'afya_patient_reminders_v1';
  final FlutterSecureStorage _storage;

  String _key(String userId) => 'reminders_$userId';

  @override
  Future<List<Reminder>> load(String userId) async =>
      Reminder.decodeList(await _storage.read(key: _key(userId)));

  @override
  Future<void> save(String userId, List<Reminder> reminders) =>
      _storage.write(key: _key(userId), value: Reminder.encodeList(reminders));
}

/// Localized text an alarm needs (channel name and action labels).
class ReminderAlertText {
  const ReminderAlertText(
      {required this.channelName,
      required this.doneLabel,
      required this.snoozeLabel});
  final String channelName, doneLabel, snoozeLabel;
}

/// Schedules and cancels the device alerts for reminders.
abstract class ReminderScheduler {
  /// Whether the system will show this app's notifications. Asks the user
  /// when the platform requires a runtime permission.
  Future<bool> ensurePermission();

  /// Whether alarms can fire at the exact time (Android "Alarms &
  /// reminders"). Asks once when they cannot. When false, alerts may be a
  /// few minutes late and the reminders screen says so.
  Future<bool> ensureExactAlarms();
  Future<void> schedule(Reminder reminder,
      {required ReminderAlertText text, required DateTime now});
  Future<void> cancel(String reminderId);
}

/// Alarm-style local notifications (see [ReminderAlarm]): exact alarm-clock
/// timing where allowed, the system alarm sound repeating until Done or
/// Snooze, text hidden on a locked screen, no network needed.
class LocalReminderScheduler implements ReminderScheduler {
  LocalReminderScheduler([FlutterLocalNotificationsPlugin? plugin])
      : _plugin = plugin ?? FlutterLocalNotificationsPlugin();
  final FlutterLocalNotificationsPlugin _plugin;

  static int idFor(String reminderId) => ReminderAlarm.idFor(reminderId);

  @override
  Future<bool> ensurePermission() async {
    await LocalNotificationService.init();
    return LocalNotificationService.requestPermission();
  }

  @override
  Future<bool> ensureExactAlarms() async {
    await LocalNotificationService.init();
    return LocalNotificationService.requestExactAlarms();
  }

  @override
  Future<void> schedule(Reminder reminder,
      {required ReminderAlertText text, required DateTime now}) async {
    await LocalNotificationService.init();
    await cancel(reminder.id);
    if (!reminder.enabled) return;
    final next = reminder.nextAlert(now);
    if (next == null) return; // a one-time reminder already in the past
    final exact = await ReminderAlarm.exactAlarmsAllowed(_plugin);
    // The alert instant is fixed in UTC from the device-local wall time.
    // Repeats then match on the UTC clock components, which keeps the same
    // local time wherever the device has no daylight-saving shift (Kenya).
    final when = tz.TZDateTime.from(next.toUtc(), tz.UTC);
    await _plugin.zonedSchedule(
      idFor(reminder.id),
      reminder.title,
      reminder.note.isEmpty ? null : reminder.note,
      when,
      ReminderAlarm.details(
          channelName: text.channelName,
          doneLabel: text.doneLabel,
          snoozeLabel: text.snoozeLabel),
      androidScheduleMode: exact
          ? AndroidScheduleMode.alarmClock
          : AndroidScheduleMode.inexactAllowWhileIdle,
      uiLocalNotificationDateInterpretation:
          UILocalNotificationDateInterpretation.absoluteTime,
      payload: ReminderAlarm.payload(
          reminderId: reminder.id,
          title: reminder.title,
          note: reminder.note,
          channelName: text.channelName,
          doneLabel: text.doneLabel,
          snoozeLabel: text.snoozeLabel),
      matchDateTimeComponents: switch (reminder.repeat) {
        ReminderRepeat.once => null,
        ReminderRepeat.daily => DateTimeComponents.time,
        ReminderRepeat.weekly => DateTimeComponents.dayOfWeekAndTime,
        ReminderRepeat.monthly => DateTimeComponents.dayOfMonthAndTime,
      },
    );
  }

  @override
  Future<void> cancel(String reminderId) async {
    await _plugin.cancel(idFor(reminderId));
    await _plugin.cancel(ReminderAlarm.snoozeIdFor(reminderId));
  }
}
