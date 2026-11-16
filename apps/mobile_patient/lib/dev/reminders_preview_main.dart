// Debug-only preview for the Patient Reminders module and its device alarms.
//
//   flutter run -t lib/dev/reminders_preview_main.dart
//
// Opens the real Reminders screen for a preview-only account id, with the
// real on-device store and alarm scheduler, so alarms can be checked on an
// emulator without signing in. Nothing reaches a server. Refuses to run in
// release.
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../core/notifications/local_notification_service.dart';
import '../core/theme/tokens.dart';
import '../features/patient/modules/patient_modules.dart';
import '../features/patient/reminders/reminder_models.dart';
import '../features/patient/reminders/reminder_services.dart';
import '../features/patient/reminders/reminders_page.dart';
import '../l10n/app_localizations.dart';

const _language =
    String.fromEnvironment('PREVIEW_LANGUAGE', defaultValue: 'en');

/// `--dart-define=PREVIEW_SEED=true` saves three test reminders (once in
/// 90 s, every 8 hours from 150 s, Mon/Wed/Fri 07:30) and arms them
/// through the real scheduler.
const _seed = bool.fromEnvironment('PREVIEW_SEED');
const _userId = 'preview-reminders';

Future<void> _seedAlarms() async {
  final now = DateTime.now();
  DateTime at(int s) {
    final t = now.add(Duration(seconds: s));
    return DateTime(t.year, t.month, t.day, t.hour, t.minute);
  }

  final items = [
    Reminder(
        id: 'seed-once',
        category: ReminderCategory.medication,
        title: 'Test once (preview)',
        at: at(90),
        repeat: ReminderRepeat.once),
    Reminder(
        id: 'seed-hourly',
        category: ReminderCategory.art,
        title: 'Every 8 hours (preview)',
        at: at(150),
        repeat: ReminderRepeat.hourly,
        everyHours: 8),
    Reminder(
        id: 'seed-days',
        category: ReminderCategory.prep,
        title: 'Mon, Wed, Fri (preview)',
        at: DateTime(now.year, now.month, now.day, 7, 30),
        repeat: ReminderRepeat.weekly,
        weekdays: const {1, 3, 5}),
  ];
  final store = SecureReminderStore();
  await store.save(_userId, items);
  final scheduler = LocalReminderScheduler();
  await scheduler.ensurePermission();
  for (final r in items) {
    await scheduler.schedule(r,
        text: const ReminderAlertText(
            channelName: 'Personal reminders',
            doneLabel: 'Done',
            snoozeLabel: 'Snooze 10 min'),
        now: now);
  }
  debugPrint('PREVIEW_SEED armed: ${items.map((r) => r.at).join(', ')}');
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (kReleaseMode) {
    runApp(const MaterialApp(
        home: Scaffold(
            body: Center(
                child: Text('The preview is not available in release.')))));
    return;
  }
  await LocalNotificationService.init();
  if (_seed) await _seedAlarms();
  final nav = PatientShellNav(
      openServices: () {},
      openMessages: () {},
      openProfile: () {},
      select: (_) {});
  runApp(MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: PatientTokens.theme(),
      locale: const Locale(_language),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Banner(
          message: 'PREVIEW',
          location: BannerLocation.topEnd,
          child: RemindersPage(nav: nav, userId: _userId))));
}
