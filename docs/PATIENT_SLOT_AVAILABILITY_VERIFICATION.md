# Patient future availability

The public availability query requires AVAILABLE and `start_time > clock_timestamp()`. Booking rechecks time while atomically transitioning the locked slot, so a stale client cannot book a slot that started during a row-lock wait. Date-only checks are not used. Instants remain UTC; Kenya display/calendar uses Africa/Nairobi.

Yesterday and earlier-today AVAILABLE rows are retained internally for history but are not public bookable availability. Direct booking of these IDs returns the existing 409 business conflict before appointment/transaction/financial creation. Historical rows are not deleted as a workaround.

Patient BookingRepository does fresh authoritative reads. Opening/reopening availability, manual refresh, app resume and reconnect reconcile slots. A refresh clears stale actionable state; failure/offline does not leave an old selected slot available for submission. New Doctor slots and booked/blocked changes therefore appear through the same authoritative endpoint.

Evidence includes retained `test_chat_booking_rejects_started_slots_without_financial_effect`, full timestamp/UTC-Nairobi boundary cases and the row-lock crossing regression, plus existing Flutter freshness/recovery tests. No assertions or historical fixtures were removed. Final automated outcomes are recorded in the milestone gate evidence. Physical yesterday/earlier-today/future screenshots for this build are pending USB availability.
