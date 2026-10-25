-- Notification Milestones 4 + 5 — Personalized Engagement and Kenyan Holiday
-- Notifications. Reuses the existing centralized outbox
-- (`notifications`/`notification_templates`/`notification_type_registry`/
-- `notification_preferences`), the Rust scheduler and the FCM transport —
-- forward-only, no parallel campaign engine, no new scheduler process.

-- ---------------------------------------------------------------------
-- Three new notification types this milestone actually emits.
-- `DOCTOR_GREETING` mirrors the already-existing `PATIENT_GREETING` (good-
-- morning engagement) for the Doctor side, which had no equivalent before.
-- `FIRST_CONSULTATION_MILESTONE` and `NEW_ARTICLE_PUBLISHED` are genuinely
-- new, event-driven engagement types. This is forward-only: the historical
-- CHECK constraint from `20261117000001` is dropped and recreated with the
-- 3 additional values, never edited in place.
-- ---------------------------------------------------------------------
ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
-- Full base list copied from the MOST RECENT prior owner of this constraint
-- (`20261121000001_milestone2_consultation_lifecycle_notifications`, not the
-- older `20261117000001` base) — that migration's own additions
-- (`NEW_CHAT_MESSAGE`, `REFUND_APPROVED/PROCESSING/COMPLETED`,
-- `CONSULTATION_COMPLETED`, `CONSULTATION_RECOVERY_RESOLVED`) must be
-- preserved here, not silently dropped, since a `DROP CONSTRAINT` + `ADD
-- CONSTRAINT` fully replaces the list rather than extending it.
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'REGISTRATION_SUBMITTED', 'REGISTRATION_VERIFIED', 'REGISTRATION_REJECTED',
    'PROFESSIONAL_VERIFICATION_REVOKED', 'ACCOUNT_SUSPENDED', 'ACCOUNT_REACTIVATED',
    'ACCOUNT_UPDATED', 'BOOKING_HOLD_CREATED', 'BOOKING_HOLD_ONE_MINUTE',
    'BOOKING_HOLD_EXPIRED', 'BOOKING_CONFIRMED', 'BOOKING_POSTPONED',
    'BOOKING_RESCHEDULED', 'BOOKING_CANCELLED', 'PAYMENT_HOLD_UPDATED',
    'CONSULTATION_REMINDER_15M', 'CONSULTATION_READY', 'CONSULTATION_REJOIN',
    'CONSULTATION_INTERRUPTED', 'CONSULTATION_NEEDS_REVIEW',
    'CHAT_MESSAGE', 'NEW_CHAT_MESSAGE', 'DOCTOR_REPLY', 'PRESCRIPTION_AVAILABLE',
    'MEDICAL_RECORD_UPDATED', 'EARNING_CREDITED', 'PAYOUT_PAID',
    'DAILY_CONSULTATION_SUMMARY', 'PATIENT_GREETING', 'HOLIDAY_GREETING',
    'BIRTHDAY_GREETING', 'WELCOME_GREETING', 'HEALTH_EDUCATION',
    'SERVICE_ANNOUNCEMENT', 'MAINTENANCE', 'ADMIN_CAMPAIGN',
    'ARTICLE_STATUS_CHANGED',
    'REFUND_APPROVED', 'REFUND_PROCESSING', 'REFUND_COMPLETED',
    'CONSULTATION_COMPLETED', 'CONSULTATION_RECOVERY_RESOLVED',
    'DOCTOR_GREETING', 'FIRST_CONSULTATION_MILESTONE', 'NEW_ARTICLE_PUBLISHED'
));

INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day) VALUES
 ('DOCTOR_GREETING', 'ENGAGEMENT', true, true, 1),
 ('FIRST_CONSULTATION_MILESTONE', 'ENGAGEMENT', true, true, 1),
 ('NEW_ARTICLE_PUBLISHED', 'ENGAGEMENT', true, true, 3)
ON CONFLICT (notification_type) DO NOTHING;

-- ---------------------------------------------------------------------
-- New preference columns. `greeting_notifications_enabled` is a narrower,
-- separate axis from `engagement_enabled` — it gates ONLY the four
-- recurring "greeting"-style types (`PATIENT_GREETING`/`DOCTOR_GREETING`/
-- `BIRTHDAY_GREETING`/`HOLIDAY_GREETING`), per the explicit instruction
-- "Patients should have greetings disabled by default" (a stricter default
-- than the existing `engagement_enabled`, which stays default-on for
-- event-driven engagement like `WELCOME_GREETING`/`FIRST_CONSULTATION_MILESTONE`/
-- `HEALTH_EDUCATION`/`NEW_ARTICLE_PUBLISHED`). The column default here is
-- just the SQL-level safety net (FALSE, the strictest case); the real
-- role-aware default (Doctors default on, Patients default off) is computed
-- in `notification_repo::user_notification_preferences` for a user with no
-- row yet — see that function's own doc comment.
--
-- `religious_greetings_enabled` is a second, independent opt-in gating
-- religious-origin holidays (Good Friday, Easter Monday, Christmas, Eid
-- al-Fitr, Eid al-Adha — see `kenyan_holidays.is_religious` below) for
-- EVERY user identically, regardless of role or any other signal —
-- deliberately never inferred from name, ethnicity or medical information,
-- per the explicit instruction. Default FALSE for everyone.
-- ---------------------------------------------------------------------
ALTER TABLE notification_preferences
    ADD COLUMN greeting_notifications_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN religious_greetings_enabled BOOLEAN NOT NULL DEFAULT FALSE;

-- ---------------------------------------------------------------------
-- Maintained Kenyan public-holiday calendar. Fixed-date civic holidays are
-- generated mechanically for a wide year range below (legally fixed
-- calendar dates under the Public Holidays Act, Cap. 110 — not a guess).
-- Movable/lunar-dependent holidays (Good Friday, Easter Monday, Eid al-Fitr,
-- Eid al-Adha) are stored as explicit, individually verified per-year rows
-- — never computed or hardcoded as a recurring formula — and this table is
-- the intended place to add future years as they are gazetted/confirmed
-- ("maintained calendar"), not a one-time hardcode.
-- ---------------------------------------------------------------------
CREATE TABLE kenyan_holidays (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    holiday_key VARCHAR(40) NOT NULL,
    observed_date DATE NOT NULL,
    is_religious BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (holiday_key, observed_date)
);
CREATE INDEX idx_kenyan_holidays_date ON kenyan_holidays(observed_date);

ALTER TABLE kenyan_holidays ENABLE ROW LEVEL SECURITY;
ALTER TABLE kenyan_holidays FORCE ROW LEVEL SECURITY;
CREATE POLICY kenyan_holidays_system_all ON kenyan_holidays
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
GRANT SELECT, INSERT, UPDATE, DELETE ON kenyan_holidays TO hellodoctor_system_worker;

-- Fixed-date civic holidays, 2025-2032 (legally fixed under Cap. 110 — safe
-- to generate mechanically for a wide range; none are religious-origin).
INSERT INTO kenyan_holidays (holiday_key, observed_date, is_religious)
SELECT 'new_year', make_date(y, 1, 1), false FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'labour_day', make_date(y, 5, 1), false FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'madaraka_day', make_date(y, 6, 1), false FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'mazingira_day', make_date(y, 10, 10), false FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'mashujaa_day', make_date(y, 10, 20), false FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'jamhuri_day', make_date(y, 12, 12), false FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'christmas', make_date(y, 12, 25), true FROM generate_series(2025, 2032) AS y
UNION ALL
SELECT 'boxing_day', make_date(y, 12, 26), false FROM generate_series(2025, 2032) AS y
ON CONFLICT (holiday_key, observed_date) DO NOTHING;

-- Movable holidays — explicit, individually verified dates only.
-- Good Friday / Easter Monday: verified against the standard Gregorian
-- computus and cross-checked against multiple independent public calendar
-- sources (timeanddate.com, officeholidays.com and others) as of this
-- migration's authoring date (2026-10-23).
INSERT INTO kenyan_holidays (holiday_key, observed_date, is_religious) VALUES
 ('good_friday', '2026-04-03', true),
 ('easter_monday', '2026-04-06', true),
 ('good_friday', '2027-03-26', true),
 ('easter_monday', '2027-03-29', true)
ON CONFLICT (holiday_key, observed_date) DO NOTHING;

-- Eid al-Fitr / Eid al-Adha are lunar-sighting-dependent and are inserted
-- only after the Kenyan Gazette confirms the date. The 2026 Idd-ul-Fitr
-- row is backed by Gazette Notice No. 3955 dated 18 March 2026. Future
-- Eid rows are deliberately not guessed; an absent row (and therefore no
-- greeting) is the safe outcome until an official notice is added here.
-- A corrected gazette date can be maintained with an UPDATE/DELETE+INSERT.
-- dates are deliberately NOT seeded: every source consulted explicitly
-- flagged them "tentative" with disagreement between calendars — per this
-- milestone's own instruction ("do not guess"), an absent row (no
-- notification sent) is the safe outcome, not a best-guess date that could
INSERT INTO kenyan_holidays (holiday_key, observed_date, is_religious) VALUES
 ('eid_al_fitr', '2026-03-20', true)
ON CONFLICT (holiday_key, observed_date) DO NOTHING;

-- ---------------------------------------------------------------------
-- Atomic, concurrency-safe per-(user, type, day) frequency-cap counter.
-- Milestone 3's original gate counted rows already `SENT_TO_PROVIDER` via a
-- plain `SELECT COUNT(*)` — safe for one worker, but two delivery-pass
-- instances racing on the same user+type could both read the same
-- under-cap count before either commits, both proceeding and exceeding the
-- cap by up to (worker count - 1). This table backs a single atomic
-- `INSERT ... ON CONFLICT DO UPDATE ... WHERE ... RETURNING` "claim a slot"
-- statement (see `notification_repo::claim_frequency_slot`), which Postgres
-- serializes via the row's own lock — safe under any number of concurrent
-- workers. `notifications.frequency_claimed` (below) makes a claim
-- idempotent per notification row, so a transient-failure retry of the same
-- row never claims a second slot for what is logically one notification.
-- ---------------------------------------------------------------------
CREATE TABLE notification_frequency_counters (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type VARCHAR(40) NOT NULL,
    local_date DATE NOT NULL,
    sent_count INTEGER NOT NULL DEFAULT 0 CHECK (sent_count >= 0),
    PRIMARY KEY (user_id, notification_type, local_date)
);
ALTER TABLE notification_frequency_counters ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_frequency_counters FORCE ROW LEVEL SECURITY;
CREATE POLICY notification_frequency_counters_system_all ON notification_frequency_counters
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
GRANT SELECT, INSERT, UPDATE ON notification_frequency_counters TO hellodoctor_system_worker;

ALTER TABLE notifications ADD COLUMN frequency_claimed BOOLEAN NOT NULL DEFAULT FALSE;

-- ---------------------------------------------------------------------
-- English + Kiswahili templates for every Milestone 4/5 type, plus one
-- template per holiday_key (differentiated within the single
-- `HOLIDAY_GREETING` notification_type exactly like the existing
-- `booking_confirmed_doctor`/`booking_confirmed_patient` pattern).
-- ---------------------------------------------------------------------
INSERT INTO notification_templates (template_key, locale, category, notification_type, title_template, body_template) VALUES
 ('patient_greeting', 'en', 'ENGAGEMENT', 'PATIENT_GREETING', 'Good morning', 'Good morning! Wishing you a healthy week ahead with AfyaMkononi.'),
 ('patient_greeting', 'sw', 'ENGAGEMENT', 'PATIENT_GREETING', 'Habari za asubuhi', 'Habari za asubuhi! Tunakutakia wiki njema na yenye afya na AfyaMkononi.'),

 ('doctor_greeting', 'en', 'ENGAGEMENT', 'DOCTOR_GREETING', 'Good morning, Doctor', 'Good morning! Wishing you a productive week ahead with AfyaMkononi.'),
 ('doctor_greeting', 'sw', 'ENGAGEMENT', 'DOCTOR_GREETING', 'Habari za asubuhi, Daktari', 'Habari za asubuhi! Tunakutakia wiki yenye mafanikio na AfyaMkononi.'),

 ('welcome_greeting', 'en', 'ENGAGEMENT', 'WELCOME_GREETING', 'Welcome to AfyaMkononi', 'Welcome to AfyaMkononi! Your profile is complete — you can now book a consultation whenever you need one.'),
 ('welcome_greeting', 'sw', 'ENGAGEMENT', 'WELCOME_GREETING', 'Karibu AfyaMkononi', 'Karibu AfyaMkononi! Wasifu wako umekamilika — sasa unaweza kuhifadhi ushauri wakati wowote unaohitaji.'),
 ('doctor_welcome_greeting', 'en', 'ENGAGEMENT', 'WELCOME_GREETING', 'Welcome to AfyaMkononi', 'Welcome to AfyaMkononi! Your professional account is now active.'),
 ('doctor_welcome_greeting', 'sw', 'ENGAGEMENT', 'WELCOME_GREETING', 'Karibu AfyaMkononi', 'Karibu AfyaMkononi! Akaunti yako ya kitaaluma sasa imewezeshwa.'),

 ('birthday_greeting', 'en', 'ENGAGEMENT', 'BIRTHDAY_GREETING', 'Happy Birthday!', 'Happy Birthday from everyone at AfyaMkononi! We wish you good health in the year ahead.'),
 ('birthday_greeting', 'sw', 'ENGAGEMENT', 'BIRTHDAY_GREETING', 'Heri ya Kuzaliwa!', 'Heri ya Kuzaliwa kutoka kwa familia ya AfyaMkononi! Tunakutakia afya njema mwaka huu.'),

 ('first_consultation_milestone', 'en', 'ENGAGEMENT', 'FIRST_CONSULTATION_MILESTONE', 'Your first consultation is complete', 'Congratulations on completing your first consultation with AfyaMkononi! We are glad to be part of your health journey.'),
 ('first_consultation_milestone', 'sw', 'ENGAGEMENT', 'FIRST_CONSULTATION_MILESTONE', 'Ushauri wako wa kwanza umekamilika', 'Hongera kwa kukamilisha ushauri wako wa kwanza na AfyaMkononi! Tunafurahi kuwa sehemu ya safari yako ya afya.'),

 ('health_education', 'en', 'ENGAGEMENT', 'HEALTH_EDUCATION', 'This week''s health tips', 'New health tips and educational articles are available in AfyaMkononi. Open the app to read them.'),
 ('health_education', 'sw', 'ENGAGEMENT', 'HEALTH_EDUCATION', 'Vidokezo vya afya vya wiki hii', 'Vidokezo vipya vya afya na makala za kielimu zinapatikana kwenye AfyaMkononi. Fungua programu kuzisoma.'),

 ('new_article_published', 'en', 'ENGAGEMENT', 'NEW_ARTICLE_PUBLISHED', 'New health article available', 'A new health article is available in AfyaMkononi. Open the app to read it.'),
 ('new_article_published', 'sw', 'ENGAGEMENT', 'NEW_ARTICLE_PUBLISHED', 'Makala mpya ya afya inapatikana', 'Makala mpya ya afya inapatikana kwenye AfyaMkononi. Fungua programu kulisoma.'),

 ('new_year', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy New Year!', 'Happy New Year from everyone at AfyaMkononi! Wishing you good health in the year ahead.'),
 ('new_year', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Mwaka Mpya!', 'Heri ya Mwaka Mpya kutoka kwa familia ya AfyaMkononi! Tunakutakia afya njema mwaka huu.'),

 ('labour_day', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Labour Day', 'Happy Labour Day from AfyaMkononi! We appreciate everything you do.'),
 ('labour_day', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Siku ya Wafanyakazi', 'Heri ya Siku ya Wafanyakazi kutoka AfyaMkononi! Tunathamini kila unachofanya.'),

 ('madaraka_day', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Madaraka Day', 'Happy Madaraka Day from AfyaMkononi! Wishing you and your family a wonderful celebration.'),
 ('madaraka_day', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Siku ya Madaraka', 'Heri ya Siku ya Madaraka kutoka AfyaMkononi! Tunakutakia wewe na familia yako sherehe njema.'),

 ('mazingira_day', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Mazingira Day', 'Happy Mazingira Day from AfyaMkononi! Let''s keep caring for our environment and our health.'),
 ('mazingira_day', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Siku ya Mazingira', 'Heri ya Siku ya Mazingira kutoka AfyaMkononi! Hebu tuendelee kutunza mazingira yetu na afya yetu.'),

 ('mashujaa_day', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Mashujaa Day', 'Happy Mashujaa Day from AfyaMkononi! We honor all who have contributed to our nation.'),
 ('mashujaa_day', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Siku ya Mashujaa', 'Heri ya Siku ya Mashujaa kutoka AfyaMkononi! Tunawaenzi wote waliochangia taifa letu.'),

 ('jamhuri_day', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Jamhuri Day', 'Happy Jamhuri Day from AfyaMkononi! Wishing you and your family a wonderful celebration.'),
 ('jamhuri_day', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Siku ya Jamhuri', 'Heri ya Siku ya Jamhuri kutoka AfyaMkononi! Tunakutakia wewe na familia yako sherehe njema.'),

 ('christmas', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Merry Christmas', 'Merry Christmas from everyone at AfyaMkononi! Wishing you a joyful and healthy holiday season.'),
 ('christmas', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Krismasi', 'Heri ya Krismasi kutoka kwa familia ya AfyaMkononi! Tunakutakia msimu wa furaha na afya njema.'),

 ('boxing_day', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Boxing Day', 'Happy Boxing Day from everyone at AfyaMkononi! Wishing you a peaceful holiday.'),
 ('boxing_day', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Siku ya Boxing', 'Heri ya Siku ya Boxing kutoka kwa familia ya AfyaMkononi! Tunakutakia likizo yenye amani.'),

 ('good_friday', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Good Friday', 'Wishing you a peaceful Good Friday from everyone at AfyaMkononi.'),
 ('good_friday', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Ijumaa Kuu', 'Tunakutakia Ijumaa Kuu yenye amani kutoka kwa familia ya AfyaMkononi.'),

 ('easter_monday', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Happy Easter', 'Happy Easter from everyone at AfyaMkononi! Wishing you a joyful celebration with family.'),
 ('easter_monday', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Heri ya Pasaka', 'Heri ya Pasaka kutoka kwa familia ya AfyaMkononi! Tunakutakia sherehe yenye furaha na familia.'),

 ('eid_al_fitr', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Eid Mubarak', 'Eid Mubarak from everyone at AfyaMkononi! Wishing you a blessed Eid al-Fitr.'),
 ('eid_al_fitr', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Eid Mubarak', 'Eid Mubarak kutoka kwa familia ya AfyaMkononi! Tunakutakia Eid al-Fitr yenye baraka.'),

 ('eid_al_adha', 'en', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Eid Mubarak', 'Eid Mubarak from everyone at AfyaMkononi! Wishing you a blessed Eid al-Adha.'),
 ('eid_al_adha', 'sw', 'ENGAGEMENT', 'HOLIDAY_GREETING', 'Eid Mubarak', 'Eid Mubarak kutoka kwa familia ya AfyaMkononi! Tunakutakia Eid al-Adha yenye baraka.')
ON CONFLICT (template_key, locale) DO NOTHING;

-- ---------------------------------------------------------------------
-- `NEW_ARTICLE_PUBLISHED` fan-out bookkeeping: marks an article as already
-- processed so the scheduler never re-scans it (and never retroactively
-- notifies patients who join long after an old article was published —
-- "prevent historical events from being sent late after downtime"
-- applies here too). `hellodoctor_system_worker` previously had no grant
-- on `doctor_articles` at all. The table's own
-- `guard_doctor_article_update` trigger (20261025000001) requires every
-- update to increment `version` by exactly one, including this one — the
-- scheduler's `UPDATE` therefore sets both columns together, exactly like
-- any other article edit.
-- ---------------------------------------------------------------------
ALTER TABLE doctor_articles ADD COLUMN engagement_notified_at TIMESTAMPTZ;

CREATE POLICY doctor_articles_system_select ON doctor_articles
    FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY doctor_articles_system_update ON doctor_articles
    FOR UPDATE TO hellodoctor_system_worker USING (true) WITH CHECK (true);
GRANT SELECT, UPDATE ON doctor_articles TO hellodoctor_system_worker;
