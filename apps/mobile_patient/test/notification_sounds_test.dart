import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/notifications/local_notification_service.dart';
import 'package:hellodoctor_mobile_patient/core/notifications/reminder_alarm.dart';

/// Product owner, 2026-11-17: no notification is silent. Reminders play the
/// urgent tone on the alarm stream (they ring on silent/vibrate); every
/// other notification plays the quick tone at high importance.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('every channel has a forced sound and none is silent', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    AndroidFlutterLocalNotificationsPlugin.registerWith();
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    const channel = MethodChannel('dexterous.com/flutter/local_notifications');
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      return call.method == 'initialize' ? true : null;
    });
    addTearDown(() => TestDefaultBinaryMessengerBinding
        .instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null));

    await LocalNotificationService.init();
    final channels = calls
        .where((c) => c.method == 'createNotificationChannel')
        .map((c) => c.arguments as Map)
        .toList();
    expect(channels, hasLength(NotificationChannels.all.length));
    for (final c in channels) {
      expect('${c['id']}', endsWith('_V2'));
      expect(c['playSound'], true);
      expect(c['importance'], greaterThanOrEqualTo(4),
          reason: '${c['id']} must not be silent');
    }
    final reminders =
        channels.singleWhere((c) => c['id'] == 'CONSULTATION_REMINDERS_V2');
    expect(reminders['sound'], 'afya_reminder_urgent');
    expect(reminders['importance'], 5);
    for (final c in channels.where((c) => c != reminders)) {
      expect(c['sound'], 'afya_notify_quick', reason: '${c['id']}');
    }
    // The old silent/default-sound channels are removed from the phone.
    final deleted = calls
        .where((c) => c.method == 'deleteNotificationChannel')
        .map((c) => c.arguments)
        .toSet();
    expect(deleted, containsAll(NotificationChannels.legacyIds));

    // A general notice (formerly on the silent GENERAL channel) now alerts.
    await LocalNotificationService.showNow(
        id: 7,
        channel: NotificationChannels.forType('SOME_FUTURE_TYPE'),
        title: 'Update',
        body: 'Synthetic notice');
    final shown = (calls.singleWhere((c) => c.method == 'show').arguments
        as Map)['platformSpecifics'] as Map;
    expect(shown['channelId'], 'GENERAL_V2');
    expect(shown['playSound'], true);
    expect(shown['sound'], 'afya_notify_quick');
    expect(shown['importance'], 4);
  });

  test('a personal reminder rings the urgent tone like an alarm', () {
    final android = ReminderAlarm.details(
            channelName: 'Reminders', doneLabel: 'Done', snoozeLabel: 'Snooze')
        .android!;
    expect(android.channelId, 'patient_reminder_alarms_v2');
    expect(android.playSound, true);
    expect((android.sound as RawResourceAndroidNotificationSound).sound,
        'afya_reminder_urgent');
    expect(android.audioAttributesUsage, AudioAttributesUsage.alarm);
    expect(android.importance, Importance.max);
    expect(android.additionalFlags, contains(4),
        reason: 'insistent: repeats until Done or Snooze');
  });
}
