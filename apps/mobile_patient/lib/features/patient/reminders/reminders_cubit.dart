import 'package:flutter_bloc/flutter_bloc.dart';

import 'reminder_models.dart';
import 'reminder_services.dart';

class RemindersState {
  const RemindersState(
      {this.loading = true,
      this.items = const [],
      this.notificationsAllowed = true,
      this.exactAlarmsAllowed = true,
      this.alarmFailed = false});
  final bool loading;
  final List<Reminder> items;

  /// False when the system will not show this app's notifications, so a
  /// saved reminder cannot alert. The screen says so instead of implying it
  /// will.
  final bool notificationsAllowed;

  /// False when Android does not allow exact alarms ("Alarms & reminders"),
  /// so alerts may come a few minutes late. The screen says so.
  final bool exactAlarmsAllowed;

  /// True when the last save could not set the device alarm although
  /// notifications are allowed (a scheduling error, not a permission). The
  /// screen says so rather than blaming the notification setting.
  final bool alarmFailed;

  /// Reminders that will still alert (or are switched off but would), soonest
  /// first.
  List<Reminder> upcoming(DateTime now) {
    final list = items.where((r) => !r.isPast(now)).toList();
    final far = DateTime(9999);
    list.sort(
        (a, b) => (a.nextAlert(now) ?? far).compareTo(b.nextAlert(now) ?? far));
    return list;
  }

  /// One-time reminders whose time has gone by, most recent first.
  List<Reminder> past(DateTime now) =>
      items.where((r) => r.isPast(now)).toList()
        ..sort((a, b) => b.at.compareTo(a.at));

  RemindersState copyWith(
          {bool? loading,
          List<Reminder>? items,
          bool? notificationsAllowed,
          bool? exactAlarmsAllowed,
          bool? alarmFailed}) =>
      RemindersState(
          loading: loading ?? this.loading,
          items: items ?? this.items,
          notificationsAllowed:
              notificationsAllowed ?? this.notificationsAllowed,
          exactAlarmsAllowed: exactAlarmsAllowed ?? this.exactAlarmsAllowed,
          alarmFailed: alarmFailed ?? this.alarmFailed);
}

/// A signed-in patient's own reminders: load, add, edit, switch on/off and
/// delete, keeping the device alarms in step with the saved list.
class RemindersCubit extends Cubit<RemindersState> {
  RemindersCubit(
      {required this.userId,
      required this.store,
      required this.scheduler,
      required this.text,
      DateTime Function()? clock})
      : _now = clock ?? DateTime.now,
        super(const RemindersState());

  final String userId;
  final ReminderStore store;
  final ReminderScheduler scheduler;

  /// Localized channel name and alarm action labels.
  final ReminderAlertText text;
  final DateTime Function() _now;

  DateTime get now => _now();

  Future<void> load() async {
    final items = await store.load(userId);
    var exact = state.exactAlarmsAllowed;
    try {
      exact = await scheduler.exactAlarmsAllowed();
    } catch (_) {}
    if (isClosed) return;
    emit(state.copyWith(
        loading: false, items: items, exactAlarmsAllowed: exact));
  }

  /// Saves [next] as the full list, then brings [changed]'s alarm in line
  /// with it. Returns false if the list could not be saved; a failure to
  /// schedule is reported through the state instead of losing the change.
  Future<bool> _commit(List<Reminder> next, Reminder changed) async {
    try {
      await store.save(userId, next);
    } catch (_) {
      return false;
    }
    var allowed = state.notificationsAllowed;
    var exact = state.exactAlarmsAllowed;
    var rearmAll = false;
    var failed = false;
    try {
      if (changed.enabled) {
        allowed = await scheduler.ensurePermission();
        if (allowed) {
          // The system "Alarms & reminders" page opens at most once per
          // visit; after that the screen's card offers it instead of
          // interrupting every save.
          final wasExact = await scheduler.exactAlarmsAllowed();
          if (wasExact || _askedExact) {
            exact = wasExact;
          } else {
            _askedExact = true;
            exact = await scheduler.ensureExactAlarms();
            rearmAll = exact;
          }
          try {
            await scheduler.schedule(changed, text: text, now: now);
          } catch (_) {
            failed = true;
          }
        }
      } else {
        await scheduler.cancel(changed.id);
      }
    } catch (_) {
      allowed = false;
    }
    if (!isClosed) {
      emit(state.copyWith(
          items: next,
          notificationsAllowed: allowed,
          exactAlarmsAllowed: exact,
          alarmFailed: failed));
    }
    // Granting exact alarms only helps alarms armed afterwards: re-arm the
    // others too, so none stays on late, inexact timing.
    if (rearmAll) await _rearm(except: changed.id);
    return true;
  }

  bool _askedExact = false;

  Future<void> _rearm({String? except}) async {
    for (final r in state.items) {
      if (r.id == except || !r.enabled) continue;
      try {
        await scheduler.schedule(r, text: text, now: now);
      } catch (_) {}
    }
  }

  /// Opens the system "Alarms & reminders" setting (the card's button) and,
  /// once allowed, re-arms every reminder to ring at its exact time.
  Future<void> allowExactAlarms() async {
    bool exact;
    try {
      exact = await scheduler.ensureExactAlarms();
    } catch (_) {
      return;
    }
    if (isClosed) return;
    final changed = exact != state.exactAlarmsAllowed;
    emit(state.copyWith(exactAlarmsAllowed: exact));
    if (changed) await _rearm();
  }

  /// Re-reads the exact-alarm setting without asking (the app came back to
  /// the foreground, for example from Settings). If it changed, every
  /// reminder is re-armed with the timing the system now allows; Android
  /// also drops exact alarms itself when the setting is turned off.
  Future<void> refreshAlarmAccess() async {
    bool exact;
    try {
      exact = await scheduler.exactAlarmsAllowed();
    } catch (_) {
      return;
    }
    if (isClosed || exact == state.exactAlarmsAllowed) return;
    emit(state.copyWith(exactAlarmsAllowed: exact));
    await _rearm();
  }

  Future<bool> add(Reminder reminder) =>
      _commit([...state.items, reminder], reminder);

  /// Replaces the reminder with the same id (edit), rescheduling its alarm.
  Future<bool> update(Reminder reminder) => _commit(
      [for (final r in state.items) r.id == reminder.id ? reminder : r],
      reminder);

  /// Switches a reminder's alarm on or off without deleting it.
  Future<bool> setEnabled(String id, bool enabled) async {
    final current = state.items.where((r) => r.id == id);
    if (current.isEmpty) return false;
    return update(current.first.copyWith(enabled: enabled));
  }

  Future<void> remove(String id) async {
    final next = state.items.where((r) => r.id != id).toList();
    await store.save(userId, next);
    try {
      await scheduler.cancel(id);
    } catch (_) {
      // The reminder is gone from the list either way.
    }
    if (!isClosed) emit(state.copyWith(items: next));
  }
}

/// Keeps device alerts tied to the signed-in account.
///
/// Alerts belong to whoever is signed in: when a session starts, that
/// account's saved (enabled) reminders are scheduled; when it ends, they are
/// cancelled so a signed-out phone does not surface a previous user's
/// reminders.
class ReminderSession {
  ReminderSession({ReminderStore? store, ReminderScheduler? scheduler})
      : store = store ?? SecureReminderStore(),
        scheduler = scheduler ?? LocalReminderScheduler();
  final ReminderStore store;
  final ReminderScheduler scheduler;
  final _scheduled = <String>{};
  bool? _exact;

  /// Called when the app returns to the foreground. If the exact-alarm
  /// setting changed while it was away (granted in Settings, or switched
  /// off, which makes Android drop exact alarms), every reminder is re-armed
  /// so it rings with the best timing the system now allows.
  Future<void> resume(String userId, ReminderAlertText text) async {
    try {
      final exact = await scheduler.exactAlarmsAllowed();
      if (exact == _exact) return;
    } catch (_) {
      return;
    }
    await start(userId, text);
  }

  Future<void> start(String userId, ReminderAlertText text) async {
    try {
      _exact = await scheduler.exactAlarmsAllowed();
      final now = DateTime.now();
      for (final r in await store.load(userId)) {
        if (!r.enabled) continue;
        await scheduler.schedule(r, text: text, now: now);
        _scheduled.add(r.id);
      }
    } catch (_) {
      // Reminders are best-effort; sign-in must never fail because of them.
    }
  }

  Future<void> end(String? userId) async {
    try {
      final ids = {
        ..._scheduled,
        if (userId != null) ...(await store.load(userId)).map((r) => r.id)
      };
      for (final id in ids) {
        await scheduler.cancel(id);
      }
    } catch (_) {
      // Nothing useful to do on a sign-out path.
    }
    _scheduled.clear();
  }
}
