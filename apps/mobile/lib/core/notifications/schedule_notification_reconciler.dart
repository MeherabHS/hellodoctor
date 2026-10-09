/// Layer 3 of the no-external-push notification architecture: keeps the
/// phone's own OS-level scheduled notifications (shown even if the app
/// process isn't running, via `flutter_local_notifications`) in sync with the
/// Doctor's real appointment schedule.
///
/// Deliberately split into a pure decision function ([computePlan]) and a
/// thin apply step ([apply]/[reconcile]) that is the only part touching
/// [LocalNotificationService]. This mirrors the backend's
/// `notification_scheduler.rs` (daily-summary pass + T-15-reminder pass,
/// both idempotent) but computed purely from an already-fetched
/// `appointments` list — this reconciler never fetches anything itself.
library;

import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import '../../features/doctor/doctor_models.dart';
import 'local_notification_service.dart';
import 'notification_ids.dart';

/// Kenya/Nairobi is a fixed UTC+3 offset year-round (no DST) — mirrors
/// `nairobi_offset()` in `services/backend/src/services/notification_service.rs`
/// exactly. Only used for our own "what Nairobi date/wall-clock time is this
/// UTC instant" arithmetic; actual OS-level scheduling still goes through
/// `LocalNotificationService.scheduleAt`, which uses the `timezone` package's
/// `Africa/Nairobi` `Location` internally.
const Duration _nairobiOffset = Duration(hours: 3);

/// Appointment statuses excluded from the daily consultation count — mirrors
/// the backend scheduler's `a.status NOT IN ('CANCELLED', 'EXPIRED')`.
const Set<String> _excludedFromDailyCount = {'CANCELLED', 'EXPIRED'};

/// Appointment statuses eligible for a T-15 reminder — mirrors the backend's
/// `a.status IN ('CONFIRMED', 'WAITING')`.
const Set<String> _t15EligibleStatuses = {'CONFIRMED', 'WAITING'};

const String _t15PayloadPrefix = 'consultation_t15:';
const String _dailyPayloadPrefix = 'daily_schedule:';

/// The backend falls back to 07:00 Nairobi time when
/// `AppConfig::doctor_daily_summary_time_local` isn't configured otherwise;
/// Flutter has no access to that backend config, so this local layer always
/// uses that same 07:00 default rather than trying to duplicate the
/// backend's configurable-time logic.
const int _dailySummaryHourNairobi = 7;

String _pad(int n, [int width = 2]) => n.toString().padLeft(width, '0');

/// One device notification that should be (re)scheduled.
@immutable
class ScheduleAction {
  const ScheduleAction({
    required this.id,
    required this.channel,
    required this.whenUtc,
    required this.title,
    required this.body,
    required this.payload,
  });

  final int id;
  final AndroidNotificationChannel channel;
  final DateTime whenUtc;
  final String title;
  final String body;
  final String payload;
}

/// The full set of decisions for one reconciliation pass.
@immutable
class ReconciliationPlan {
  const ReconciliationPlan({
    required this.toSchedule,
    required this.toCancel,
  });

  final List<ScheduleAction> toSchedule;
  final List<int> toCancel;
}

class ScheduleNotificationReconciler {
  const ScheduleNotificationReconciler._();

  // ---------------------------------------------------------------------
  // Nairobi date/time helpers (fixed +3h offset, no `timezone` dependency).
  // ---------------------------------------------------------------------

  static DateTime _toNairobiWallClock(DateTime utc) =>
      utc.toUtc().add(_nairobiOffset);

  /// `yyyy-MM-dd` for the Nairobi-local calendar date containing [utc].
  static String nairobiDateString(DateTime utc) {
    final local = _toNairobiWallClock(utc);
    return '${_pad(local.year, 4)}-${_pad(local.month)}-${_pad(local.day)}';
  }

  /// `HH:mm` (24-hour) for the Nairobi-local wall-clock time of [utc].
  static String _nairobiHhMm(DateTime utc) {
    final local = _toNairobiWallClock(utc);
    return '${_pad(local.hour)}:${_pad(local.minute)}';
  }

  /// The UTC instant corresponding to Nairobi-local midnight of the day
  /// containing [utc]. Mirrors the backend's `day_start_utc` derivation
  /// exactly (today's Nairobi local date -> midnight Nairobi -> back to UTC).
  static DateTime _nairobiDayStartUtc(DateTime utc) {
    final local = _toNairobiWallClock(utc);
    final nairobiMidnightAsUtcFields =
        DateTime.utc(local.year, local.month, local.day);
    return nairobiMidnightAsUtcFields.subtract(_nairobiOffset);
  }

  // ---------------------------------------------------------------------
  // Pure decision logic — zero plugin dependency, fully unit-testable.
  // ---------------------------------------------------------------------

  static ReconciliationPlan computePlan({
    required String doctorId,
    required List<AppointmentDto> appointments,
    required DateTime nowUtc,
    required Map<int, String> pendingByPayload,
  }) {
    final now = nowUtc.toUtc();
    final todayKey = dailySummaryKey(doctorId, nairobiDateString(now));
    final dayStartUtc = _nairobiDayStartUtc(now);
    final dayEndUtc = dayStartUtc.add(const Duration(days: 1));
    final cutoffUtc =
        dayStartUtc.add(const Duration(hours: _dailySummaryHourNairobi));

    final toSchedule = <ScheduleAction>[];
    final toCancel = <int>{};

    // --- 1. Daily summary ------------------------------------------------
    var todayCount = 0;
    for (final appt in appointments) {
      final start = appt.start;
      if (start == null) continue;
      final startUtc = start.toUtc();
      if (startUtc.isBefore(dayStartUtc) || !startUtc.isBefore(dayEndUtc)) {
        continue;
      }
      if (_excludedFromDailyCount.contains(appt.status)) continue;
      todayCount++;
    }

    if (todayCount > 0 && now.isBefore(cutoffUtc)) {
      final body = todayCount == 1
          ? 'You have 1 consultation scheduled today.'
          : 'You have $todayCount consultations scheduled today.';
      toSchedule.add(ScheduleAction(
        id: stableNotificationId(todayKey),
        channel: NotificationChannels.dailySchedule,
        whenUtc: cutoffUtc,
        title: 'Good morning',
        body: body,
        payload: todayKey,
      ));
    } else if (todayCount == 0) {
      // Spec's explicit rule for this local layer: zero consultations means
      // no summary notification at all, even if one was already pending.
      final existingId = pendingByPayload.entries
          .where((e) => e.value == todayKey)
          .map((e) => e.key)
          .firstOrNull;
      if (existingId != null) toCancel.add(existingId);
    }

    // --- 2. T-15 reminders -------------------------------------------------
    final shouldHaveT15 = <String>{};
    for (final appt in appointments) {
      final start = appt.start;
      if (start == null) continue;
      if (!_t15EligibleStatuses.contains(appt.status)) continue;
      final startUtc = start.toUtc();
      if (!startUtc.isAfter(now)) continue; // already started/passed

      shouldHaveT15.add(appt.id);

      var fireAt = startUtc.subtract(const Duration(minutes: 15));
      if (fireAt.isBefore(now)) fireAt = now;

      final key = t15ReminderKey(appt.id);
      toSchedule.add(ScheduleAction(
        id: stableNotificationId(key),
        channel: NotificationChannels.consultationReminders,
        whenUtc: fireAt,
        title: 'Consultation starting soon',
        body: 'Your consultation starts at ${_nairobiHhMm(startUtc)}.',
        payload: key,
      ));
    }

    // --- 3. Reconciliation / cleanup of stale pending local notifications --
    pendingByPayload.forEach((id, payload) {
      if (payload.startsWith(_t15PayloadPrefix)) {
        final appointmentId = payload.substring(_t15PayloadPrefix.length);
        if (!shouldHaveT15.contains(appointmentId)) {
          toCancel.add(id);
        }
      } else if (payload.startsWith(_dailyPayloadPrefix)) {
        if (payload != todayKey) {
          toCancel.add(id);
        }
      }
      // Any other payload isn't ours to manage — left untouched.
    });

    return ReconciliationPlan(
      toSchedule: toSchedule,
      toCancel: toCancel.toList(),
    );
  }

  // ---------------------------------------------------------------------
  // Apply step — the only part of this file touching the real plugin.
  // ---------------------------------------------------------------------

  static Future<void> apply(ReconciliationPlan plan) async {
    for (final action in plan.toSchedule) {
      await LocalNotificationService.scheduleAt(
        id: action.id,
        channel: action.channel,
        whenUtc: action.whenUtc,
        title: action.title,
        body: action.body,
        payload: action.payload,
      );
    }
    for (final id in plan.toCancel) {
      await LocalNotificationService.cancel(id);
    }
  }

  /// Public entry point: reads currently-pending local notifications, computes
  /// the reconciliation plan purely from the given [appointments] and
  /// [nowUtc] (nothing is fetched here), and applies it.
  static Future<void> reconcile({
    required String doctorId,
    required List<AppointmentDto> appointments,
    required DateTime nowUtc,
  }) async {
    final pending = await LocalNotificationService.pendingByPayload();
    final plan = computePlan(
      doctorId: doctorId,
      appointments: appointments,
      nowUtc: nowUtc,
      pendingByPayload: pending,
    );
    await apply(plan);
  }
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}
