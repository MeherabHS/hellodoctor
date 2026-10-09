import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/notifications/notification_ids.dart';
import 'package:hellodoctor_mobile/core/notifications/schedule_notification_reconciler.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';

AppointmentDto _appt({
  required String id,
  required String status,
  DateTime? start,
}) =>
    AppointmentDto(
      id: id,
      status: status,
      lifecycle: status,
      start: start,
      // PHI fields deliberately left null — the reconciler must never read
      // these to build notification text, but set them anyway to prove it
      // doesn't leak them even when present.
      patientName: 'Should Never Appear',
      complaint: 'Should Never Appear Either',
    );

void main() {
  const doctorId = 'doctor-1';

  group('daily summary', () {
    test(
        '6 valid CONFIRMED/WAITING appointments today -> one schedule '
        'decision with correct count', () {
      final nowUtc =
          DateTime.utc(2026, 1, 1, 2); // 05:00 Nairobi, before cutoff
      final dayStartUtc = DateTime.utc(2025, 12, 31, 21); // Jan 1 00:00 Nairobi
      final appointments = List.generate(
        6,
        (i) => _appt(
          id: 'a$i',
          status: i.isEven ? 'CONFIRMED' : 'WAITING',
          start: dayStartUtc.add(Duration(hours: i)),
        ),
      );

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: appointments,
        nowUtc: nowUtc,
        pendingByPayload: const {},
      );

      final dailyActions = plan.toSchedule
          .where((a) => a.payload.startsWith('daily_schedule:'))
          .toList();
      expect(dailyActions, hasLength(1));
      expect(dailyActions.single.body,
          'You have 6 consultations scheduled today.');
      expect(dailyActions.single.title, 'Good morning');
      final expectedKey = dailySummaryKey(doctorId, '2026-01-01');
      expect(dailyActions.single.payload, expectedKey);
      expect(dailyActions.single.id, stableNotificationId(expectedKey));
    });

    test('0 appointments today -> no schedule decision, cancels stale pending',
        () {
      final nowUtc = DateTime.utc(2026, 1, 1, 2);
      final todayKey = dailySummaryKey(doctorId, '2026-01-01');

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: const [],
        nowUtc: nowUtc,
        pendingByPayload: {42: todayKey},
      );

      expect(
        plan.toSchedule.where((a) => a.payload.startsWith('daily_schedule:')),
        isEmpty,
      );
      expect(plan.toCancel, contains(42));
    });

    test('CANCELLED appointment is excluded from the daily count', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 2);
      final dayStartUtc = DateTime.utc(2025, 12, 31, 21);
      final appointments = [
        _appt(
            id: 'cancelled-1',
            status: 'CANCELLED',
            start: dayStartUtc.add(const Duration(hours: 5))),
      ];

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: appointments,
        nowUtc: nowUtc,
        pendingByPayload: const {},
      );

      expect(
        plan.toSchedule.where((a) => a.payload.startsWith('daily_schedule:')),
        isEmpty,
      );
    });

    test(
        'Nairobi date boundary: 23:30 UTC appointment counts toward the next '
        'Nairobi day, not its own UTC calendar date', () {
      // "today" in Nairobi terms is Jan 2 (now = Jan 2 03:00 UTC = 06:00
      // Nairobi, before the 07:00 cutoff).
      final nowUtc = DateTime.utc(2026, 1, 2, 3);
      // This appointment's UTC calendar date is Jan 1, but its Nairobi
      // wall-clock time is Jan 2 02:30 -- it must count as "today" (Jan 2).
      final boundaryAppointment = _appt(
        id: 'boundary-1',
        status: 'CONFIRMED',
        start: DateTime.utc(2026, 1, 1, 23, 30),
      );

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: [boundaryAppointment],
        nowUtc: nowUtc,
        pendingByPayload: const {},
      );

      final dailyActions = plan.toSchedule
          .where((a) => a.payload.startsWith('daily_schedule:'))
          .toList();
      expect(dailyActions, hasLength(1));
      expect(
          dailyActions.single.body, 'You have 1 consultation scheduled today.');
      expect(
          dailyActions.single.payload, dailySummaryKey(doctorId, '2026-01-02'));
    });
  });

  group('T-15 reminders', () {
    test(
        'future CONFIRMED appointment -> exactly one T-15 decision at '
        'start - 15m', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 6, 0);
      // 07:30 UTC == 10:30 Nairobi (UTC+3), and still in the future relative
      // to nowUtc.
      final start = DateTime.utc(2026, 1, 1, 7, 30);
      final appt = _appt(id: 'appt-1', status: 'CONFIRMED', start: start);

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: [appt],
        nowUtc: nowUtc,
        pendingByPayload: const {},
      );

      final t15 = plan.toSchedule
          .where((a) => a.payload.startsWith('consultation_t15:'))
          .toList();
      expect(t15, hasLength(1));
      expect(t15.single.payload, t15ReminderKey('appt-1'));
      expect(t15.single.whenUtc, start.subtract(const Duration(minutes: 15)));
      expect(t15.single.title, 'Consultation starting soon');
      expect(t15.single.body, 'Your consultation starts at 10:30.');
    });

    test(
        'CANCELLED appointment with a previously pending T-15 payload -> '
        'cancel decision', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);
      final appt = _appt(
        id: 'appt-cancel',
        status: 'CANCELLED',
        start: DateTime.utc(2026, 1, 1, 10, 30),
      );
      final staleKey = t15ReminderKey('appt-cancel');
      final staleId = stableNotificationId(staleKey);

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: [appt],
        nowUtc: nowUtc,
        pendingByPayload: {staleId: staleKey},
      );

      expect(
        plan.toSchedule.where((a) => a.payload == staleKey),
        isEmpty,
      );
      expect(plan.toCancel, contains(staleId));
    });

    test(
        'RESCHEDULED appointment (same id, new start) -> exactly one '
        'schedule decision, no duplicate', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);
      final newStart = DateTime.utc(2026, 1, 1, 12, 0);
      final oldKey = t15ReminderKey('appt-resched');
      final oldId = stableNotificationId(oldKey);
      final appt =
          _appt(id: 'appt-resched', status: 'CONFIRMED', start: newStart);

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: [appt],
        nowUtc: nowUtc,
        // Simulate the old pending request for the same id/payload still
        // being present (the plugin replaces it in place on scheduleAt).
        pendingByPayload: {oldId: oldKey},
      );

      final matching =
          plan.toSchedule.where((a) => a.payload == oldKey).toList();
      expect(matching, hasLength(1));
      expect(matching.single.whenUtc,
          newStart.subtract(const Duration(minutes: 15)));
      // Reconciliation cleanup must not also emit a cancel for the id that
      // is still valid and being rescheduled in place.
      expect(plan.toCancel, isNot(contains(oldId)));
    });

    test('20 appointments -> 20 distinct T-15 decisions, no duplicates', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);
      final appointments = List.generate(
        20,
        (i) => _appt(
          id: 'appt-$i',
          status: i.isEven ? 'CONFIRMED' : 'WAITING',
          start: nowUtc.add(Duration(minutes: 30 + i)),
        ),
      );

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: appointments,
        nowUtc: nowUtc,
        pendingByPayload: const {},
      );

      final t15 = plan.toSchedule
          .where((a) => a.payload.startsWith('consultation_t15:'))
          .toList();
      expect(t15, hasLength(20));
      expect(t15.map((a) => a.id).toSet(), hasLength(20));
      expect(t15.map((a) => a.payload).toSet(), hasLength(20));
    });

    test(
        'appointment whose start is already in the past -> no T-15 '
        'decision, cancels any previously pending one', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);
      final pastAppt = _appt(
        id: 'appt-past',
        status: 'CONFIRMED',
        start: nowUtc.subtract(const Duration(minutes: 5)),
      );
      final staleKey = t15ReminderKey('appt-past');
      final staleId = stableNotificationId(staleKey);

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: [pastAppt],
        nowUtc: nowUtc,
        pendingByPayload: {staleId: staleKey},
      );

      expect(plan.toSchedule.where((a) => a.payload == staleKey), isEmpty);
      expect(plan.toCancel, contains(staleId));
    });

    test(
        'appointment booked with under 15 minutes notice fires at "now", '
        'never in the past', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);
      final appt = _appt(
        id: 'appt-soon',
        status: 'WAITING',
        start: nowUtc.add(const Duration(minutes: 5)),
      );

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: [appt],
        nowUtc: nowUtc,
        pendingByPayload: const {},
      );

      final t15 = plan.toSchedule
          .where((a) => a.payload == t15ReminderKey('appt-soon'))
          .toList();
      expect(t15, hasLength(1));
      expect(t15.single.whenUtc, nowUtc);
    });
  });

  group('cleanup of unrelated payloads', () {
    test('a payload matching neither prefix is left untouched', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: const [],
        nowUtc: nowUtc,
        pendingByPayload: const {99: 'notification_event:some-other-thing'},
      );

      expect(plan.toCancel, isNot(contains(99)));
    });

    test('a stale prior-day daily_schedule payload is cancelled', () {
      final nowUtc = DateTime.utc(2026, 1, 1, 2);
      final staleKey = dailySummaryKey(doctorId, '2025-12-31');
      final staleId = stableNotificationId(staleKey);

      final plan = ScheduleNotificationReconciler.computePlan(
        doctorId: doctorId,
        appointments: const [],
        nowUtc: nowUtc,
        pendingByPayload: {staleId: staleKey},
      );

      expect(plan.toCancel, contains(staleId));
    });
  });

  test('notification text never contains patient name or complaint (PHI)', () {
    final nowUtc = DateTime.utc(2026, 1, 1, 9, 0);
    final appt = _appt(
      id: 'appt-phi',
      status: 'CONFIRMED',
      start: nowUtc.add(const Duration(minutes: 20)),
    );

    final plan = ScheduleNotificationReconciler.computePlan(
      doctorId: doctorId,
      appointments: [appt],
      nowUtc: nowUtc,
      pendingByPayload: const {},
    );

    for (final action in plan.toSchedule) {
      expect(action.title.contains('Should Never Appear'), isFalse);
      expect(action.body.contains('Should Never Appear'), isFalse);
    }
  });
}
