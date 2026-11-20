import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/notifications/local_notification_service.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('upcoming consultation uses the high-priority Android reminder channel',
      () async {
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
    await LocalNotificationService.init();
    expect(calls.where((c) => c.method == 'initialize'), hasLength(1));
    final channels = calls
        .where((c) => c.method == 'createNotificationChannel')
        .map((c) => c.arguments as Map)
        .toList();
    expect(channels, hasLength(6));
    final reminders =
        channels.singleWhere((c) => c['id'] == 'CONSULTATION_REMINDERS_V3');
    // Forced sounds (2026-11-17): the urgent tone for reminders, the quick
    // tone for the rest. Every channel is maximum importance (2026-11-20).
    expect(channels.map((c) => c['id']), everyElement(endsWith('_V3')));
    expect(reminders['importance'], 5);
    expect(reminders['playSound'], true);
    expect(reminders['sound'], 'afya_reminder_urgent');
    for (final c in channels.where((c) => c != reminders)) {
      expect(c['importance'], 5, reason: '${c['id']} must be max importance');
      expect(c['playSound'], true);
      expect(c['sound'], 'afya_notify_quick');
    }
    final deleted = calls
        .where((c) => c.method == 'deleteNotificationChannel')
        .map((c) => c.arguments)
        .toSet();
    expect(deleted, containsAll(NotificationChannels.legacyIds));
    expect(deleted, contains('MESSAGES_V2'));

    await LocalNotificationService.showNow(
        id: 123,
        channel: NotificationChannels.consultationReminders,
        title: 'Consultation starting soon',
        body: 'Your consultation starts in 15 minutes.',
        payload: 'remote:synthetic-reminder');
    final show = calls.singleWhere((c) => c.method == 'show').arguments as Map;
    expect(show['id'], 123);
    expect(show['title'], 'Consultation starting soon');
    expect(show['body'], 'Your consultation starts in 15 minutes.');
    expect(show['payload'], 'remote:synthetic-reminder');
    final android = show['platformSpecifics'] as Map;
    expect(android['channelId'], 'CONSULTATION_REMINDERS_V3');
    expect(android['importance'], 5);
    expect(android['priority'], 2);
    expect(android['playSound'], true);
    expect(android['sound'], 'afya_reminder_urgent');

    // Any other notification is shown at max priority too.
    await LocalNotificationService.showNow(
        id: 124,
        channel: NotificationChannels.messages,
        title: 'New message',
        body: 'Open the chat to read it.');
    final message = calls
        .where((c) => c.method == 'show')
        .map((c) => c.arguments as Map)
        .singleWhere((a) => a['id'] == 124);
    final messageAndroid = message['platformSpecifics'] as Map;
    expect(messageAndroid['channelId'], 'MESSAGES_V3');
    expect(messageAndroid['importance'], 5);
    expect(messageAndroid['priority'], 2);
  });
}
