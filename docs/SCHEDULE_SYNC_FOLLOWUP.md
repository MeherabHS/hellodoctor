# Scheduling synchronization follow-up — 2026-10-08

The two reported Sabrina Akter entries were persisted availability, not patient
bookings. Neither had an appointment attached:

| Slot | Stored UTC start | Old Admin display on a Dhaka PC | Doctor schedule (EAT) | Created UTC |
| --- | --- | --- | --- | --- |
| `bc9f613e-736f-4a54-94db-1f4010806418` | 2026-10-09 05:00 | Oct 9, 11:00 AM | Oct 9, 08:00 | 2026-10-07 16:49:31 |
| `989ac273-d40a-4163-a1e4-f224c5d7ce59` | 2026-12-01 10:00 | Dec 1, 4:00 PM | Dec 1, 13:00 | 2026-10-06 18:41:44 |

Neither has a historical creation audit or a matching saved fixture UUID in the
searched artifacts. Their creator cannot be established. No frontend placeholder
supplies either entry. The previous report's three confirmed fixture UUIDs are
different records.

The user explicitly chose **“Cancel those two unbooked slots.”** A guarded
transaction locked these exact rows, rechecked ownership/status and absence of
active appointments, changed both AVAILABLE -> CANCELLED and inserted two
`SLOT_CANCELLED` audit events with before/after snapshots and the authorized reason.
The script is `services/backend/target/schedule-sync-followup/cancel_reported_availability.sql`.
Post-commit verification confirms both CANCELLED and both audit events. Nothing
was deleted; no appointment or payment was modified. Cancelled rows remain in
history and the Cancelled filter; they are unavailable for booking. The unrelated
three proposed historical cancellation-to-block corrections remain unapplied.

## Confirmed display and refresh defects

Admin used the browser's time zone while Doctor Web uses Africa/Nairobi. Admin
displayed all dates while Doctor Web selected one date. Its schedule reads had no
visible loading/error/freshness state, so a failed read could silently leave an old
snapshot. Slot mutations awaited a full dashboard reload, including unrelated,
sequential dossier/finance reads. Doctor appointment data did not automatically
refresh with its slots, and every Doctor schedule poll cleared the displayed rows.
Doctor Web also displayed today's availability count while a different date was
selected, which made the December view appear to contain unrelated availability.

## Changes

* Admin schedules explicitly use East Africa Time (UTC+3), matching Doctor Web,
  including date boundaries. A date picker defaults to today; All dates retains
  access to the complete historical list. Availability is explicitly distinguished
  from a patient booking. Each chip exposes its creation time and ID in its tooltip.
* Schedule reads have an independent hook, refresh on a 15-second visible/online
  interval and focus/reconnect, and ignore superseded or unmounted reads. Mutations
  apply the confirmed server result and refresh only the schedule. No other
  dashboard read can overwrite schedule state or hold its controls busy.
* Admin shows refresh failures and last successful synchronization; failed reads
  preserve the last snapshot and disable stale slot actions. The proxy also marks
  responses `private, no-store` explicitly.
* Doctor Web refreshes appointments as well as slots every 15 seconds while
  visible/online, checks session generation, preserves rows during polls, and
  displays schedule refresh failures and synchronization time. Availability counts
  now follow the selected date and exclude blocked/cancelled slots.

The underlying polling contract remains eventual: external changes can take up to
15 seconds plus request latency in a visible, online portal. This is not WebSocket
push synchronization. No approval step was added to normal Doctor slot creation.

## Verification

* Doctor Web: TypeScript, source-safety/format, **32/32 existing tests**, production build.
* Admin Web: TypeScript, production build, **2/2 new time-zone/date-boundary tests**.
* Real installed-Edge walkthrough against production builds and a newly created,
  disposable PostgreSQL database: **11 assertions passed, zero browser exceptions**.
  Real Doctor MFA/Admin TOTP, Doctor UI creation -> Admin automatic display,
  Admin block -> Doctor automatic display, Doctor unblock and cancellation ->
  Admin automatic display, matching EAT times on Dhaka-configured browsers,
  selected-date availability counts, appointment polling, and absence of the test
  slot from the shared database.
* Controlled failed-read responses verify visible warnings, retained snapshots and
  disabled stale Admin actions. Successful mutation checks use the real backend;
  only the failure scenarios intercept responses.
* The test script `audit_evidence/doctor_web/schedule_sync_isolated.py` provisions
  owned test servers/database/Redis DB and cleans them up even on failure. It never
  creates a slot in the shared database. Results and local logs are under
  `services/backend/target/schedule-sync-followup/`.
* No Rust or mobile implementation changed in this follow-up. The preceding full
  backend run passed 50 library and 106 serialized API tests; it was not repeated
  for these web-only edits.
* Final read-only check against the actual shared backend and rebuilt portals:
  both reported UUIDs render CANCELLED with matching EAT times, no appointment,
  and December 1 shows zero availability. Existing blocked state and Doctor polling
  also pass. The probe issued zero slot mutations. Screenshots:
  `target/schedule-sync-followup/december-admin.png` and `december-doctor.png` under
  `services/backend/`. The database still has 41 slots and its original two booked
  rows; both approved cancellations have audit records. No owned test database or
  test Redis data remains.

Both normal portals were rebuilt and restarted. Refresh any already-open browser
tab to load the new frontend bundle.

## Dedicated Doctor sidebar entry

At the user's request, **Schedule & Slots** now appears immediately before
**Appointments** in Doctor Web's sidebar. It opens the schedule workspace directly,
with its visible **+ Add Schedule Slot** action. Schedule has its own active
navigation state and no longer appears as a child of Appointments. Existing
calendar and appointment shortcuts remain available.

Verification after this navigation change: 32/32 Doctor Web tests, TypeScript,
source-safety/format checks and the production build pass. The isolated Edge
walkthrough now passes **13 assertions**, including sidebar order, independent
active state, direct access, browser Back/Forward, narrow-screen drawer cleanup,
and opening and completing slot creation through this entry. The existing
cross-portal synchronization checks also pass with zero browser exceptions.
Owned test servers/database/Redis data were cleaned up; no test slot was created
in the shared database. Doctor Web was restarted on port 3001.
