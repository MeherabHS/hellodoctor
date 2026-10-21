/// Deterministic identifiers for the no-external-push notification
/// architecture (mirrors `apps/mobile`'s `core/notifications/notification_ids.dart`,
/// see docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md for the shared design).
///
/// `flutter_local_notifications` requires a platform notification id that is
/// a plain 32-bit `int`, but this app's natural keys are strings (a backend
/// UUID, an appointment id). Hashing a stable string key down to a stable,
/// always-non-negative 31-bit int gives every caller the same id for the
/// same logical event every time — the actual mechanism behind "never
/// duplicate/cancel the wrong one" and "repeated reconciliation must not
/// duplicate it". Two different string keys can in principle collide to the
/// same int (a 31-bit space is large but finite); that is an acceptable,
/// extremely low-odds trade-off for a purely-local, best-effort notification
/// id, not a correctness requirement enforced anywhere else in this codebase.
library;

/// FNV-1a, 32-bit — simple, dependency-free, and stable across Dart/platform
/// versions (unlike `Object.hashCode`, which is explicitly NOT guaranteed
/// stable across isolates/runs and must never be used for a persisted or
/// cross-process-meaningful id).
int stableNotificationId(String key) {
  const fnvPrime = 0x01000193;
  var hash = 0x811c9dc5;
  for (final byte in key.codeUnits) {
    hash ^= byte;
    hash = (hash * fnvPrime) & 0xFFFFFFFF;
  }
  // Mask to 31 bits: platform notification ids are a signed 32-bit int, and
  // a negative id is rejected/mishandled by some Android OEM skins.
  return hash & 0x7FFFFFFF;
}

/// `consultation_t15:<appointment_id>` — reserved for parity with the Doctor
/// side's T-15 reminder; note the backend's `CONSULTATION_REMINDER_15M`
/// scheduler currently targets the doctor's `user_id` only (confirmed by
/// direct backend source inspection — `notification_scheduler.rs`'s
/// `run_t15_reminder_pass` inserts against `c.doctor_user_id`, never a
/// patient id), so this key is not yet fed by any real backend event for a
/// patient; a local, client-side-scheduled reminder (scheduled directly from
/// the appointment's own confirmed slot time, not from a backend
/// notification) is what actually drives it today — see
/// `schedule_notification_reconciler.dart`.
String t15ReminderKey(String appointmentId) =>
    'consultation_t15:$appointmentId';

/// `notification_event:<backend_notification_id>` — the dedup source for
/// bridging a backend `/notifications` row to a local device notification.
String remoteEventKey(String backendNotificationId) =>
    'notification_event:$backendNotificationId';
