-- B-01 prevention control (docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md §3):
-- GET /healthz now compares this binary's embedded migration set against
-- what's actually applied, so a stale long-running process is detectable
-- instead of silently 500ing on real traffic. That check runs on the
-- ordinary request-serving `hellodoctor_api` pool, which has never had
-- read access to sqlx's own bookkeeping table.
GRANT SELECT ON _sqlx_migrations TO hellodoctor_api;
