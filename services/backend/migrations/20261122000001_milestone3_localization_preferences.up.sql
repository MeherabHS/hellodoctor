-- Notification Milestone 3 — English/Swahili templates, per-user category
-- preferences, quiet hours. Reuses the existing `notification_templates`/
-- `notification_type_registry` tables (schema-only since 20261117000001,
-- never read by any code before this) — no parallel notification system.

-- ---------------------------------------------------------------------
-- Per-user notification preferences. Quiet hours default OFF (safe
-- default — nothing changes in behavior until a user opts in); category
-- toggles default ON (nothing is silently muted until the user turns it
-- off). A user with no row at all is treated identically to one with every
-- default value — callers LEFT JOIN this table, never requiring a row to
-- exist (see `run_push_delivery_pass`'s gating query).
-- ---------------------------------------------------------------------
CREATE TABLE notification_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    operational_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    engagement_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    quiet_hours_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    quiet_hours_start_local TIME NOT NULL DEFAULT '22:00',
    quiet_hours_end_local TIME NOT NULL DEFAULT '07:00',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

ALTER TABLE notification_preferences ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_preferences FORCE ROW LEVEL SECURITY;

CREATE POLICY notification_preferences_owner_select ON notification_preferences
    FOR SELECT TO hellodoctor_api
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
CREATE POLICY notification_preferences_owner_insert ON notification_preferences
    FOR INSERT TO hellodoctor_api
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
CREATE POLICY notification_preferences_owner_update ON notification_preferences
    FOR UPDATE TO hellodoctor_api
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
-- The delivery-pass gating check (system-worker, bare pool, no session
-- identity) needs unconditional read access, mirroring
-- `notification_device_tokens_backend_select`'s exact precedent.
CREATE POLICY notification_preferences_system_select ON notification_preferences
    FOR SELECT TO hellodoctor_system_worker USING (true);

GRANT SELECT, INSERT, UPDATE ON notification_preferences TO hellodoctor_api;
GRANT SELECT ON notification_preferences TO hellodoctor_system_worker;

-- ---------------------------------------------------------------------
-- `notification_templates`/`notification_type_registry` were previously
-- schema-only, and the only read access `20261117000001` ever granted
-- `hellodoctor_api` was gated to `current_setting('app.current_role') =
-- 'ADMIN'` (meant for a future admin template-management screen — not for
-- this milestone's actual use, where `NotificationService::render` must
-- read template copy under whichever role happens to be creating a given
-- notification: an ordinary Patient/Doctor-authenticated `hellodoctor_api`
-- session, or `hellodoctor_payment_worker`/`hellodoctor_finance_worker`
-- (neither of which had ANY grant on these two tables at all). Template rows
-- hold only non-sensitive copy strings (no per-user or clinical data), so an
-- unconditional read policy for these roles is safe — this is additive (a
-- second permissive SELECT policy; Postgres OR's multiple permissive
-- policies together), so the existing admin-only policy keeps working
-- unchanged for whatever future admin screen needs it.
-- ---------------------------------------------------------------------
GRANT SELECT ON notification_templates, notification_type_registry
    TO hellodoctor_payment_worker, hellodoctor_finance_worker;
CREATE POLICY notification_templates_render_read ON notification_templates
    FOR SELECT TO hellodoctor_api, hellodoctor_payment_worker, hellodoctor_finance_worker
    USING (true);
CREATE POLICY notification_type_registry_render_read ON notification_type_registry
    FOR SELECT TO hellodoctor_api, hellodoctor_payment_worker, hellodoctor_finance_worker
    USING (true);

-- ---------------------------------------------------------------------
-- `users.preferred_language` existed since before this milestone but had no
-- real write path anywhere in the backend (confirmed by grep — the Patient
-- Flutter app's language toggle was purely device-local). `hellodoctor_api`
-- previously had no UPDATE grant on `users` at all (only the column-
-- restricted SELECT from `20261026000001_canonical_user_isolation`); this
-- adds the narrowest possible write: exactly one column, and only on the
-- caller's own row (both the column-level GRANT and the row-level USING/
-- WITH CHECK enforce this independently — neither alone would be enough
-- under `FORCE ROW LEVEL SECURITY`, which denies all access of a command
-- type with no applicable permissive policy).
-- ---------------------------------------------------------------------
CREATE POLICY users_api_update_own_language ON users
    FOR UPDATE TO hellodoctor_api
    USING (id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
GRANT UPDATE (preferred_language) ON users TO hellodoctor_api;

-- `NotificationService::notify_payout_paid` runs under `hellodoctor_finance_worker`
-- (`DisbursementService::confirm_disbursement_item_paid`'s own
-- `DbRole::FinanceWorker` transaction) and needs the paid Doctor's
-- `preferred_language` to render the notification — `hellodoctor_finance_worker`
-- had no grant on `users` at all before this (confirmed by reading
-- `20261026000001_canonical_user_isolation`'s grant list). Mirrors
-- `hellodoctor_api`'s own narrow `preferred_language`-only SELECT grant
-- (never the full row); `USING (true)` (not an owner check) because this
-- role acts in a system capacity on behalf of whichever Doctor a
-- disbursement item belongs to, exactly like `users_scheduler_read`'s
-- identical unconditional-read precedent for `hellodoctor_system_worker`.
CREATE POLICY users_finance_worker_language_read ON users
    FOR SELECT TO hellodoctor_finance_worker USING (true);
-- `id` must be granted too, even though it's never in the SELECT list:
-- Postgres checks column privileges for every column a query *references*,
-- including one used only in a WHERE filter (`WHERE id = $1`) — the same
-- reason `hellodoctor_api`'s own narrower-than-full-row grant already
-- includes `id` alongside its other readable columns.
GRANT SELECT (id, preferred_language) ON users TO hellodoctor_finance_worker;

-- ---------------------------------------------------------------------
-- English + Kiswahili templates for every notification type this codebase
-- actually emits as of Milestone 2. `template_key` disambiguates recipient
-- role or outcome where the wording genuinely differs (e.g.
-- `booking_confirmed_doctor` vs `booking_confirmed_patient`); where it
-- doesn't (e.g. CONSULTATION_READY), one key serves both recipients.
-- `{placeholder}` tokens are the ONLY substitution points the renderer
-- recognizes — a strict, per-call allowlist (see
-- `notification_service.rs`'s `render` helper), never arbitrary interpolation.
-- ---------------------------------------------------------------------
INSERT INTO notification_templates (template_key, locale, category, notification_type, title_template, body_template) VALUES
 ('registration_verified', 'en', 'SECURITY', 'REGISTRATION_VERIFIED', 'Registration verified', 'HelloDoctor has approved your submitted KMPDC registration information.'),
 ('registration_verified', 'sw', 'SECURITY', 'REGISTRATION_VERIFIED', 'Usajili umethibitishwa', 'HelloDoctor imeidhinisha taarifa zako za usajili wa KMPDC ulizowasilisha.'),

 ('registration_rejected', 'en', 'SECURITY', 'REGISTRATION_REJECTED', 'Registration rejected', 'HelloDoctor has rejected your submitted professional registration. Please review your account status.'),
 ('registration_rejected', 'sw', 'SECURITY', 'REGISTRATION_REJECTED', 'Usajili umekataliwa', 'HelloDoctor imekataa usajili wako wa kitaaluma ulizowasilisha. Tafadhali angalia hali ya akaunti yako.'),

 ('professional_verification_revoked', 'en', 'SECURITY', 'PROFESSIONAL_VERIFICATION_REVOKED', 'Professional verification revoked', 'HelloDoctor has revoked your professional verification. Please review your account status.'),
 ('professional_verification_revoked', 'sw', 'SECURITY', 'PROFESSIONAL_VERIFICATION_REVOKED', 'Uthibitisho wa kitaaluma umeondolewa', 'HelloDoctor imeondoa uthibitisho wako wa kitaaluma. Tafadhali angalia hali ya akaunti yako.'),

 ('earning_credited', 'en', 'TRANSACTIONAL', 'EARNING_CREDITED', 'Consultation earning credited', 'Consultation earning credited: {currency} {amount}.'),
 ('earning_credited', 'sw', 'TRANSACTIONAL', 'EARNING_CREDITED', 'Mapato ya ushauri yamewekwa', 'Mapato ya ushauri yamewekwa: {currency} {amount}.'),

 ('payout_paid', 'en', 'TRANSACTIONAL', 'PAYOUT_PAID', 'Payout confirmed', 'Your {currency} {amount} payout has been confirmed.'),
 ('payout_paid', 'sw', 'TRANSACTIONAL', 'PAYOUT_PAID', 'Malipo yamethibitishwa', 'Malipo yako ya {currency} {amount} yamethibitishwa.'),

 ('article_published', 'en', 'OPERATIONAL', 'ARTICLE_STATUS_CHANGED', 'Article published', 'Your article is now available in the published feed.'),
 ('article_published', 'sw', 'OPERATIONAL', 'ARTICLE_STATUS_CHANGED', 'Makala imechapishwa', 'Makala yako sasa inapatikana kwenye orodha ya machapisho.'),
 ('article_archived', 'en', 'OPERATIONAL', 'ARTICLE_STATUS_CHANGED', 'Article archived', 'Your article is no longer available in the published feed.'),
 ('article_archived', 'sw', 'OPERATIONAL', 'ARTICLE_STATUS_CHANGED', 'Makala imehifadhiwa', 'Makala yako haipatikani tena kwenye orodha ya machapisho.'),

 ('new_chat_message_doctor', 'en', 'CLINICAL', 'NEW_CHAT_MESSAGE', 'New consultation message', 'Your patient sent you a consultation message.'),
 ('new_chat_message_doctor', 'sw', 'CLINICAL', 'NEW_CHAT_MESSAGE', 'Ujumbe mpya wa ushauri', 'Mgonjwa wako amekutumia ujumbe wa ushauri.'),
 ('new_chat_message_patient', 'en', 'CLINICAL', 'NEW_CHAT_MESSAGE', 'New consultation message', 'You have a new message from your doctor.'),
 ('new_chat_message_patient', 'sw', 'CLINICAL', 'NEW_CHAT_MESSAGE', 'Ujumbe mpya wa ushauri', 'Umepata ujumbe mpya kutoka kwa daktari wako.'),

 ('booking_hold_created', 'en', 'TRANSACTIONAL', 'BOOKING_HOLD_CREATED', 'Consultation slot reserved', 'Your consultation slot is reserved for 10 minutes. Complete payment before the timer ends.'),
 ('booking_hold_created', 'sw', 'TRANSACTIONAL', 'BOOKING_HOLD_CREATED', 'Nafasi ya ushauri imehifadhiwa', 'Nafasi yako ya ushauri imehifadhiwa kwa dakika 10. Kamilisha malipo kabla muda haujaisha.'),

 ('booking_hold_one_minute', 'en', 'TRANSACTIONAL', 'BOOKING_HOLD_ONE_MINUTE', 'One minute left to complete payment', 'Your consultation reservation expires in about one minute. Complete payment now or choose a new slot after expiry.'),
 ('booking_hold_one_minute', 'sw', 'TRANSACTIONAL', 'BOOKING_HOLD_ONE_MINUTE', 'Dakika moja imebaki kukamilisha malipo', 'Hifadhi yako ya ushauri itaisha baada ya dakika moja hivi. Kamilisha malipo sasa au chagua nafasi mpya baada ya muda kuisha.'),

 ('booking_hold_expired', 'en', 'TRANSACTIONAL', 'BOOKING_HOLD_EXPIRED', 'Consultation reservation expired', 'This unpaid consultation reservation expired. Please make a new booking and upload your records again.'),
 ('booking_hold_expired', 'sw', 'TRANSACTIONAL', 'BOOKING_HOLD_EXPIRED', 'Hifadhi ya ushauri imeisha muda', 'Hifadhi hii ya ushauri isiyolipiwa imeisha muda. Tafadhali fanya uhifadhi mpya na upakie tena rekodi zako.'),

 ('daily_consultation_summary', 'en', 'OPERATIONAL', 'DAILY_CONSULTATION_SUMMARY', 'Today''s consultations', 'You have {count} consultation(s) scheduled today.'),
 ('daily_consultation_summary', 'sw', 'OPERATIONAL', 'DAILY_CONSULTATION_SUMMARY', 'Ushauri wa leo', 'Una ushauri {count} uliopangwa leo.'),

 ('consultation_reminder_15m', 'en', 'CLINICAL', 'CONSULTATION_REMINDER_15M', 'Consultation starting soon', 'Your consultation starts in 15 minutes.'),
 ('consultation_reminder_15m', 'sw', 'CLINICAL', 'CONSULTATION_REMINDER_15M', 'Ushauri unaanza hivi karibuni', 'Ushauri wako unaanza baada ya dakika 15.'),

 ('booking_confirmed_doctor', 'en', 'TRANSACTIONAL', 'BOOKING_CONFIRMED', 'Booking confirmed', 'A consultation booking has been confirmed.'),
 ('booking_confirmed_doctor', 'sw', 'TRANSACTIONAL', 'BOOKING_CONFIRMED', 'Uhifadhi umethibitishwa', 'Uhifadhi wa ushauri umethibitishwa.'),
 ('booking_confirmed_patient', 'en', 'TRANSACTIONAL', 'BOOKING_CONFIRMED', 'Booking confirmed', 'Your consultation booking has been confirmed.'),
 ('booking_confirmed_patient', 'sw', 'TRANSACTIONAL', 'BOOKING_CONFIRMED', 'Uhifadhi umethibitishwa', 'Uhifadhi wako wa ushauri umethibitishwa.'),

 ('booking_rescheduled_doctor', 'en', 'TRANSACTIONAL', 'BOOKING_RESCHEDULED', 'Consultation rescheduled', 'A consultation has been rescheduled. Check the app for the new time.'),
 ('booking_rescheduled_doctor', 'sw', 'TRANSACTIONAL', 'BOOKING_RESCHEDULED', 'Ushauri umepangwa upya', 'Ushauri umepangwa upya. Angalia programu kwa muda mpya.'),
 ('booking_rescheduled_patient', 'en', 'TRANSACTIONAL', 'BOOKING_RESCHEDULED', 'Consultation rescheduled', 'Your consultation has been rescheduled. Check the app for the new time.'),
 ('booking_rescheduled_patient', 'sw', 'TRANSACTIONAL', 'BOOKING_RESCHEDULED', 'Ushauri umepangwa upya', 'Ushauri wako umepangwa upya. Angalia programu kwa muda mpya.'),

 ('prescription_available', 'en', 'CLINICAL', 'PRESCRIPTION_AVAILABLE', 'Prescription available', 'A new prescription is available for your consultation.'),
 ('prescription_available', 'sw', 'CLINICAL', 'PRESCRIPTION_AVAILABLE', 'Dawa zinapatikana', 'Agizo jipya la dawa linapatikana kwa ushauri wako.'),

 ('refund_approved', 'en', 'TRANSACTIONAL', 'REFUND_APPROVED', 'Refund approved', 'Your refund request has been approved and will be processed. No funds have been returned yet.'),
 ('refund_approved', 'sw', 'TRANSACTIONAL', 'REFUND_APPROVED', 'Marejesho yameidhinishwa', 'Ombi lako la marejesho limeidhinishwa na litashughulikiwa. Fedha bado hazijarejeshwa.'),

 ('refund_processing', 'en', 'TRANSACTIONAL', 'REFUND_PROCESSING', 'Refund processing', 'Your refund is now being processed.'),
 ('refund_processing', 'sw', 'TRANSACTIONAL', 'REFUND_PROCESSING', 'Marejesho yanashughulikiwa', 'Marejesho yako sasa yanashughulikiwa.'),

 ('refund_completed', 'en', 'TRANSACTIONAL', 'REFUND_COMPLETED', 'Refund completed', 'Your refund has been completed.'),
 ('refund_completed', 'sw', 'TRANSACTIONAL', 'REFUND_COMPLETED', 'Marejesho yamekamilika', 'Marejesho yako yamekamilika.'),

 ('consultation_ready', 'en', 'CLINICAL', 'CONSULTATION_READY', 'Consultation ready', 'Your consultation is ready to join.'),
 ('consultation_ready', 'sw', 'CLINICAL', 'CONSULTATION_READY', 'Ushauri uko tayari', 'Ushauri wako uko tayari kujiunga.'),

 ('consultation_rejoin', 'en', 'CLINICAL', 'CONSULTATION_REJOIN', 'Rejoin available', 'Both participants have left the consultation room. Rejoin before the grace period ends to continue, or it will be marked interrupted.'),
 ('consultation_rejoin', 'sw', 'CLINICAL', 'CONSULTATION_REJOIN', 'Unaweza kujiunga tena', 'Washiriki wote wameondoka kwenye chumba cha ushauri. Jiunge tena kabla muda wa neema haujaisha, la sivyo itawekwa alama ya kukatizwa.'),

 ('consultation_interrupted', 'en', 'CLINICAL', 'CONSULTATION_INTERRUPTED', 'Consultation interrupted', 'This consultation was interrupted by a connection issue and could not continue. It has not been marked completed.'),
 ('consultation_interrupted', 'sw', 'CLINICAL', 'CONSULTATION_INTERRUPTED', 'Ushauri umekatizwa', 'Ushauri huu ulikatizwa kwa tatizo la mtandao na haukuweza kuendelea. Haujawekwa alama ya kukamilika.'),

 ('consultation_needs_review', 'en', 'OPERATIONAL', 'CONSULTATION_NEEDS_REVIEW', 'Consultation needs review', 'A consultation was interrupted and needs Admin review.'),
 ('consultation_needs_review', 'sw', 'OPERATIONAL', 'CONSULTATION_NEEDS_REVIEW', 'Ushauri unahitaji ukaguzi', 'Ushauri ulikatizwa na unahitaji ukaguzi wa Msimamizi.'),

 ('consultation_recovery_resolved_pending', 'en', 'CLINICAL', 'CONSULTATION_RECOVERY_RESOLVED', 'Consultation review resolved', 'Admin review of your interrupted consultation is complete. A financial review is still pending; nothing has been refunded or paid out yet.'),
 ('consultation_recovery_resolved_pending', 'sw', 'CLINICAL', 'CONSULTATION_RECOVERY_RESOLVED', 'Ukaguzi wa ushauri umekamilika', 'Ukaguzi wa Msimamizi kuhusu ushauri wako uliokatizwa umekamilika. Ukaguzi wa kifedha bado unaendelea; hakuna fedha zilizorejeshwa au kulipwa bado.'),
 ('consultation_recovery_resolved_no_settlement', 'en', 'CLINICAL', 'CONSULTATION_RECOVERY_RESOLVED', 'Consultation review resolved', 'Admin review of your interrupted consultation is complete. No further financial action is required.'),
 ('consultation_recovery_resolved_no_settlement', 'sw', 'CLINICAL', 'CONSULTATION_RECOVERY_RESOLVED', 'Ukaguzi wa ushauri umekamilika', 'Ukaguzi wa Msimamizi kuhusu ushauri wako uliokatizwa umekamilika. Hakuna hatua zaidi za kifedha zinazohitajika.'),

 ('consultation_completed', 'en', 'CLINICAL', 'CONSULTATION_COMPLETED', 'Consultation completed', 'Your consultation has been completed.'),
 ('consultation_completed', 'sw', 'CLINICAL', 'CONSULTATION_COMPLETED', 'Ushauri umekamilika', 'Ushauri wako umekamilika.')
ON CONFLICT (template_key, locale) DO NOTHING;

-- ---------------------------------------------------------------------
-- `appointment_notification_recipients` (20261120000001) now also returns
-- each recipient's own `preferred_language`, so a rendered notification can
-- use the ACTUAL recipient's language rather than a second query under a
-- role that may not have direct `users` access (`hellodoctor_payment_worker`/
-- `hellodoctor_finance_worker` have no grant on `users` at all — confirmed
-- by reading `20261026000001_canonical_user_isolation`'s grant list, not
-- assumed). A function's return-column list cannot be changed with
-- `CREATE OR REPLACE`; this is forward-only (a new migration dropping and
-- recreating the function), never an edit of the original file.
-- ---------------------------------------------------------------------
DROP FUNCTION appointment_notification_recipients(UUID);
CREATE FUNCTION appointment_notification_recipients(p_appointment UUID)
RETURNS TABLE(
    doctor_user_id UUID, doctor_language VARCHAR(5),
    patient_user_id UUID, patient_language VARCHAR(5)
)
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT d.user_id, du.preferred_language,
        COALESCE(p.user_id, g.user_id),
        COALESCE(pu.preferred_language, gu.preferred_language)
   FROM appointments a
   JOIN doctor_profiles d ON d.id = a.doctor_id
   JOIN users du ON du.id = d.user_id
   JOIN patient_profiles p ON p.id = a.patient_id
   LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id
   LEFT JOIN users pu ON pu.id = p.user_id
   LEFT JOIN users gu ON gu.id = g.user_id
  WHERE a.id = p_appointment AND consultation_participant_allowed(p_appointment)
$$;
REVOKE ALL ON FUNCTION appointment_notification_recipients(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION appointment_notification_recipients(UUID)
    TO hellodoctor_api, hellodoctor_payment_worker, hellodoctor_finance_worker;
