import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

import 'notification_ids.dart';

/// Alarm-style personal reminders (product owner, 2026-11-13: "work like an
/// alarm clock, with sound").
///
/// - Scheduled with Android's alarm-clock mode when exact alarms are allowed,
///   so the alert fires on time even in Doze; otherwise inexact (the reminders
///   screen says so).
/// - Its own channel with the system alarm sound, alarm audio usage, maximum
///   importance and the "insistent" flag: the sound repeats until the patient
///   acts on it, and stops by itself after [ringFor].
/// - Done and Snooze actions work from the notification without opening the
///   app.
/// - Lock-screen privacy is unchanged: the text is hidden on a locked screen
///   (reminder titles can name a treatment). No full-screen takeover, because
///   it would show that text over the lock screen.
class ReminderAlarm {
  const ReminderAlarm._();

  static const channelId = 'patient_reminder_alarms_v1';
  static const doneAction = 'reminder_done';
  static const snoozeAction = 'reminder_snooze';
  static const snoozeFor = Duration(minutes: 10);
  static const ringFor = Duration(minutes: 10);
  static const _payloadKind = 'afya_reminder_alarm';

  /// Android `Notification.FLAG_INSISTENT`: repeat the sound until dismissed.
  static const _flagInsistent = 4;

  static int idFor(String reminderId) =>
      stableNotificationId('patient_reminder:$reminderId');
  static int snoozeIdFor(String reminderId) =>
      stableNotificationId('patient_reminder_snooze:$reminderId');

  static NotificationDetails details(
          {required String channelName,
          required String doneLabel,
          required String snoozeLabel}) =>
      NotificationDetails(
        android: AndroidNotificationDetails(
          channelId,
          channelName,
          channelDescription: channelName,
          importance: Importance.max,
          priority: Priority.max,
          category: AndroidNotificationCategory.alarm,
          visibility: NotificationVisibility.private,
          playSound: true,
          sound: const UriAndroidNotificationSound(
              'content://settings/system/alarm_alert'),
          audioAttributesUsage: AudioAttributesUsage.alarm,
          enableVibration: true,
          additionalFlags: Int32List.fromList([_flagInsistent]),
          timeoutAfter: ringFor.inMilliseconds,
          autoCancel: true,
          actions: [
            AndroidNotificationAction(doneAction, doneLabel,
                cancelNotification: true),
            AndroidNotificationAction(snoozeAction, snoozeLabel,
                cancelNotification: true),
          ],
        ),
      );

  /// Carries what Snooze needs to show the same alarm again. Stays on the
  /// device inside the notification; never sent anywhere.
  static String payload({
    required String reminderId,
    required String title,
    required String note,
    required String channelName,
    required String doneLabel,
    required String snoozeLabel,
  }) =>
      jsonEncode({
        'kind': _payloadKind,
        'id': reminderId,
        'title': title,
        'note': note,
        'channel': channelName,
        'done': doneLabel,
        'snooze': snoozeLabel,
      });

  static Map<String, dynamic>? decode(String? payload) {
    if (payload == null || !payload.contains(_payloadKind)) return null;
    try {
      final data = jsonDecode(payload);
      if (data is Map<String, dynamic> && data['kind'] == _payloadKind) {
        return data;
      }
    } catch (_) {}
    return null;
  }

  static bool isReminderPayload(String? payload) => decode(payload) != null;

  /// Exact alarm-clock scheduling where allowed, inexact otherwise.
  static Future<bool> exactAlarmsAllowed(
      FlutterLocalNotificationsPlugin plugin) async {
    final android = plugin.resolvePlatformSpecificImplementation<
        AndroidFlutterLocalNotificationsPlugin>();
    if (android == null) return true;
    return await android.canScheduleExactNotifications() ?? false;
  }

  static Future<void> scheduleOnce(
    FlutterLocalNotificationsPlugin plugin, {
    required int id,
    required DateTime at,
    required Map<String, dynamic> data,
  }) async {
    final exact = await exactAlarmsAllowed(plugin);
    await plugin.zonedSchedule(
      id,
      '${data['title']}',
      '${data['note']}'.isEmpty ? null : '${data['note']}',
      tz.TZDateTime.from(at.toUtc(), tz.UTC),
      details(
          channelName: '${data['channel']}',
          doneLabel: '${data['done']}',
          snoozeLabel: '${data['snooze']}'),
      androidScheduleMode: exact
          ? AndroidScheduleMode.alarmClock
          : AndroidScheduleMode.inexactAllowWhileIdle,
      uiLocalNotificationDateInterpretation:
          UILocalNotificationDateInterpretation.absoluteTime,
      payload: jsonEncode(data),
    );
  }
}

/// Handles a reminder alarm's Snooze action, in the foreground or (as the
/// plugin's background entry point) with the app closed. Done needs no work:
/// the action itself dismisses the alarm.
@pragma('vm:entry-point')
Future<void> handleReminderAlarmAction(NotificationResponse response) async {
  if (response.actionId != ReminderAlarm.snoozeAction) return;
  final data = ReminderAlarm.decode(response.payload);
  if (data == null) return;
  tzdata.initializeTimeZones();
  final plugin = FlutterLocalNotificationsPlugin();
  await plugin.initialize(const InitializationSettings(
      android: AndroidInitializationSettings('@mipmap/ic_launcher')));
  await ReminderAlarm.scheduleOnce(plugin,
      id: ReminderAlarm.snoozeIdFor('${data['id']}'),
      at: DateTime.now().add(ReminderAlarm.snoozeFor),
      data: data);
}
