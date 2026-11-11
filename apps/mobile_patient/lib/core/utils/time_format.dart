import '../l10n/kenya_time.dart';

/// Formats an already-local [DateTime] as a 12-hour clock time with an
/// uppercase AM/PM suffix (e.g. `21:20` → `9:20 PM`). Every appointment/slot
/// screen must use this instead of hand-rolled 24-hour formatting so the
/// patient always sees a readable time regardless of device locale
/// (`alwaysUse24HourFormat`) — callers are responsible for ensuring [t] is
/// already in the zone they want displayed (e.g. via `kenyaTime(...)` or
/// `.toLocal()`); this function only changes the 24h/12h presentation.
String formatTime12h(DateTime t) {
  final hour24 = t.hour;
  final period = hour24 >= 12 ? 'PM' : 'AM';
  final hour12 = hour24 % 12 == 0 ? 12 : hour24 % 12;
  final minute = t.minute.toString().padLeft(2, '0');
  return '$hour12:$minute $period';
}

/// "11:24 PM EAT", prefixed with the EAT date ("12 Nov, ...") when [instant]
/// is not today in Africa/Nairobi. `now` is injectable for tests.
String formatEatDateTime(DateTime instant, {DateTime? now}) {
  final eat = kenyaTime(instant);
  final today = kenyaTime(now ?? DateTime.now());
  final sameDay = eat.year == today.year &&
      eat.month == today.month &&
      eat.day == today.day;
  if (sameDay) return formatTimeEat(instant);
  const months = [
    'Jan',
    'Feb',
    'Mar',
    'Apr',
    'May',
    'Jun',
    'Jul',
    'Aug',
    'Sep',
    'Oct',
    'Nov',
    'Dec'
  ];
  return '${eat.day} ${months[eat.month - 1]}, ${formatTimeEat(instant)}';
}

/// Clock time of [instant] in Africa/Nairobi, explicitly labelled
/// `EAT` (UTC+3). Converts regardless of the zone [instant] arrives in, so the
/// device's own timezone can never leak into what a patient or doctor sees.
String formatTimeEat(DateTime instant) =>
    '${formatTime12h(kenyaTime(instant))} EAT';
