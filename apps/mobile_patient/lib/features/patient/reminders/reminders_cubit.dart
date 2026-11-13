import 'package:flutter_bloc/flutter_bloc.dart';

import 'reminder_models.dart';
import 'reminder_services.dart';

class RemindersState {
  const RemindersState(
      {this.loading = true,
      this.items = const [],
      this.notificationsAllowed = true});
  final bool loading;
  final List<Reminder> items;

  /// False when the system will not show this app's notifications, so a
  /// saved reminder cannot alert. The screen says so instead of implying it
  /// will.
  final bool notificationsAllowed;

  /// Reminders that will still alert, soonest first.
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
          {bool? loading, List<Reminder>? items, bool? notificationsAllowed}) =>
      RemindersState(
          loading: loading ?? this.loading,
          items: items ?? this.items,
          notificationsAllowed:
              notificationsAllowed ?? this.notificationsAllowed);
}

/// A signed-in patient's own reminders: load, add, delete, and keep the
/// device alerts in step with the saved list.
class RemindersCubit extends Cubit<RemindersState> {
  RemindersCubit(
      {required this.userId,
      required this.store,
      required this.scheduler,
      required this.channelName,
      DateTime Function()? clock})
      : _now = clock ?? DateTime.now,
        super(const RemindersState());

  final String userId;
  final ReminderStore store;
  final ReminderScheduler scheduler;

  /// Localized name of the Android notification channel.
  final String channelName;
  final DateTime Function() _now;

  DateTime get now => _now();

  Future<void> load() async {
    final items = await store.load(userId);
    if (isClosed) return;
    emit(state.copyWith(loading: false, items: items));
  }

  /// Saves the reminder, then schedules its alert. Returns false if it could
  /// not be saved; a failure to schedule is reported through
  /// [RemindersState.notificationsAllowed] rather than losing the reminder.
  Future<bool> add(Reminder reminder) async {
    final next = [...state.items, reminder];
    try {
      await store.save(userId, next);
    } catch (_) {
      return false;
    }
    var allowed = true;
    try {
      allowed = await scheduler.ensurePermission();
      if (allowed) {
        await scheduler.schedule(reminder, channelName: channelName, now: now);
      }
    } catch (_) {
      allowed = false;
    }
    if (!isClosed) {
      emit(state.copyWith(items: next, notificationsAllowed: allowed));
    }
    return true;
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
/// account's saved reminders are scheduled; when it ends, they are cancelled
/// so a signed-out phone does not surface a previous user's reminders.
class ReminderSession {
  ReminderSession({ReminderStore? store, ReminderScheduler? scheduler})
      : store = store ?? SecureReminderStore(),
        scheduler = scheduler ?? LocalReminderScheduler();
  final ReminderStore store;
  final ReminderScheduler scheduler;
  final _scheduled = <String>{};

  Future<void> start(String userId, String channelName) async {
    try {
      final now = DateTime.now();
      for (final r in await store.load(userId)) {
        await scheduler.schedule(r, channelName: channelName, now: now);
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
