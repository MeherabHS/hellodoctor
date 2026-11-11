#!/usr/bin/env bash
set -Eeuo pipefail

# DRAFT - disposable ARM64 CI only. Read-only/isolated-write probe against a
# disposable Postgres that the integration workflow tears down afterward;
# never point this at a real deployment's database.
#
# Verifies, against the LIVE deployed schema (not the sqlx::test ephemeral
# databases cargo test already covers), that the five capability roles from
# services/backend/migrations/20261003000001_rls_hardening.up.sql exist with
# the grant boundaries that migration documents, and that RLS actually
# filters rows for the ordinary API role. This is deliberately narrow: it
# proves the roles/policies made it onto the ARM64-built, freshly-migrated
# database exactly as committed - it does not re-derive or re-verify the
# policy design itself, which is cargo test's job (48 existing RLS/grant
# tests in services/backend/tests/api_tests.rs).

PGHOST="${PGHOST:-127.0.0.1}"
PGPORT="${PGPORT:-55432}"
PGUSER="${POSTGRES_USER:-postgres}"
PGDATABASE="${POSTGRES_DB:-hellodoctor}"
export PGPASSWORD="${POSTGRES_PASSWORD:?set the disposable Postgres password}"

# Fail-closed guard: this script runs real INSERT/UPDATE/DELETE statements
# (scoped to synthetic rows it creates itself) and is only ever meant to run
# against the disposable loopback Postgres the CI workflow just started. It
# refuses to run against anything else unless the caller explicitly
# acknowledges the target with ALLOW_NON_LOOPBACK_RLS_PROBE=yes - a copy-paste
# of this script's PGHOST into a real deployment's CI job is exactly the
# mistake this guard exists to catch.
case "$PGHOST" in
  127.0.0.1|localhost|::1) ;;
  *)
    if [ "${ALLOW_NON_LOOPBACK_RLS_PROBE:-}" != "yes" ]; then
      echo "FAIL: refusing to run against PGHOST='${PGHOST}' (not loopback). This script is for the disposable ARM64 CI Postgres only. Set ALLOW_NON_LOOPBACK_RLS_PROBE=yes only if you have independently confirmed this is a disposable, non-production database." >&2
      exit 1
    fi
    ;;
esac

psql_q() { psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -X -q -t -A -v ON_ERROR_STOP=1 "$@"; }

fail() { echo "FAIL: $*" >&2; exit 1; }

# Unique per invocation so concurrent/retried CI runs never collide on the
# UNIQUE(users.phone_number) constraint, and so cleanup below can be scoped
# precisely to rows this run created (never a blanket DELETE).
RUN_TAG="rlsprobe$$$(date +%s)"
PHONE_A="+2547000${RUN_TAG: -6}1"
PHONE_B="+2547000${RUN_TAG: -6}2"
CLEANUP_IDS=()

cleanup() {
  # Runs on both success and failure (trap EXIT) so a failed assertion never
  # leaves synthetic rows behind for the next run to collide with. Superuser
  # bypasses RLS, so this always reaches rows this run created regardless of
  # the test outcome above.
  if [ "${#CLEANUP_IDS[@]}" -gt 0 ]; then
    psql_q -c "DELETE FROM patient_profiles WHERE user_id = ANY(ARRAY['${CLEANUP_IDS[0]}','${CLEANUP_IDS[1]}']::uuid[]);" || true
    psql_q -c "DELETE FROM users WHERE id = ANY(ARRAY['${CLEANUP_IDS[0]}','${CLEANUP_IDS[1]}']::uuid[]);" || true
  fi
}
trap cleanup EXIT

echo "== Role existence and attributes =="
for role in hellodoctor_api hellodoctor_auth_worker hellodoctor_payment_worker \
            hellodoctor_system_worker hellodoctor_finance_worker; do
  row="$(psql_q -c "SELECT rolbypassrls, rolsuper FROM pg_roles WHERE rolname = '${role}'")"
  [ -n "$row" ] || fail "role ${role} does not exist on the freshly migrated database"
  [ "$row" = "f|f" ] || fail "role ${role} has unexpected attributes (bypassrls|super = $row), expected f|f"
  echo "PASS: ${role} exists, NOBYPASSRLS, not superuser"
done

echo "== Role switching is genuinely active (not a silent no-op) =="
# If SET LOCAL ROLE failed silently (typo, role missing, privilege issue),
# every assertion below would still "pass" vacuously under the superuser's
# own BYPASSRLS/ownership privileges - this would be a false PASS, not a
# real RLS verification. Prove current_user actually changed before trusting
# anything that follows.
switched_user="$(psql_q -c "BEGIN; SET LOCAL ROLE hellodoctor_api; SELECT current_user; COMMIT;")"
[ "$switched_user" = "hellodoctor_api" ] \
  || fail "SET LOCAL ROLE hellodoctor_api did not take effect (current_user='${switched_user}') - every check below would be meaningless under this condition"
echo "PASS: SET LOCAL ROLE hellodoctor_api genuinely changes current_user"

echo "== Grant-boundary negative test (no RLS session context needed) =="
# hellodoctor_api has SELECT/INSERT on appointments (see migration header) but
# NO UPDATE grant anywhere on appointments - lifecycle-state transitions are
# payment_worker/system_worker only. A plain GRANT check, independent of any
# RLS predicate.
set +e
out="$(psql_q -c "
  BEGIN;
  SET LOCAL ROLE hellodoctor_api;
  UPDATE appointments SET updated_at = now() WHERE false;
" 2>&1)"
rc=$?
set -e
[ "$rc" -ne 0 ] || fail "UPDATE appointments under hellodoctor_api unexpectedly SUCCEEDED (rc=0) - the grant boundary is not enforced"
echo "$out" | grep -qi "permission denied for table appointments" \
  || fail "expected 'permission denied for table appointments' under hellodoctor_api, got: $out (rc=$rc)"
echo "PASS: hellodoctor_api cannot UPDATE appointments (grant boundary enforced, rc=$rc)"

echo "== RLS negative test: cross-patient row visibility =="
# Create two disposable, run-scoped patient rows under postgres (superuser
# bypasses RLS for setup), then prove hellodoctor_api + app.current_user_id
# scoped to patient A cannot see patient B's row via patient_profiles_access.
ids="$(psql_q -c "
  WITH a AS (
    INSERT INTO users (id, phone_number, role, created_at)
    VALUES (gen_random_uuid(), '${PHONE_A}', 'PATIENT', now())
    RETURNING id
  ), b AS (
    INSERT INTO users (id, phone_number, role, created_at)
    VALUES (gen_random_uuid(), '${PHONE_B}', 'PATIENT', now())
    RETURNING id
  ), pa AS (
    INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at)
    SELECT gen_random_uuid(), a.id, 'RLS Probe A ${RUN_TAG}', DATE '1990-01-01', 'OTHER', now() FROM a RETURNING user_id
  ), pb AS (
    INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at)
    SELECT gen_random_uuid(), b.id, 'RLS Probe B ${RUN_TAG}', DATE '1990-01-01', 'OTHER', now() FROM b RETURNING user_id
  )
  SELECT (SELECT user_id FROM pa) || ' ' || (SELECT user_id FROM pb)
")"
patient_a="$(echo "$ids" | awk '{print $1}')"
patient_b="$(echo "$ids" | awk '{print $2}')"
[ -n "$patient_a" ] && [ -n "$patient_b" ] || fail "setup insert did not return two UUIDs (got: '$ids')"
CLEANUP_IDS=("$patient_a" "$patient_b")

visible_rows="$(psql_q -c "
  BEGIN;
  SET LOCAL ROLE hellodoctor_api;
  SET LOCAL app.current_role = 'PATIENT';
  SET LOCAL app.current_user_id = '${patient_a}';
  SELECT count(*) FROM patient_profiles WHERE user_id = '${patient_b}';
")"
[ "$visible_rows" = "0" ] || fail "patient A could see patient B's profile row under RLS (count=$visible_rows)"
echo "PASS: cross-patient row is invisible under hellodoctor_api + patient_profiles_access"

own_row="$(psql_q -c "
  BEGIN;
  SET LOCAL ROLE hellodoctor_api;
  SET LOCAL app.current_role = 'PATIENT';
  SET LOCAL app.current_user_id = '${patient_a}';
  SELECT count(*) FROM patient_profiles WHERE user_id = '${patient_a}';
")"
[ "$own_row" = "1" ] || fail "patient A's own profile row was unexpectedly hidden (count=$own_row), policy is likely mis-scoped"
echo "PASS: a patient's own row remains visible (policy is not accidentally denying everything)"

echo "ALL RLS/grant negative-access checks passed (run tag: ${RUN_TAG})."
