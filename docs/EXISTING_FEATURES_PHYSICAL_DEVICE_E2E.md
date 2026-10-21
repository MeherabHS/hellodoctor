# Existing-feature physical-device acceptance — 2026-10-15

**2026-10-16 update:** the user narrowed work to pre-E2E queue/availability and payment/chat corrections, then explicitly deferred physical verification. Those corrections now pass 221 backend integration, 53 Rust library, 197 Doctor Flutter, 195 Patient Flutter, 50 Doctor Web and 13 Admin tests. Source/binary/schema agree, updated development APKs are built, and real Web/API targeted checks pass. The full physical simulation remains unstarted. See [current correction report](PRE_E2E_QUEUE_AVAILABILITY_CORRECTION.md) and its linked payment/attachment reports. The October 15 observations below are historical evidence, not a claim that every feature has been accepted.

Acceptance is IN_PROGRESS, not a whole-product pass. Video/Agora implementation and media verification are explicitly DEFERRED by the user.

## Physical Chat observations

Using synthetic Patient Test Account and Dr. Synthetic Doctor One, the Patient selected a real Doctor slot, saw the backend Chat price (KES 500), booked, and reached a payment-pending screen whose consultation action was disabled. The existing signed development-payment callback confirmed booking `APT-20261015-5CC0D508`; no real provider payment is claimed.

The Doctor queue then displayed the authoritative Patient name and Chat consultation. Patient and Doctor sent the exact test messages through their physical Flutter clients. Both API views returned the same conversation/history, with one occurrence of each test message. The Patient cold restart reopened the persisted reply. Evidence: [booking](audit_evidence/existing_features_device_e2e/chat_booking.json), [shared history](audit_evidence/existing_features_device_e2e/chat_shared_history.json), and the adjacent `patient_*` / `doctor_*` screenshots and action log. User-entered extra test messages were preserved.

The Doctor queue needed pull-to-refresh for the new booking. A Doctor cold restart returned to authentication during this session; user interaction and subsequent failed/limited login attempts prevent attributing that observation to a specific session defect yet. Authentication rate limits were respected. Doctor cold-start session persistence remains unverified.

## RAM snapshot

At the recorded background snapshot, proportional set size was Patient 73.8 MiB and Doctor 51.5 MiB, combined 125.3 MiB. These are background session measurements, not foreground peaks: [raw values and time](audit_evidence/existing_features_device_e2e/ram_usage_background.json).

## Reminder follow-up

The user requested an actual upcoming-consultation pop-up demonstration. The release configuration fix is built with 190 Doctor tests passing. Installation and the physical demonstration remain pending ADB access: [notification report](DOCTOR_NOTIFICATION_POPUP_VERIFICATION.md).

## Booking calendar regression found during acceptance

The Patient calendar could expose AVAILABLE slots that had already started, and direct booking lacked a start-time check. One appended integration regression first failed against that behavior. The source fix now filters public slots to future starts and checks `clock_timestamp()` while transitioning the locked slot, including time spent waiting for the row lock. The new regression passes: HTTP 409 for elapsed/in-progress slots, unchanged AVAILABLE status, no appointment, and no financial transaction effect.

Evidence: `stale_slot_before.log` (observed exit 101), `stale_slot_after.log` (observed exit 0), and `backend_clippy.log` in [the evidence directory](audit_evidence/existing_features_device_e2e/). Formatting and Clippy pass. The complete library/integration rerun is still running; no final full-suite count is claimed for this change yet. All original API test content was checked against an ignored pre-edit copy and is unchanged; one test was appended. The existing development server is still serving the previous binary while the new source is tested. Its running executable was renamed to `hellodoctor-backend-before-started-slot-20261015.exe` to allow compilation without interrupting service.

## Remaining acceptance

Physical Q&A submission/Doctor answer, notifications, records/uploads/prescriptions, history authorization, completion/wallet, grievances, schedule/settings, revocation/recovery and Web/Admin cross-checks remain to be completed in this pass. Existing prior reports are separate evidence and are not represented as new physical verification here. No emulator or host software was installed, no real payment was sent, no Patient cancellation was introduced, and no Video/Agora implementation was undertaken.
