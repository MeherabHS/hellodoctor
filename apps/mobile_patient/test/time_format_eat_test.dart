import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/utils/time_format.dart';

void main() {
  // The label must be correct regardless of the zone the instant arrives in
  // (UTC from the API, or the device's local zone) — never the device's own.
  test('formatTimeEat renders Nairobi wall-clock labelled EAT', () {
    final instant = DateTime.utc(2026, 11, 11, 18, 10); // 21:10 EAT
    expect(formatTimeEat(instant), '9:10 PM EAT');
    expect(formatTimeEat(instant.toLocal()), '9:10 PM EAT');
  });
  test('formatTimeEat handles the 21:00Z midnight boundary', () {
    expect(formatTimeEat(DateTime.utc(2026, 11, 11, 20, 59)), '11:59 PM EAT');
    expect(formatTimeEat(DateTime.utc(2026, 11, 11, 21, 0)), '12:00 AM EAT');
  });
}
