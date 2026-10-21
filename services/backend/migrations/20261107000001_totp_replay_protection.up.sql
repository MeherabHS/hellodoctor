-- Adds single-use/replay protection for TOTP second-factor verification
-- (HelloDoctor security remediation campaign, finding "TOTP single-use
-- hardening"). Previously `crypto::verify_totp` only checked the submitted
-- code against the current time window (±1 step skew), so a captured valid
-- code could be replayed for up to ~60-90s. This column records the Unix
-- time-step (floor(unix_timestamp / 30)) of the last successfully verified
-- code per credential; a new verification attempt must present a step
-- strictly greater than this value to be accepted.
ALTER TABLE mfa_credentials
    ADD COLUMN last_totp_step BIGINT;
