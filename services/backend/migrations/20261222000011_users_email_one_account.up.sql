-- One email belongs to exactly one account and therefore one role (Patient,
-- Doctor, Peer Navigator or Admin). `users_email_key` already rejects exact
-- duplicates; this also rejects the same address in a different letter case
-- from any write path that skipped normalisation.
CREATE UNIQUE INDEX IF NOT EXISTS users_email_lower_key
    ON users (lower(email))
    WHERE email IS NOT NULL;
