import 'package:timezone/data/latest_all.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

final _nairobi = (() {
  tzdata.initializeTimeZones();
  return tz.getLocation('Africa/Nairobi');
})();

DateTime kenyaTime(DateTime instant) => tz.TZDateTime.from(instant, _nairobi);
