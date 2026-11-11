/// Formats an already-local [DateTime] as a 12-hour clock time with an
/// uppercase AM/PM suffix (e.g. `21:20` → `9:20 PM`). Every appointment/slot
/// screen must use this instead of hand-rolled 24-hour formatting so the
/// doctor always sees a readable time regardless of device locale
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
