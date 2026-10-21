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
        channels.singleWhere((c) => c['id'] == 'CONSULTATION_REMINDERS');
    expect(reminders['importance'], 4);

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
    expect(android['channelId'], 'CONSULTATION_REMINDERS');
    expect(android['importance'], 4);
    expect(android['priority'], 1);
  });
}
