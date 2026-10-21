import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/time/kenya_time.dart';

void main() {
  test(
      'Nairobi schedule date crosses midnight using real instant, independent of input offset',
      () {
    final utc = kenyaTime(DateTime.parse('2026-10-16T21:15:00Z'));
    final dhaka = kenyaTime(DateTime.parse('2026-10-17T03:15:00+06:00'));
    expect([utc.year, utc.month, utc.day, utc.hour, utc.minute],
        [2026, 10, 17, 0, 15]);
    expect(dhaka, utc);
    expect(utc.timeZoneOffset, const Duration(hours: 3));
    expect(utc.toUtc(), DateTime.utc(2026, 10, 16, 21, 15));
  });
  test(
      'Doctor picker fields map to Nairobi UTC instants across date boundary and round trip',
      () {
    final picked = kenyaWallTime(2026, 10, 17, 0, 15);
    expect(picked.toUtc(), DateTime.utc(2026, 10, 16, 21, 15));
    final daytime = kenyaWallTime(2026, 10, 17, 14, 30);
    expect(daytime.toUtc(), DateTime.utc(2026, 10, 17, 11, 30));
    final roundTrip = kenyaTime(daytime.toUtc());
    expect([
      roundTrip.year,
      roundTrip.month,
      roundTrip.day,
      roundTrip.hour,
      roundTrip.minute
    ], [
      2026,
      10,
      17,
      14,
      30
    ]);
  });
}
