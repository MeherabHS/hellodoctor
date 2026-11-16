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
  bool allowed = true, exact = true;

  /// What the user does on the system "Alarms & reminders" page.
  bool grantOnAsk = false;
  int exactAsks = 0;
  @override
  Future<bool> ensurePermission() async => allowed;
  @override
  Future<bool> ensureExactAlarms() async {
    if (!exact) {
      exactAsks++;
      if (grantOnAsk) exact = true;
    }
    return exact;
  }

  @override
  Future<bool> exactAlarmsAllowed() async => exact;
  @override
  Future<void> schedule(Reminder reminder,
      {required ReminderAlertText text, required DateTime now}) async {
    if (!reminder.enabled) return;
    scheduled.add(reminder.id);
  }

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

const _text = ReminderAlertText(
    channelName: 'Personal reminders',
    doneLabel: 'Done',
    snoozeLabel: 'Snooze 10 min');

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
        text: _text,
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
      await session.start('user-a', _text);
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
            text: _text,
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

  group('edit and switch off', () {
    RemindersCubit cubitWith(MemoryReminderStore store, FakeScheduler s) =>
        RemindersCubit(
            userId: 'user-a',
            store: store,
            scheduler: s,
            text: _text,
            clock: () => now);

    test('editing reschedules the alarm with the new time and keeps the id',
        () async {
      final store = MemoryReminderStore();
      final scheduler = FakeScheduler();
      final cubit = cubitWith(store, scheduler);
      await cubit.load();
      final r = reminder('e', DateTime(2026, 11, 13, 12));
      await cubit.add(r);
      expect(await cubit.update(r.copyWith(at: DateTime(2026, 11, 13, 18))),
          isTrue);
      expect(store.data['user-a']!.single.at, DateTime(2026, 11, 13, 18));
      expect(scheduler.scheduled, ['e', 'e']);
      await cubit.close();
    });

    test('switching off cancels the alarm but keeps the reminder; on re-arms',
        () async {
      final store = MemoryReminderStore();
      final scheduler = FakeScheduler();
      final cubit = cubitWith(store, scheduler);
      await cubit.load();
      await cubit.add(reminder('t', DateTime(2026, 11, 13, 12)));
      await cubit.setEnabled('t', false);
      expect(store.data['user-a']!.single.enabled, isFalse);
      expect(scheduler.cancelled, contains('t'));
      expect(cubit.state.items.single.enabled, isFalse);
      await cubit.setEnabled('t', true);
      expect(scheduler.scheduled.where((id) => id == 't'), hasLength(2));
      await cubit.close();
    });

    test('a disabled reminder is not re-armed at sign-in', () async {
      final store = MemoryReminderStore()
        ..data['user-a'] = [
          reminder('on', DateTime(2026, 11, 20, 8)),
          reminder('off', DateTime(2026, 11, 20, 8)).copyWith(enabled: false),
        ];
      final scheduler = FakeScheduler();
      await ReminderSession(store: store, scheduler: scheduler)
          .start('user-a', _text);
      expect(scheduler.scheduled, ['on']);
    });

    test('saved reminders from before enable/disable load as enabled', () {
      final decoded = Reminder.decodeList(
          '[{"id":"x","category":"art","title":"T","note":"","at":"2026-11-20T08:00:00.000","repeat":"daily"}]');
      expect(decoded.single.enabled, isTrue);
      expect(
          Reminder.decodeList(Reminder.encodeList(
              [decoded.single.copyWith(enabled: false)])).single.enabled,
          isFalse);
    });

    test('without exact alarms the state says alerts may be late', () async {
      final scheduler = FakeScheduler()..exact = false;
      final cubit = cubitWith(MemoryReminderStore(), scheduler);
      await cubit.load();
      await cubit.add(reminder('x', DateTime(2026, 11, 13, 12)));
      expect(cubit.state.exactAlarmsAllowed, isFalse);
      expect(scheduler.scheduled, ['x']);
      await cubit.close();
    });
  });

  group('clock-style repeats', () {
    // now = Fri 13 Nov 2026 10:00
    test('selected days ring on the next chosen day at the set time', () {
      final r = reminder('d', DateTime(2026, 11, 2, 8), ReminderRepeat.weekly)
          .copyWith(weekdays: {1, 3, 5}); // Mon, Wed, Fri
      expect(r.ringDays, [1, 3, 5]);
      // Friday 08:00 has gone, so next is Monday 16th.
      expect(r.nextAlert(now), DateTime(2026, 11, 16, 8));
      final rings = r.firstRings(now)..sort();
      expect(rings, [
        DateTime(2026, 11, 16, 8),
        DateTime(2026, 11, 18, 8),
        DateTime(2026, 11, 20, 8),
      ]);
    });

    test('a future start on an unchosen day is not itself a ring', () {
      // Starts Sat 14th, rings only on weekdays: first ring Mon 16th.
      final r = reminder('d', DateTime(2026, 11, 14, 7), ReminderRepeat.weekly)
          .copyWith(weekdays: {1, 2, 3, 4, 5});
      expect(r.nextAlert(now), DateTime(2026, 11, 16, 7));
    });

    test('weekly reminders saved before day selection keep their weekday', () {
      final r = Reminder.decodeList(
              '[{"id":"x","category":"art","title":"T","note":"","at":"2026-11-04T08:00:00.000","repeat":"weekly"}]')
          .single;
      expect(r.ringDays, [DateTime.wednesday]);
      expect(r.nextAlert(now), DateTime(2026, 11, 18, 8));
    });

    test('every 8 hours rings three times a day from the start time', () {
      final r = reminder('h', DateTime(2026, 11, 1, 8), ReminderRepeat.hourly)
          .copyWith(everyHours: 8);
      expect(r.ringMinutesOfDay, [8 * 60, 16 * 60, 0]);
      expect(r.nextAlert(now), DateTime(2026, 11, 13, 16));
      expect(r.firstRings(now)..sort(), [
        DateTime(2026, 11, 13, 16),
        DateTime(2026, 11, 14, 0),
        DateTime(2026, 11, 14, 8),
      ]);
    });

    test('every hour uses 24 daily alarms, the most one reminder needs', () {
      final r =
          reminder('h', DateTime(2026, 11, 13, 9, 30), ReminderRepeat.hourly)
              .copyWith(everyHours: 1);
      expect(r.firstRings(now), hasLength(24));
      expect(r.nextAlert(now), DateTime(2026, 11, 13, 10, 30));
    });

    test('days and interval survive storage', () {
      final r = reminder('x', DateTime(2026, 11, 20, 8), ReminderRepeat.weekly)
          .copyWith(weekdays: {6, 7});
      final h = reminder('y', DateTime(2026, 11, 20, 6), ReminderRepeat.hourly)
          .copyWith(everyHours: 12);
      final back = Reminder.decodeList(Reminder.encodeList([r, h]));
      expect(back[0].weekdays, {6, 7});
      expect(back[1].repeat, ReminderRepeat.hourly);
      expect(back[1].everyHours, 12);
    });
  });

  group('exact alarm access', () {
    RemindersCubit cubitWith(MemoryReminderStore store, FakeScheduler s) =>
        RemindersCubit(
            userId: 'user-a',
            store: store,
            scheduler: s,
            text: _text,
            clock: () => now);

    test('granting on the first save re-arms every reminder on exact timing',
        () async {
      final store = MemoryReminderStore()
        ..data['user-a'] = [reminder('old', DateTime(2026, 11, 20, 8))];
      final scheduler = FakeScheduler()
        ..exact = false
        ..grantOnAsk = true;
      final cubit = cubitWith(store, scheduler);
      await cubit.load();
      await cubit.add(reminder('new', DateTime(2026, 11, 13, 12)));
      expect(cubit.state.exactAlarmsAllowed, isTrue);
      expect(scheduler.scheduled, containsAll(['new', 'old']));
      await cubit.close();
    });

    test('a refused request is asked once, not on every save', () async {
      final scheduler = FakeScheduler()..exact = false;
      final cubit = cubitWith(MemoryReminderStore(), scheduler);
      await cubit.load();
      await cubit.add(reminder('a', DateTime(2026, 11, 13, 12)));
      await cubit.add(reminder('b', DateTime(2026, 11, 13, 13)));
      expect(scheduler.exactAsks, 1);
      expect(cubit.state.exactAlarmsAllowed, isFalse);
      await cubit.close();
    });

    test('allowing later from Settings re-arms everything', () async {
      final store = MemoryReminderStore()
        ..data['user-a'] = [
          reminder('a', DateTime(2026, 11, 20, 8)),
          reminder('off', DateTime(2026, 11, 20, 8)).copyWith(enabled: false),
        ];
      final scheduler = FakeScheduler()..exact = false;
      final cubit = cubitWith(store, scheduler);
      await cubit.load();
      expect(cubit.state.exactAlarmsAllowed, isFalse);
      scheduler.exact = true; // turned on in Settings, app resumes
      await cubit.refreshAlarmAccess();
      expect(cubit.state.exactAlarmsAllowed, isTrue);
      expect(scheduler.scheduled, ['a']);
      await cubit.close();
    });

    test('the session re-arms on resume only when access changed', () async {
      final store = MemoryReminderStore()
        ..data['user-a'] = [reminder('a', DateTime(2099, 1, 1, 8))];
      final scheduler = FakeScheduler()..exact = false;
      final session = ReminderSession(store: store, scheduler: scheduler);
      await session.start('user-a', _text);
      await session.resume('user-a', _text);
      expect(scheduler.scheduled, ['a']);
      scheduler.exact = true;
      await session.resume('user-a', _text);
      expect(scheduler.scheduled, ['a', 'a']);
    });
  });
}
