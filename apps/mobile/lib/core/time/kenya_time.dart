import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

final _nairobi = (() {
  tzdata.initializeTimeZones();
  return tz.getLocation('Africa/Nairobi');
})();

DateTime kenyaTime(DateTime instant) => tz.TZDateTime.from(instant, _nairobi);

/// Interpret picker calendar fields in the product timezone, then send UTC.
DateTime kenyaWallTime(int year, int month, int day, int hour, int minute) =>
    tz.TZDateTime(_nairobi, year, month, day, hour, minute);
