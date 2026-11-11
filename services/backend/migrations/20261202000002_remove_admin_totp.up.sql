-- Admin login's mandatory TOTP second factor is removed: Admin now shares
-- the exact same single-factor (password + emailed OTP) login shape as
-- Doctor and Patient. No self-service TOTP enrollment endpoint ever
-- existed (a credential could only ever be provisioned by hand-inserting a
-- row), so the second factor added operational friction and a dedicated
-- code path without a corresponding reduction in actual risk versus the
-- OTP challenge/attempt-count/ip_jail protection every role already shares.
-- `mfa_credentials` had exactly one consumer (Admin TOTP); dropped along
-- with the enum it depended on. `pending_mfa_challenges` (the generic
-- OTP-challenge bootstrap table Patient/Doctor/Admin all still use) is
-- unrelated and untouched.
DROP TABLE mfa_credentials;
DROP TYPE mfa_method_enum;
