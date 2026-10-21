# Pre-E2E queue and availability correction — 2026-10-16

The implementation and targeted automated/browser verification are complete. Physical verification is **DEFERRED_BY_USER** following the user's instruction to continue without the phone. The full ecosystem simulation has not begun.

## Causes and resulting contract

The Doctor appointment query grouped every CONFIRMED/WAITING record as upcoming even after its slot ended. Flutter defaulted to All Scheduled, treated almost every non-COMPLETED lifecycle as upcoming, and lost completed records from its Completed filter when the backend lifecycle became DONE. Doctor Web's desktop surface, Dashboard and badge used similar status-only filtering; its local sort could also override backend priority.

PostgreSQL now calculates `queue_actionable`: IN_CONSULTATION is current work; CONFIRMED/WAITING qualifies when the slot has not ended or its CHAT conversation is still unlocked and within its authoritative messaging window. All terminal appointment states are excluded. A paid Chat whose scheduled slot passed may correctly remain current while its real clinical-chat window is active. It is not a completed historical consultation.

The backend orders IN_CONSULTATION first, then current/future eligible records by start ascending, then history by start descending, with appointment ID as tie-breaker. `GET /doctor/appointments?view=upcoming` selects only eligible work. The existing complete appointments list remains available for History, Completed and schedule context, preserving that API's existing consumers. Both Flutter Queue and Web appointments now default to the server eligibility flag; Web retains server ordering. Completed uses appointment status, so DONE lifecycle records remain discoverable. Cached Doctor snapshots cannot establish upcoming eligibility before fresh server data arrives.

Patient availability previously checked only AVAILABLE status. It now also requires `start_time > NOW()` in the shared public-slot repository. Booking independently checks `start_time > clock_timestamp()` in the locked AVAILABLE-to-LOCKED_IN_PAYMENT update. This includes time spent waiting for a lock and prevents a transaction's earlier NOW() from accepting a slot that started meanwhile. Failure rolls back without appointment/payment rows. The slot enum has no EXPIRED state; no migration, cleanup worker or historical deletion is necessary.

Patient availability has no offline cache reads. Profile/tab entry, resume, reconnect and a 30-second foreground refresh fetch server truth. Booking clears stale choices and disables proceeding while refreshing; failures keep them unavailable. A still-valid selected slot retains its matching date. Availability and chat timestamps display in Africa/Nairobi while preserving UTC instants. Doctor/Web historical AVAILABLE openings are labeled Past opening using the server's new `bookable` flag.

## Files

- Backend: `src/repository/doctor_repo.rs`, `src/api/v1/doctor_handlers.rs`.
- Doctor Flutter: `doctor_models.dart`, `doctor_repository.dart`, `doctor_pages.dart`.
- Patient Flutter: `availability_refresh.dart`, `kenya_time.dart`, booking models/Cubit/page and Doctor profile page.
- Doctor Web: contracts, Queue, Desktop, DoctorApplication, Schedule and the BFF query allowlist.

## Evidence

Added backend cases cover stale and terminal Queue exclusion, exact nearest-first ordering, active priority, completed history, Doctor isolation, yesterday/same-day/boundary availability, Nairobi midnight, rejected direct booking and a real row-lock wait crossing the start time. Existing fifty-way booking-race tests were preserved. The earlier expired-slot regression has an observed red run before the production fix and a green run afterward.

The live development database still contains **13 historical AVAILABLE slots**. The Patient endpoint initially returned none, correctly. Two future slots were then created through the assigned synthetic Doctor's own schedule API, using database current time; the Patient endpoint returned exactly those two. No historical rows were deleted. [Live API record](audit_evidence/existing_features_device_e2e/correction_live_api.json).

Real installed-Edge/BFF checks confirm the Web default Upcoming selection, identical eligible IDs/order to the backend, and the preserved Completed filter. These are browser checks, not physical screenshots. [Results](audit_evidence/existing_features_device_e2e/correction_live_web.json), [Queue screenshot](audit_evidence/existing_features_device_e2e/correction_doctor_web_queue.png), [Completed screenshot](audit_evidence/existing_features_device_e2e/correction_doctor_web_completed.png).

Source, running development binary and database agree on migration `20261105000001`; no schema change was introduced. [Schema check](audit_evidence/existing_features_device_e2e/correction_live_schema.log).

## Status

| Required status | Result |
| --- | --- |
| DOCTOR_QUEUE_ACTIVE_ONLY_DEFAULT_VERIFIED | Automated Flutter + real Web/backend |
| DOCTOR_QUEUE_NEAREST_UPCOMING_FIRST_VERIFIED | Exact backend assertions + Web ID/order check |
| DOCTOR_COMPLETED_HISTORY_PRESERVED | Backend + Flutter + Web filter |
| PATIENT_PAST_SLOTS_FILTERED_SERVER_SIDE | Backend + live historical-row check |
| PATIENT_SAME_DAY_EXPIRED_SLOTS_FILTERED | Backend relative-time fixtures |
| PAST_SLOT_DIRECT_BOOKING_REJECTED | Exact 409 and no financial effects |
| PATIENT_AVAILABILITY_RESUME_REFRESH_VERIFIED | Lifecycle/reconnect and selection regressions |
| DOCTOR_WEB_QUEUE_CONSISTENCY_VERIFIED | Real Edge/BFF/backend |
| PATIENT_PHYSICAL_DEVICE_SLOT_AVAILABILITY_VERIFIED | DEFERRED_BY_USER |
| DOCTOR_PHYSICAL_DEVICE_QUEUE_ORDER_VERIFIED | DEFERRED_BY_USER |

Final counts, commands and assertion audit are shared with [payment verification](PATIENT_PAYMENT_UX_VERIFICATION.md). Video/Agora remains DEFERRED. Full physical E2E acceptance remains pending.
