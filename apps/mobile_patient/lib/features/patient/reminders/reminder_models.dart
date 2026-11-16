import 'dart:convert';

/// The six reminder kinds in the product owner's reminder designs.
enum ReminderCategory { art, prep, appointment, testing, medication, other }

/// How a reminder repeats, like a phone's clock app:
/// - [weekly] rings on the chosen [Reminder.weekdays] (Mon-Fri, weekends...);
/// - [hourly] rings every [Reminder.everyHours] hours around the clock,
///   starting from the reminder's time (e.g. 08:00, 16:00, 00:00 for 8).
enum ReminderRepeat { once, daily, weekly, hourly, monthly }

/// Hour intervals offered for [ReminderRepeat.hourly]. Each divides 24, so
/// the ring times are the same every day and each one can be a native daily
/// alarm that Android re-arms by itself, with no app process needed.
const reminderHourIntervals = [1, 2, 3, 4, 6, 8, 12];

/// A patient-authored reminder.
///
/// Reminders are personal and local: they are stored on the device, scoped to
/// the signed-in account, and alert at the device's local time (v2.1 brief,
/// M05). They are never sent to the backend.
class Reminder {
  const Reminder({
    required this.id,
    required this.category,
    required this.title,
    required this.at,
    required this.repeat,
    this.note = '',
    this.enabled = true,
    this.weekdays = const {},
    this.everyHours = 8,
  });

  /// Days a [ReminderRepeat.weekly] reminder rings on (1 = Monday ... 7 =
  /// Sunday, as [DateTime.weekday]). Empty means the weekday of [at], which
  /// is how weekly reminders saved before day selection behave.
  final Set<int> weekdays;

  /// Hours between rings for [ReminderRepeat.hourly]; one of
  /// [reminderHourIntervals].
  final int everyHours;

  /// The weekdays a weekly reminder actually rings on, in order.
  List<int> get ringDays =>
      (weekdays.isEmpty ? {at.weekday} : weekdays).toList()..sort();

  /// Minutes after midnight of each daily ring of an hourly reminder, in
  /// the order they follow the start time.
  List<int> get ringMinutesOfDay {
    final step = reminderHourIntervals.contains(everyHours) ? everyHours : 8;
    final start = at.hour * 60 + at.minute;
    return [
      for (var t = 0; t < 24 * 60; t += step * 60) (start + t) % (24 * 60)
    ];
  }

  /// A disabled reminder is kept in the list but never alerts.
  final bool enabled;

  Reminder copyWith({
    ReminderCategory? category,
    String? title,
    String? note,
    DateTime? at,
    ReminderRepeat? repeat,
    bool? enabled,
    Set<int>? weekdays,
    int? everyHours,
  }) =>
      Reminder(
        id: id,
        category: category ?? this.category,
        title: title ?? this.title,
        note: note ?? this.note,
        at: at ?? this.at,
        repeat: repeat ?? this.repeat,
        enabled: enabled ?? this.enabled,
        weekdays: weekdays ?? this.weekdays,
        everyHours: everyHours ?? this.everyHours,
      );

  final String id;
  final ReminderCategory category;
  final String title, note;

  /// First (or only) alert time, as a device-local wall-clock time.
  final DateTime at;
  final ReminderRepeat repeat;

  /// A one-time reminder whose time has gone by. Repeating reminders never
  /// become past.
  bool isPast(DateTime now) =>
      repeat == ReminderRepeat.once && !at.isAfter(now);

  /// The next time this reminder will alert, or `null` if it never will
  /// again. Used for ordering and for the card's date line.
  DateTime? nextAlert(DateTime now) {
    switch (repeat) {
      case ReminderRepeat.once:
        return at.isAfter(now) ? at : null;
      case ReminderRepeat.daily:
        if (at.isAfter(now)) return at;
        return _nextDaily(now, at.hour * 60 + at.minute);
      case ReminderRepeat.weekly:
      case ReminderRepeat.hourly:
        // The start time itself rings only if it falls on a chosen day.
        return _earliest(firstRings(now));
      case ReminderRepeat.monthly:
        if (at.isAfter(now)) return at;
        var year = now.year, month = now.month;
        for (var i = 0; i < 14; i++) {
          final lastDay = DateTime(year, month + 1, 0).day;
          // A day the month does not have (e.g. the 31st) is skipped, which
          // is how the platform's monthly match behaves.
          if (at.day <= lastDay) {
            final next = DateTime(year, month, at.day, at.hour, at.minute);
            if (next.isAfter(now)) return next;
          }
          month++;
          if (month > 12) {
            month = 1;
            year++;
          }
        }
        return null;
    }
  }

  /// The first ring of each native alarm this reminder needs, after [now]
  /// and not before [at]. One entry for once/daily/monthly, one per chosen
  /// day for weekly, one per daily ring time for hourly.
  List<DateTime> firstRings(DateTime now) {
    final from =
        at.isAfter(now) ? at.subtract(const Duration(minutes: 1)) : now;
    switch (repeat) {
      case ReminderRepeat.once:
        return at.isAfter(now) ? [at] : [];
      case ReminderRepeat.daily:
      case ReminderRepeat.monthly:
        final next = nextAlert(now);
        return next == null ? [] : [next];
      case ReminderRepeat.weekly:
        return [for (final d in ringDays) _nextOnWeekday(from, d)];
      case ReminderRepeat.hourly:
        return [for (final m in ringMinutesOfDay) _nextDaily(from, m)];
    }
  }

  DateTime _nextDaily(DateTime after, int minuteOfDay) {
    var next = DateTime(after.year, after.month, after.day, minuteOfDay ~/ 60,
        minuteOfDay % 60);
    while (!next.isAfter(after)) {
      next =
          DateTime(next.year, next.month, next.day + 1, next.hour, next.minute);
    }
    return next;
  }

  DateTime _nextOnWeekday(DateTime after, int weekday) {
    var next = DateTime(after.year, after.month, after.day, at.hour, at.minute);
    while (next.weekday != weekday || !next.isAfter(after)) {
      next =
          DateTime(next.year, next.month, next.day + 1, next.hour, next.minute);
    }
    return next;
  }

  static DateTime? _earliest(List<DateTime> times) =>
      times.isEmpty ? null : times.reduce((a, b) => a.isBefore(b) ? a : b);

  Map<String, dynamic> toJson() => {
        'id': id,
        'category': category.name,
        'title': title,
        'note': note,
        'at': at.toIso8601String(),
        'repeat': repeat.name,
        'enabled': enabled,
        if (weekdays.isNotEmpty) 'weekdays': (weekdays.toList()..sort()),
        if (repeat == ReminderRepeat.hourly) 'every_hours': everyHours,
      };

  static Reminder? fromJson(Object? raw) {
    if (raw is! Map) return null;
    final at = DateTime.tryParse('${raw['at']}');
    final id = raw['id'], title = raw['title'];
    if (at == null || id is! String || title is! String) return null;
    return Reminder(
      id: id,
      category: ReminderCategory.values.firstWhere(
          (c) => c.name == raw['category'],
          orElse: () => ReminderCategory.other),
      title: title,
      note: raw['note'] is String ? raw['note'] as String : '',
      at: at,
      repeat: ReminderRepeat.values.firstWhere((r) => r.name == raw['repeat'],
          orElse: () => ReminderRepeat.once),
      // Saved before enable/disable existed: treated as enabled.
      enabled: raw['enabled'] != false,
      weekdays: raw['weekdays'] is List
          ? {
              for (final d in raw['weekdays'] as List)
                if (d is int && d >= 1 && d <= 7) d
            }
          : const {},
      everyHours: reminderHourIntervals.contains(raw['every_hours'])
          ? raw['every_hours'] as int
          : 8,
    );
  }

  static String encodeList(List<Reminder> items) =>
      jsonEncode(items.map((r) => r.toJson()).toList());

  /// Tolerant decode: a corrupt store yields an empty list, never a crash.
  static List<Reminder> decodeList(String? raw) {
    if (raw == null || raw.isEmpty) return [];
    try {
      final data = jsonDecode(raw);
      if (data is! List) return [];
      return data.map(fromJson).whereType<Reminder>().toList();
    } catch (_) {
      return [];
    }
  }
}
