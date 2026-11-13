import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/patient/modules/patient_modules.dart';
import 'package:hellodoctor_mobile_patient/features/patient/reminders/reminder_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/reminders/reminder_services.dart';
import 'package:hellodoctor_mobile_patient/features/patient/reminders/reminders_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/reminders/reminders_page.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class MemoryReminderStore implements ReminderStore {
  final data = <String, List<Reminder>>{};
  bool failSave = false;
  @override
  Future<List<Reminder>> load(String userId) async => [...?data[userId]];
  @override
  Future<void> save(String userId, List<Reminder> reminders) async {
    if (failSave) throw StateError('disk full');
    data[userId] = [...reminders];
  }
}

class FakeScheduler implements ReminderScheduler {
  final scheduled = <String>[], cancelled = <String>[];
  bool allowed = true;
  @override
  Future<bool> ensurePermission() async => allowed;
  @override
  Future<void> schedule(Reminder reminder,
          {required String channelName, required DateTime now}) async =>
      scheduled.add(reminder.id);
  @override
  Future<void> cancel(String reminderId) async => cancelled.add(reminderId);
}

Reminder reminder(String id, DateTime at,
        [ReminderRepeat repeat = ReminderRepeat.once]) =>
    Reminder(
        id: id,
        category: ReminderCategory.art,
        title: 'Take my ART medication',
        at: at,
        repeat: repeat);

void main() {
  final now = DateTime(2026, 11, 13, 10); // a Friday

  group('Reminder', () {
    test('a one-time reminder is past once its time has gone', () {
      expect(reminder('a', DateTime(2026, 11, 13, 9)).isPast(now), isTrue);
      expect(reminder('a', DateTime(2026, 11, 13, 11)).isPast(now), isFalse);
      expect(reminder('a', DateTime(2026, 11, 13, 9)).nextAlert(now), isNull);
    });

    test('a repeating reminder is never past and rolls to its next time', () {
      final daily =
          reminder('d', DateTime(2026, 11, 1, 8), ReminderRepeat.daily);
      expect(daily.isPast(now), isFalse);
      expect(daily.nextAlert(now), DateTime(2026, 11, 14, 8));

      final weekly =
          reminder('w', DateTime(2026, 11, 4, 8), ReminderRepeat.weekly);
      // 4 Nov 2026 is a Wednesday; the next Wednesday after Fri 13 is the 18th.
      expect(weekly.nextAlert(now), DateTime(2026, 11, 18, 8));

      final monthly =
          reminder('m', DateTime(2026, 10, 31, 8), ReminderRepeat.monthly);
      // November has no 31st, so the next alert is 31 December.
      expect(monthly.nextAlert(now), DateTime(2026, 12, 31, 8));
    });

    test('round-trips through storage and tolerates corrupt data', () {
      final r = Reminder(
          id: 'x',
          category: ReminderCategory.testing,
          title: 'My HIV test',
          note: 'Viral load',
          at: DateTime(2026, 12, 1, 9, 30),
          repeat: ReminderRepeat.monthly);
      final back = Reminder.decodeList(Reminder.encodeList([r])).single;
      expect(back.id, 'x');
      expect(back.category, ReminderCategory.testing);
      expect(back.note, 'Viral load');
      expect(back.at, DateTime(2026, 12, 1, 9, 30));
      expect(back.repeat, ReminderRepeat.monthly);
      expect(Reminder.decodeList('not json'), isEmpty);
      expect(Reminder.decodeList('[{"id":1}]'), isEmpty);
      expect(Reminder.decodeList(null), isEmpty);
    });
  });

  group('RemindersCubit', () {
    late MemoryReminderStore store;
    late FakeScheduler scheduler;
    RemindersCubit cubit(String user) => RemindersCubit(
        userId: user,
        store: store,
        scheduler: scheduler,
        channelName: 'Personal reminders',
        clock: () => now);
    setUp(() {
      store = MemoryReminderStore();
      scheduler = FakeScheduler();
    });

    test('adding saves the reminder and schedules its alert', () async {
      final c = cubit('user-a');
      await c.load();
      expect(await c.add(reminder('r1', DateTime(2026, 11, 13, 20))), isTrue);
      expect(store.data['user-a']!.single.id, 'r1');
      expect(scheduler.scheduled, ['r1']);
      expect(c.state.upcoming(now).single.id, 'r1');
      expect(c.state.notificationsAllowed, isTrue);
      await c.close();
    });

    test('reminders never appear in another account', () async {
      final a = cubit('user-a');
      await a.load();
      await a.add(reminder('r1', DateTime(2026, 11, 13, 20)));
      final b = cubit('user-b');
      await b.load();
      expect(b.state.items, isEmpty);
      await a.close();
      await b.close();
    });

    test('with notifications off the reminder is kept and the state says so',
        () async {
      scheduler.allowed = false;
      final c = cubit('user-a');
      await c.load();
      expect(await c.add(reminder('r1', DateTime(2026, 11, 13, 20))), isTrue);
      expect(scheduler.scheduled, isEmpty);
      expect(c.state.items.single.id, 'r1');
      expect(c.state.notificationsAllowed, isFalse);
      await c.close();
    });

    test('a failed save reports failure and schedules nothing', () async {
      store.failSave = true;
      final c = cubit('user-a');
      await c.load();
      expect(await c.add(reminder('r1', DateTime(2026, 11, 13, 20))), isFalse);
      expect(scheduler.scheduled, isEmpty);
      expect(c.state.items, isEmpty);
      await c.close();
    });

    test('deleting removes it and cancels its alert', () async {
      final c = cubit('user-a');
      await c.load();
      await c.add(reminder('r1', DateTime(2026, 11, 13, 20)));
      await c.remove('r1');
      expect(store.data['user-a'], isEmpty);
      expect(scheduler.cancelled, ['r1']);
      expect(c.state.items, isEmpty);
      await c.close();
    });

    test('upcoming is soonest first; past holds elapsed one-time reminders',
        () async {
      final c = cubit('user-a');
      await c.load();
      await c.add(reminder('later', DateTime(2026, 11, 20, 8)));
      await c.add(reminder('gone', DateTime(2026, 11, 12, 8)));
      await c.add(
          reminder('daily', DateTime(2026, 11, 1, 18), ReminderRepeat.daily));
      expect(c.state.upcoming(now).map((r) => r.id), ['daily', 'later']);
      expect(c.state.past(now).map((r) => r.id), ['gone']);
      await c.close();
    });

    test('a session schedules on sign-in and cancels on sign-out', () async {
      store.data['user-a'] = [reminder('r1', DateTime(2099, 1, 1, 8))];
      final session = ReminderSession(store: store, scheduler: scheduler);
      await session.start('user-a', 'Personal reminders');
      expect(scheduler.scheduled, ['r1']);
      await session.end('user-a');
      expect(scheduler.cancelled, ['r1']);
    });
  });

  group('Reminders screens', () {
    final nav = PatientShellNav(
        openServices: () {},
        openMessages: () {},
        openProfile: () {},
        select: (_) {});

    Future<void> pump(WidgetTester tester, Size size, Widget home) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(MaterialApp(
          theme: PatientTokens.theme(),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: home));
      await tester.pumpAndSettle();
    }

    for (final size in const [
      Size(320, 568),
      Size(411, 923),
      Size(800, 1280)
    ]) {
      testWidgets('the list shows an honest empty state at $size',
          (tester) async {
        await pump(
            tester,
            size,
            RemindersPage(
                nav: nav,
                userId: 'user-a',
                store: MemoryReminderStore(),
                scheduler: FakeScheduler()));
        expect(tester.takeException(), isNull);
        expect(find.text('No upcoming reminders'), findsOneWidget);
        expect(find.text('Add reminder'), findsOneWidget);
      });

      testWidgets('the create form fits and saves at $size', (tester) async {
        final store = MemoryReminderStore();
        final scheduler = FakeScheduler();
        final cubit = RemindersCubit(
            userId: 'user-a',
            store: store,
            scheduler: scheduler,
            channelName: 'Personal reminders',
            clock: () => now);
        addTearDown(cubit.close);
        await cubit.load();
        await pump(tester, size, CreateReminderPage(cubit: cubit));
        expect(tester.takeException(), isNull);
        expect(find.text('Create ART Reminder'), findsOneWidget);

        // Choosing a category changes the heading and the suggested title.
        await tester.ensureVisible(find.text('Testing'));
        await tester.tap(find.text('Testing'));
        await tester.pumpAndSettle();
        // On a small screen the heading has scrolled away; bring it back.
        await tester.drag(find.byType(ListView), const Offset(0, 2000));
        await tester.pumpAndSettle();
        expect(find.text('Create Testing Reminder'), findsOneWidget);
        expect(find.text('My HIV test'), findsOneWidget);

        final save = find.widgetWithText(FilledButton, 'Save Reminder');
        await tester.ensureVisible(save);
        await tester.pumpAndSettle();
        await tester.tap(save);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        final saved = store.data['user-a']!.single;
        expect(saved.category, ReminderCategory.testing);
        expect(saved.title, 'My HIV test');
        expect(scheduler.scheduled, [saved.id]);
      });
    }
  });
}
