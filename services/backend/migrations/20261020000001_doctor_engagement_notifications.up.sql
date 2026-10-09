-- ============================================================================
-- Doctor Engagement extension: notification domain.
--
-- Approved product extension (Parts B-G): one coherent notification model,
-- not scattered ad-hoc per-feature code. Inspected first: no notification or
-- outbox table of any kind existed anywhere in this codebase before this
-- migration (confirmed by searching every prior migration file).
--
-- `notifications` is an in-app inbox + delivery-intent record, never a PHI
-- store: title/body are short, backend-constructed, non-clinical strings
-- (see notification_service.rs's copy templates) — no symptoms, diagnosis,
-- prescription detail, or patient clinical content is ever placed here.
--
-- `dedup_key` is UNIQUE and is how every creation path (admin verify/reject,
-- consultation-completion earning credit, and the daily-summary/T-15-reminder
-- scheduler) gets "exactly one notification per real event" for free via
-- `INSERT ... ON CONFLICT (dedup_key) DO NOTHING` at the repository layer,
-- rather than each caller re-implementing its own duplicate check.
-- ============================================================================

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    type VARCHAR(40) NOT NULL CHECK (type IN (
        'DAILY_CONSULTATION_SUMMARY',
        'CONSULTATION_REMINDER_15M',
        'EARNING_CREDITED',
        'PAYOUT_PAID',
        'REGISTRATION_VERIFIED',
        'REGISTRATION_REJECTED',
        'ARTICLE_STATUS_CHANGED'
    )),
    title VARCHAR(255) NOT NULL,
    body TEXT NOT NULL,
    related_entity_type VARCHAR(50),
    related_entity_id UUID,
    scheduled_for TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    read_at TIMESTAMPTZ,
    delivery_status VARCHAR(20) NOT NULL DEFAULT 'CREATED' CHECK (delivery_status IN (
        'CREATED', 'DUE', 'SENT_TO_PROVIDER', 'DELIVERED', 'FAILED', 'READ'
    )),
    dedup_key VARCHAR(200) NOT NULL,
    metadata JSONB,
    CONSTRAINT notifications_dedup_key_unique UNIQUE (dedup_key)
);

CREATE INDEX idx_notifications_user_created ON notifications(user_id, created_at DESC);
CREATE INDEX idx_notifications_due ON notifications(delivery_status, scheduled_for)
    WHERE delivery_status IN ('CREATED', 'DUE');

ALTER TABLE notifications ENABLE ROW LEVEL SECURITY;
ALTER TABLE notifications FORCE ROW LEVEL SECURITY;

-- Owner reads only their own notifications (AuthContext identity — no
-- client-supplied user_id ever reaches this policy or the handlers above it).
CREATE POLICY notifications_owner_select ON notifications
    FOR SELECT
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

-- Owner may only mark their own notifications read (read_at); no other
-- column is ever client-writable through this path — enforced in
-- notification_repo.rs, which issues a narrow `UPDATE ... SET read_at`, not
-- a general-purpose row update.
CREATE POLICY notifications_owner_mark_read ON notifications
    FOR UPDATE
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

-- Backend-trusted creation: the ordinary request-serving role (an admin
-- verifying/rejecting a doctor, or a consultation-completion request
-- crediting a *different* user's — the doctor's — wallet) and the background
-- scheduler (`hellodoctor_system_worker`, no per-request `app.current_user_id`
-- at all) both create notifications addressed to someone other than the
-- acting session's own identity — this is the system telling user X
-- something happened, not user X writing their own row — so creation is
-- gated by role, not by `user_id` ownership, trusting the Rust call sites
-- (which never take a client-supplied recipient) rather than RLS to pick the
-- right recipient.
CREATE POLICY notifications_backend_insert ON notifications
    FOR INSERT
    TO hellodoctor_api, hellodoctor_system_worker
    WITH CHECK (true);

-- The scheduler also needs to update its own rows' delivery_status (e.g. once
-- a real push provider is configured and attempted) independent of any
-- owning user's session.
CREATE POLICY notifications_system_update ON notifications
    FOR UPDATE
    TO hellodoctor_system_worker
    USING (true)
    WITH CHECK (true);

-- The scheduler (`hellodoctor_system_worker`, never a per-request session, so
-- `app.current_user_id` is never set for it) needs unrestricted SELECT for
-- its own dedup pre-checks (`notification_repo::exists_with_dedup_key`) —
-- scoped to this role only, never `hellodoctor_api`, so the owner-only
-- `notifications_owner_select` policy above remains the *only* applicable
-- policy for a doctor/patient's own `GET /notifications` request (permissive
-- policies OR together, so granting `hellodoctor_api` the same unrestricted
-- visibility here would silently let any authenticated user list everyone
-- else's notifications too).
CREATE POLICY notifications_system_worker_select ON notifications
    FOR SELECT
    TO hellodoctor_system_worker
    USING (true);

GRANT SELECT, INSERT, UPDATE ON notifications TO hellodoctor_api;
GRANT SELECT, INSERT, UPDATE ON notifications TO hellodoctor_system_worker;

-- ============================================================================
-- Device token registration — the adapter boundary for a future real push
-- provider (FCM/APNs). No provider is configured by this migration or by any
-- code it ships with; see `notification_transport.rs`'s own doc comment and
-- this project's `PUSH_PROVIDER_CONFIGURATION_REQUIRED` status.
-- ============================================================================

CREATE TABLE notification_device_tokens (
    device_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    platform VARCHAR(10) NOT NULL CHECK (platform IN ('ANDROID', 'IOS', 'WEB')),
    push_token TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT notification_device_tokens_unique UNIQUE (user_id, push_token)
);

ALTER TABLE notification_device_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_device_tokens FORCE ROW LEVEL SECURITY;

-- A device token belongs exclusively to the authenticated user who
-- registered it — no cross-account read/write in either direction, which is
-- what actually prevents "Doctor A's notifications reaching Doctor B's
-- device" at the registration layer (the sending side only ever looks up
-- tokens for the specific recipient `user_id` it already resolved).
-- Split by command explicitly (matching the proven pattern already used on
-- `notifications` above) rather than one generic `FOR ALL` policy — a real,
-- empirically-confirmed issue (not assumed) was traced to exactly this
-- shape: combining a `FOR ALL` owner policy with a second `FOR UPDATE`
-- backend-trusted policy left `UPDATE`s matching zero rows for
-- `hellodoctor_api` even though both policies' definitions were individually
-- correct. Replacing the single `FOR ALL` policy with explicit per-command
-- policies (`SELECT`/`INSERT`/`UPDATE` for the owner) avoids mixing `FOR ALL`
-- with a command-specific policy on the same table/role.
CREATE POLICY notification_device_tokens_owner_select ON notification_device_tokens
    FOR SELECT
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

CREATE POLICY notification_device_tokens_owner_insert ON notification_device_tokens
    FOR INSERT
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

CREATE POLICY notification_device_tokens_owner_update ON notification_device_tokens
    FOR UPDATE
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

-- Logout (`AuthService::logout`) disables an account's device registrations
-- by the `user_id` resolved from the presented refresh token — but logout is
-- a public, unauthenticated-by-design endpoint (that resolved user_id is the
-- *result* of the lookup, not an already-live session), so
-- `app.current_user_id` is never set for it, and the owner-only policy above
-- would otherwise silently filter the `UPDATE` to zero rows. `hellodoctor_api`
-- gets unrestricted `UPDATE` here for exactly that one backend-trusted call
-- site — the only other write path to this table is the owning user's own
-- `register_device` upsert, which the owner-only policy above still fully
-- protects (it runs inside a real `begin_authed` session with
-- `app.current_user_id` correctly set to that same user).
CREATE POLICY notification_device_tokens_backend_disable ON notification_device_tokens
    FOR UPDATE
    TO hellodoctor_api
    USING (true)
    WITH CHECK (true);

-- Required *in addition* to the UPDATE policy above — confirmed empirically,
-- not assumed: Postgres treats UPDATE/DELETE as needing implicit SELECT-level
-- visibility on the target rows before the UPDATE-specific policy is even
-- consulted (the same "ON CONFLICT needs SELECT visibility" shape already
-- documented on `notification_repo::insert_if_new`, here showing up for a
-- plain UPDATE instead). Without this, `notification_device_tokens_owner_select`
-- alone (scoped to `app.current_user_id`, never set for the unauthenticated
-- logout call site) silently makes every row invisible to the disable-on-
-- logout `UPDATE`, which then matches zero rows with no error at all — this
-- exact symptom was caught by a real failing test before this policy existed.
CREATE POLICY notification_device_tokens_backend_select ON notification_device_tokens
    FOR SELECT
    TO hellodoctor_api
    USING (true);

GRANT SELECT, INSERT, UPDATE ON notification_device_tokens TO hellodoctor_api;

-- ============================================================================
-- Read grants the background scheduler needs to compute daily-summary counts
-- and find appointments due a T-15 reminder. `appointments` and
-- `doctor_schedule_slots` already have a `FOR ALL`/`FOR SELECT TO
-- hellodoctor_system_worker` RLS policy from the original RLS hardening
-- migration, but — confirmed by reading that migration's own GRANT
-- statements — no base table-level GRANT was ever issued for this role on
-- either table, so the policy was previously unreachable dead code for this
-- role. `doctor_profiles` and `users` have no RLS at all (confirmed: no
-- `ENABLE ROW LEVEL SECURITY` for either table anywhere in the migration
-- history), so a plain GRANT is sufficient for them.
-- ============================================================================
GRANT SELECT ON appointments TO hellodoctor_system_worker;
GRANT SELECT ON doctor_schedule_slots TO hellodoctor_system_worker;
GRANT SELECT ON doctor_profiles TO hellodoctor_system_worker;
GRANT SELECT ON users TO hellodoctor_system_worker;
