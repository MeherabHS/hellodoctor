import 'dart:convert';

/// The six reminder kinds in the product owner's reminder designs.
enum ReminderCategory { art, prep, appointment, testing, medication, other }

enum ReminderRepeat { once, daily, weekly, monthly }

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
  });

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
    if (at.isAfter(now)) return at;
    switch (repeat) {
      case ReminderRepeat.once:
        return null;
      case ReminderRepeat.daily:
        var next = DateTime(now.year, now.month, now.day, at.hour, at.minute);
        if (!next.isAfter(now)) next = next.add(const Duration(days: 1));
        return next;
      case ReminderRepeat.weekly:
        var next = DateTime(now.year, now.month, now.day, at.hour, at.minute);
        while (next.weekday != at.weekday || !next.isAfter(now)) {
          next = next.add(const Duration(days: 1));
        }
        return next;
      case ReminderRepeat.monthly:
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

  Map<String, dynamic> toJson() => {
        'id': id,
        'category': category.name,
        'title': title,
        'note': note,
        'at': at.toIso8601String(),
        'repeat': repeat.name,
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
