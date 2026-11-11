<#
.SYNOPSIS
  Mandatory pre-benchmark reset: restores the hellodoctor-m8-final-local disposable
  stack's Postgres database to the verified canonical baseline
  (docs/performance-evidence/milestone-8-final/remediation/canonical_baseline/
  canonical_baseline.dump), flushes the benchmark Redis instance, restarts the
  backend, and verifies every precondition a benchmark run depends on before
  allowing the caller to proceed.

  Built 2026-11-04 after discovering that the project's benchmark harness had
  never reset Postgres between runs: the same deterministic 2,750-patient subset
  this benchmark always exercises had accumulated 19,450 real appointments
  (avg 7.07/patient) purely from repeated unreset runs, which silently confounded
  every prior cold-vs-warm comparison. See docs/CHECKPOINT.md and
  docs/performance-evidence/milestone-8-final/remediation/phase7_warm_repeat_200rps/
  WARM_REPEAT_REPORT.md for the full incident.

  Per explicit product-owner instruction: selective TRUNCATE/DELETE or patient-pool
  rotation are NOT acceptable substitutes for this snapshot-restore mechanism. This
  script IS the mechanism. (The repo's cleanup_capacity_test_seed_data.sql +
  seed_capacity_dataset*.sql scripts were used ONCE, manually, to originally build
  canonical_baseline.dump -- they are a bootstrap tool, not meant to be re-run per
  benchmark; re-running them against a database that already has the dev-seed
  fixture mixed in is fine, since they're scoped by phone-number convention, but
  snapshot-restore is strictly faster and gives byte-identical state, which
  deleting-and-regenerating only approximates.)

  ABORTS (non-zero exit, specific message) on ANY verification failure. Never
  silently proceeds on an unverified database -- that silent-proceed behavior is
  exactly what caused the original confound.

.PARAMETER DumpPath
  Path to the canonical baseline dump. Defaults to the project's own verified one.

.PARAMETER ManifestPath
  Path to the JSON manifest of expected row counts / roles / RLS tables / dump
  hash this restore must match. Defaults alongside DumpPath.

.PARAMETER PostgresContainer / RedisContainer / BackendContainer
  Defaults match the hellodoctor-m8-final-local stack. This script refuses to run
  against any container name not matching 'hellodoctor-m8-final-local-*' unless
  -Force is passed, as a guard against accidentally pointing this at a shared/dev
  stack.

.PARAMETER BackendHealthUrl
  Defaults to the stack's direct backend port (not the proxy), matching what the
  benchmark harness already uses for /metrics.

.EXAMPLE
  ./restore_canonical_benchmark_baseline.ps1
  # Restores, verifies, and exits 0 only if every check passes -- safe to chain
  # with '; if ($LASTEXITCODE -eq 0) { <run benchmark> }' or PowerShell's own $?.
#>
param(
    [string]$DumpPath = (Join-Path $PSScriptRoot '..\..\..\docs\performance-evidence\milestone-8-final\remediation\canonical_baseline\canonical_baseline.dump'),
    [string]$ManifestPath = (Join-Path $PSScriptRoot '..\..\..\docs\performance-evidence\milestone-8-final\remediation\canonical_baseline\CANONICAL_BASELINE_MANIFEST.json'),
    [string]$PostgresContainer = 'hellodoctor-m8-final-local-postgres-1',
    [string]$RedisContainer = 'hellodoctor-m8-final-local-redis-1',
    [string]$BackendContainer = 'hellodoctor-m8-final-local-backend-1',
    [string]$BackendHealthUrl = 'http://127.0.0.1:58280/healthz',
    [string]$DbName = 'hellodoctor',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

function Abort($reason) {
    Write-Host ''
    Write-Host "RESTORE_ABORTED: $reason" -ForegroundColor Red
    Write-Host 'Benchmark run must NOT proceed. Nothing downstream should trust this database state.' -ForegroundColor Red
    exit 1
}

foreach ($c in @($PostgresContainer, $RedisContainer, $BackendContainer)) {
    if (-not $Force -and $c -notmatch '^hellodoctor-m8-final-local-') {
        Abort "Container '$c' does not match the expected hellodoctor-m8-final-local-* naming -- refusing to restore against a possibly shared/dev/production stack. Pass -Force to override if this is intentional."
    }
}

if (-not (Test-Path $DumpPath)) { Abort "Dump file not found: $DumpPath" }
if (-not (Test-Path $ManifestPath)) { Abort "Manifest file not found: $ManifestPath" }
$DumpPath = (Resolve-Path $DumpPath).ProviderPath
$manifest = Get-Content $ManifestPath -Raw | ConvertFrom-Json

Write-Host "=== Canonical baseline restore: $DumpPath ==="

# ---- 0. Confirm the dump on disk still matches the manifest's recorded hash.
# Catches someone silently swapping the dump file for a different one.
$actualHash = (Get-FileHash -Path $DumpPath -Algorithm SHA256).Hash.ToLowerInvariant()
$expectedHash = $manifest.dump_sha256.ToLowerInvariant()
if ($actualHash -ne $expectedHash) {
    Abort "Dump file sha256 mismatch. Expected $expectedHash, got $actualHash. The dump on disk does not match the manifest -- do not trust it."
}
Write-Host "[0/8] Dump hash verified: $actualHash"

# ---- 1. Copy dump into the Postgres container.
docker cp $DumpPath "${PostgresContainer}:/tmp/canonical_baseline_restore.dump"
if ($LASTEXITCODE -ne 0) { Abort 'docker cp of dump into postgres container failed.' }
Write-Host '[1/8] Dump copied into container.'

# ---- 2. Terminate other backends, drop + recreate the database.
# `pg_terminate_backend` then a SEPARATE `DROP DATABASE` is a TOCTOU race: a
# connection pool that reconnects in the gap between the two commands (e.g.
# the benchmark backend's own pool, freshly restarted by step [8/8] of a
# PRIOR run of this same script and actively reconnecting) can hold the
# database open again before the DROP statement runs -- confirmed live:
# "database "hellodoctor" is being accessed by other users" / "DROP DATABASE
# failed" when this script was invoked a second time shortly after a backend
# restart. Fixed with `DROP DATABASE ... WITH (FORCE)` (Postgres 13+; this
# stack runs 16), which disconnects other sessions and drops the database in
# one atomic statement -- no separate terminate step, no race window.
docker exec $PostgresContainer psql -U postgres -c "DROP DATABASE IF EXISTS $DbName WITH (FORCE);"
if ($LASTEXITCODE -ne 0) { Abort 'DROP DATABASE failed.' }
docker exec $PostgresContainer psql -U postgres -c "CREATE DATABASE $DbName;"
if ($LASTEXITCODE -ne 0) { Abort 'CREATE DATABASE failed.' }
Write-Host '[2/8] Database dropped and recreated.'

# ---- 3. Restore. Deliberately NO --no-owner/--no-privileges -- see this
# script's own header and CANONICAL_BASELINE.md: those flags strip the GRANTs
# to the five capability roles and crash-loop the backend on startup.
docker exec $PostgresContainer pg_restore -U postgres -d $DbName /tmp/canonical_baseline_restore.dump
if ($LASTEXITCODE -ne 0) { Abort 'pg_restore exited non-zero -- restored state is not trustworthy.' }
Write-Host '[3/8] pg_restore completed.'

# ---- 4. Row-count verification against the manifest. Exact match required.
$countSql = ($manifest.expected_row_counts.PSObject.Properties | ForEach-Object {
    "SELECT '$($_.Name)' AS t, count(*) AS n FROM $($_.Name)"
}) -join ' UNION ALL '
$countOutput = docker exec $PostgresContainer psql -U postgres -d $DbName -t -A -F',' -c $countSql
$mismatches = @()
foreach ($line in ($countOutput -split "`r?`n" | Where-Object { $_ })) {
    $parts = $line -split ','
    if ($parts.Count -ne 2) { continue }
    $table = $parts[0].Trim()
    $actual = [int]$parts[1].Trim()
    $expected = [int]$manifest.expected_row_counts.$table
    if ($actual -ne $expected) {
        $mismatches += "$table expected=$expected actual=$actual"
    }
}
if ($mismatches.Count -gt 0) {
    Abort "Row-count verification failed after restore:`n$($mismatches -join "`n")"
}
$expectedTableCount = @($manifest.expected_row_counts.PSObject.Properties).Count
Write-Host "[4/8] Row counts verified exactly against manifest ($expectedTableCount tables)."

# ---- 5. Capability roles + RLS forced-enabled verification.
$rolesOutput = docker exec $PostgresContainer psql -U postgres -d $DbName -t -A -c "SELECT rolname FROM pg_roles WHERE rolname LIKE 'hellodoctor_%' ORDER BY 1;"
$actualRoles = @($rolesOutput -split "`r?`n" | Where-Object { $_ } | ForEach-Object { $_.Trim() })
foreach ($expectedRole in $manifest.expected_capability_roles) {
    if ($actualRoles -notcontains $expectedRole) {
        Abort "Expected capability role '$expectedRole' missing after restore -- GRANTs likely stripped (check for --no-owner/--no-privileges having crept back in)."
    }
}
Write-Host "[5/8] All $($manifest.expected_capability_roles.Count) capability roles present."

$rlsTableList = ($manifest.rls_forced_tables | ForEach-Object { "'$_'" }) -join ','
$rlsSql = "SELECT relname, relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname IN ($rlsTableList) ORDER BY 1;"
$rlsOutput = docker exec $PostgresContainer psql -U postgres -d $DbName -t -A -F',' -c $rlsSql
$rlsRows = @($rlsOutput -split "`r?`n" | Where-Object { $_ })
if ($rlsRows.Count -ne $manifest.rls_forced_tables.Count) {
    Abort "Expected RLS status for $($manifest.rls_forced_tables.Count) tables, got $($rlsRows.Count) rows back -- a table may be missing."
}
foreach ($row in $rlsRows) {
    $p = $row -split ','
    if ($p[1].Trim() -ne 't' -or $p[2].Trim() -ne 't') {
        Abort "RLS not enabled+forced on table '$($p[0])' after restore (relrowsecurity=$($p[1]), relforcerowsecurity=$($p[2]))."
    }
}
Write-Host "[5/8] RLS enabled+forced on all $($rlsRows.Count) checked tables."

# ---- 6. Redis: deterministic empty state (not just the two rate-limit key
# families -- a full FLUSHDB, since this is restore-time, not warm-up-time).
docker exec $RedisContainer redis-cli FLUSHDB | Out-Null
$redisCount = (docker exec $RedisContainer redis-cli DBSIZE).Trim()
if ($redisCount -ne '0') { Abort "Redis FLUSHDB did not result in an empty database (DBSIZE=$redisCount)." }
Write-Host '[6/8] Redis flushed to empty.'

# ---- 7. (Former stale-slot time-shift step REMOVED 2026-11-04.) A first
# attempt here tried to UPDATE stale AVAILABLE slots' start_time/end_time
# forward by a shared delta. That is unsafe: doctor_schedule_slots has a GiST
# exclusion constraint (`no_overlapping_slots`, WHERE status != 'CANCELLED')
# preventing any doctor from having two overlapping slot ranges, and a
# clock_timestamp()-derived shift delta carries sub-minute fractional
# precision that does not line up with the existing 15-minute slot grid --
# confirmed live: this aborted a real validation run with "conflicting key
# value violates exclusion constraint no_overlapping_slots" because a shifted
# stale slot's new window partially overlapped an already-existing, untouched
# future slot for the same doctor. Root-caused and fixed properly instead, at
# the actual source of the problem: examples/capacity_test.rs's available_slots
# query was simply missing the `start_time > clock_timestamp()` filter that
# production code already applies correctly for this exact purpose (see
# src/repository/doctor_repo.rs's own available-slots query). That query-level
# fix needs zero data mutation, has zero collision risk, and stays correct no
# matter how many days pass before a given restore -- see that file's own
# comment for the full explanation. No DB-side step is needed here anymore.

# ---- 8. Reset the two web-login fixture accounts (admin@hellodoctor.health,
# sabrina.akter@hellodoctor.health) used only by web_workload.rs's Admin/Doctor
# login preflight -- NOT part of capacity_test.rs's measured patient traffic.
# canonical_baseline.dump captures these with a real (non-placeholder) Argon2id
# hash whose plaintext is genuinely unknown (correctly never stored anywhere),
# so every restore must re-set them to the same fixed, documented,
# never-production benchmark password already established by
# examples/print_benchmark_doctor_hash.rs ("AuthStressBenchmark-2026!"), or
# WEB_ADMIN_PASSWORD/WEB_DOCTOR_PASSWORD have nothing valid to authenticate
# against after a restore.
$benchmarkWebLoginHash = '$argon2id$v=19$m=19456,t=2,p=1$Z9oJXF1DKs7fZEdBsMCEtQ$kdadd/rSfwSdVeP6Vvto5ahTYkBU8eu5h6u0hyIJ/iE'
$resetSql = "UPDATE users SET password_hash = '$benchmarkWebLoginHash', updated_at = NOW() WHERE email IN ('admin@hellodoctor.health', 'sabrina.akter@hellodoctor.health') RETURNING email;"
$resetOutput = docker exec $PostgresContainer psql -U postgres -d $DbName -t -A -c $resetSql
# This container's psql still emits the "UPDATE n" command-completion tag as
# its own line even with -t -A (tuples-only normally suppresses it) -- confirmed
# live, so it must be filtered out explicitly rather than trusted to be absent.
$resetEmails = @($resetOutput -split "`r?`n" | Where-Object { $_ -and $_ -notmatch '^(UPDATE|INSERT|DELETE)\s+\d+$' })
if ($resetEmails.Count -ne 2) {
    Abort "Expected to reset exactly 2 web-login fixture accounts (admin@hellodoctor.health, sabrina.akter@hellodoctor.health), updated $($resetEmails.Count) -- one may be missing from this dump."
}
docker exec $PostgresContainer psql -U postgres -d $DbName -c "UPDATE auth_sessions SET revoked_at = NOW() WHERE user_id IN (SELECT id FROM users WHERE email IN ('admin@hellodoctor.health','sabrina.akter@hellodoctor.health')) AND revoked_at IS NULL;" | Out-Null
Write-Host '[7/8] Web-login fixture accounts (admin + doctor) reset to the documented benchmark password.'

# ---- 8. Restart backend, poll /healthz, verify migrations current.
docker restart $BackendContainer | Out-Null
$ready = $false
$deadline = (Get-Date).AddSeconds(60)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    try {
        $health = Invoke-WebRequest -Uri $BackendHealthUrl -UseBasicParsing -TimeoutSec 5
        $json = $health.Content | ConvertFrom-Json
        if ($json.status -eq 'ready' -and $json.migrations -eq 'current') {
            $ready = $true
            break
        }
    } catch {
        # Not up yet -- keep polling until the deadline.
    }
}
if (-not $ready) { Abort "Backend did not report healthy+current-migrations within 60s of restart via $BackendHealthUrl." }
if ($json.database_latest_migration -ne $manifest.schema_migration_version) {
    Abort "Schema migration mismatch: manifest expects $($manifest.schema_migration_version), live database reports $($json.database_latest_migration). The dump may predate a migration added since -- do NOT benchmark against a schema the dump doesn't match; rebuild the canonical baseline first."
}
Write-Host "[8/8] Backend healthy, migrations current ($($json.database_latest_migration))."

Write-Host ''
Write-Host 'CANONICAL_BASELINE_RESTORE_VERIFIED' -ForegroundColor Green
exit 0
