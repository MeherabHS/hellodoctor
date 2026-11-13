import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:timezone/timezone.dart' as tz;

import '../../../core/notifications/local_notification_service.dart';
import '../../../core/notifications/notification_ids.dart';
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
                    sharedPreferencesName: 'afya_patient_reminders_v1'));
  final FlutterSecureStorage _storage;

  String _key(String userId) => 'reminders_$userId';

  @override
  Future<List<Reminder>> load(String userId) async =>
      Reminder.decodeList(await _storage.read(key: _key(userId)));

  @override
  Future<void> save(String userId, List<Reminder> reminders) =>
      _storage.write(key: _key(userId), value: Reminder.encodeList(reminders));
}

/// Schedules and cancels the device alerts for reminders.
abstract class ReminderScheduler {
  /// Whether the system will show this app's notifications. Asks the user
  /// when the platform requires a runtime permission.
  Future<bool> ensurePermission();
  Future<void> schedule(Reminder reminder,
      {required String channelName, required DateTime now});
  Future<void> cancel(String reminderId);
}

/// Local notifications through `flutter_local_notifications`.
///
/// - Alerts fire at the device's local time, with no network.
/// - Timing is inexact (the system may deliver a few minutes late). Exact
///   alarms need an extra Android permission that this app does not request.
/// - On a locked screen Android hides the notification's text
///   ([NotificationVisibility.private]); the reminder's title and note show
///   only once the phone is unlocked.
class LocalReminderScheduler implements ReminderScheduler {
  LocalReminderScheduler([FlutterLocalNotificationsPlugin? plugin])
      : _plugin = plugin ?? FlutterLocalNotificationsPlugin();
  final FlutterLocalNotificationsPlugin _plugin;

  static const _channelId = 'patient_personal_reminders';

  static int idFor(String reminderId) =>
      stableNotificationId('patient_reminder:$reminderId');

  @override
  Future<bool> ensurePermission() async {
    await LocalNotificationService.init();
    return LocalNotificationService.requestPermission();
  }

  @override
  Future<void> schedule(Reminder reminder,
      {required String channelName, required DateTime now}) async {
    await LocalNotificationService.init();
    await _plugin.cancel(idFor(reminder.id));
    final next = reminder.nextAlert(now);
    if (next == null) return; // a one-time reminder already in the past
    // The alert instant is fixed in UTC from the device-local wall time.
    // Repeats then match on the UTC clock components, which keeps the same
    // local time wherever the device has no daylight-saving shift (Kenya).
    final when = tz.TZDateTime.from(next.toUtc(), tz.UTC);
    await _plugin.zonedSchedule(
      idFor(reminder.id),
      reminder.title,
      reminder.note.isEmpty ? null : reminder.note,
      when,
      NotificationDetails(
          android: AndroidNotificationDetails(_channelId, channelName,
              importance: Importance.high,
              priority: Priority.high,
              visibility: NotificationVisibility.private,
              category: AndroidNotificationCategory.reminder)),
      androidScheduleMode: AndroidScheduleMode.inexactAllowWhileIdle,
      uiLocalNotificationDateInterpretation:
          UILocalNotificationDateInterpretation.absoluteTime,
      matchDateTimeComponents: switch (reminder.repeat) {
        ReminderRepeat.once => null,
        ReminderRepeat.daily => DateTimeComponents.time,
        ReminderRepeat.weekly => DateTimeComponents.dayOfWeekAndTime,
        ReminderRepeat.monthly => DateTimeComponents.dayOfMonthAndTime,
      },
    );
  }

  @override
  Future<void> cancel(String reminderId) => _plugin.cancel(idFor(reminderId));
}
