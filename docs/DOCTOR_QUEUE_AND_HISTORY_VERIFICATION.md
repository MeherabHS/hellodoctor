# Doctor Queue, Schedule and Consultation History

The default backend Queue contains current IN_CONSULTATION work first, then today's remaining actionable consultations, ascending by scheduled time in Africa/Nairobi. Terminal records and tomorrow's consultations are absent. An active Chat also needs an unlocked, unexpired backend window. Clients consume `queue_actionable`; they do not decide queue eligibility from a phone/browser clock.

`GET /doctor/appointments` defaults to `view=upcoming`. Explicit `view=schedule` covers future dates, `view=history` covers terminal/elapsed encounters, and `view=all` supports the existing master Schedule data source. Doctor Flutter and Web have a separate Consultation History navigation surface. Queue no longer uses All/Completed tabs as a history substitute.

Historical detail exposes the authorized encounter's summary, transcript, attachments and prescriptions. Own-encounter access checks the assigned verified active Doctor and an actual consultation session, so cancelling an unpaid booking does not create a historical treatment entitlement. It does not grant access to another Doctor's encounter with the same Patient. Temporary wider treatment-context rules remain distinct from retained own-encounter read access.

Doctor Queue retains launch/open/resume/reconnect/notification reconciliation and bounded foreground refresh. Completed historical data remains stored; no old appointments were deleted to tidy the Queue.

Evidence: the full integration gate and added `test_milestone_queue_today_schedule_future_and_history_separate`, own-history BOLA tests, Flutter Queue/History regression, and Doctor Web contract tests in `docs/audit_evidence/lifecycle_milestone/`. Full-suite outcome and live/device acceptance are recorded in the milestone gate record; no old screenshot is presented as a new physical acceptance result.

A narrow `doctor_encounter_patient_name` definer resolves the authorized encounter display name without reopening the full mutable private Patient profile after its treatment window closes. The pre-existing private-profile cutoff and foreign-profile assertions remain unchanged. Schedule and explicit All views sort future actionable records nearest first; historical terminal rows remain descending.

The final shell reconciliation correction adds a lifecycle observer, a 30-second foreground Queue timer, refresh on return to Queue, and refresh when a new backend notification appears. It cancels the timer/subscription on disposal. Two added behavioral widget regressions use the actual DoctorCubit to prove stale work is replaced on resume/tick/tab return, background ticks do not poll, duplicate notification state does not refetch, and a disposed shell does not refresh. Reconnect remains handled by the existing DoctorCubit connectivity subscription.

Doctor Flutter now uses the existing timezone package to display Queue/history times and group Schedule slots in Africa/Nairobi. Date/time picker fields are interpreted in Nairobi before UTC serialization. Date-only comparisons use calendar fields rather than host-offset instants, including midnight and DST-safe day stepping. Two new regressions assert UTC/Nairobi midnight, equivalent Dhaka-offset input and picker-to-UTC round trips. These changes affect presentation and requested instants, not the authoritative backend Queue/booking predicates.


## Subsequent API traffic correction (2026-10-16)

See [API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md](API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md) for route/background polling, in-flight coalescing, bounded pagination, selected-day Schedule reads and the separate post-milestone test/device evidence. Earlier counts and screenshots above remain historical evidence for their recorded revision. No new physical-device claim is implied.
