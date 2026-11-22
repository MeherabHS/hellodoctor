-- Profile photos for Admins (product owner, 2026-11-22: a signed-in Admin had
-- no profile page and no way to set a photo).
--
-- Admins have no profile table of their own (an Admin is a `users` row with
-- role ADMIN), so the photo reference gets its own one-row-per-Admin table
-- instead of new columns on `users`. Same object-reference shape as patient,
-- doctor and peer photos (20261112000001_profile_pictures,
-- 20261222000014_counterpart_photos): the encrypted bytes live in the private
-- object store; only the reference is kept here.
CREATE TABLE admin_profile_pictures (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    object_key TEXT NOT NULL,
    bucket TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

ALTER TABLE admin_profile_pictures ENABLE ROW LEVEL SECURITY;
ALTER TABLE admin_profile_pictures FORCE ROW LEVEL SECURITY;

-- An Admin reads and writes only their own row. Nobody else, other Admins
-- included, sees it: no screen shows one Admin another Admin's photo.
CREATE POLICY admin_profile_pictures_own ON admin_profile_pictures FOR ALL
    USING (
        current_setting('app.current_role', true) = 'ADMIN'
        AND user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    )
    WITH CHECK (
        current_setting('app.current_role', true) = 'ADMIN'
        AND user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    );

GRANT SELECT, INSERT, UPDATE, DELETE ON admin_profile_pictures TO hellodoctor_api;
