<#
.SYNOPSIS
  Runs one short, comparable six-service Milestone 8 benchmark scenario:
  the API capacity_test generator plus (optionally) the web_workload
  Admin/Doctor generator, started within the same second, against the
  live hellodoctor-m8-final-local stack, with before/after /metrics and
  docker-stats evidence captured to a named output directory.

  This replaces the original ad hoc, un-reviewable `web_page_workload.sh`
  (never committed, never reusable) with a version-controlled, reviewable
  script -- see services/backend/examples/web_workload.rs's module doc for
  the full rationale.

.PARAMETER Scenario
  One of: idle, plausible, stress40 -- selects the web-workload worker
  counts/cadence. 'idle' runs the API workload alone (no web_workload
  process at all).

.PARAMETER DurationSecs
  Shared measurement window length for both generators. Default 360
  (120s ramp + 240s steady state at the fixed session count) -- a SHORT
  diagnostic test, never the 60-minute acceptance run.

.PARAMETER OutDir
  Where to write capacity_test.log / web_workload.log / metrics snapshots.

.EXAMPLE
  ./run_six_service_web_comparison.ps1 -Scenario plausible -DurationSecs 360 `
      -OutDir ..\..\docs\performance-evidence\milestone-8-final\remediation\phase2_plausible_web
#>
param(
    [Parameter(Mandatory = $true)][ValidateSet('idle', 'plausible', 'stress40')]
    [string]$Scenario,
    [int]$DurationSecs = 360,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$BaseUrl = 'http://127.0.0.1:58080',
    [string]$MetricsUrl = 'http://127.0.0.1:58280/metrics',
    [string]$CapacityDatabaseUrl = 'postgres://postgres:m8-local-password@127.0.0.1:55432/hellodoctor',
    [string]$WebDatabaseUrl = 'postgres://postgres:m8-local-password@127.0.0.1:55432/hellodoctor',
    [int]$CapacityActiveSessions = 3400,
    [int]$CapacityTargetRps = 250,
    # Milestone 8 pacing correction (2026-11-04): CAPACITY_TARGET_RPS has no
    # effect at all in CAPACITY_MODE=sessions (capacity_test.rs only consults
    # it in the separate top-down "rps" mode) -- the real offered rate in
    # sessions mode is governed purely by active-session count divided by mean
    # think-time. Confirmed live: with the long-standing defaults below
    # (5/20s, mean 12.5s) and 2,750 sessions, steady-state actually ran at
    # ~225 RPS against a nominal "200 RPS" scenario. These two params let a
    # caller correct the mean think-time to land the real offered rate near a
    # specific target without touching capacity_test.rs itself (both were
    # already env-var-driven, just hardcoded in this wrapper). Defaults here
    # are unchanged (5/20) so every existing invocation's behavior is
    # untouched unless a caller explicitly overrides them.
    [int]$CapacityThinkMinSecs = 5,
    [int]$CapacityThinkMaxSecs = 20,
    [string]$SecretsEnvFile,
    [string]$RedisContainer = 'hellodoctor-m8-final-local-redis-1',
    # Milestone 8 benchmark-methodology correction (2026-11-04): mandatory by
    # default. Every measured run must start from the verified canonical
    # Postgres/Redis baseline -- selective TRUNCATE/DELETE or patient-pool
    # rotation are explicitly NOT acceptable substitutes for this (product-
    # owner instruction). -SkipCanonicalRestore exists only for a quick
    # harness-mechanics smoke test against whatever state already happens to
    # be live; never pass it for a real evidence-producing run.
    [switch]$SkipCanonicalRestore,
    # Milestone 8 endurance-run investigation (2026-11-04): neither
    # $PSScriptRoot nor $PSCommandPath is reliably populated yet while a
    # PARAM BLOCK's own default-value expression is being evaluated when this
    # script is invoked via a nested `Start-Process -FilePath powershell.exe
    # -ArgumentList @(..., '-File', '<path>', ...)` -- confirmed live (both
    # relative and absolute `-File` paths), crashed this exact default before
    # a single line of the script BODY ran, before the canonical restore,
    # before either load generator started. Both are reliably populated once
    # execution reaches the body, so the real default is resolved there
    # instead (see immediately below) -- this parameter is left with no
    # default-value expression here on purpose.
    [string]$CanonicalRestoreScript,
    [string]$PostgresContainer = 'hellodoctor-m8-final-local-postgres-1',
    # Clears ONLY the two benchmark rate-limit key families that are known to
    # accumulate real state across repeated runs against this same long-lived
    # stack and silently bias results: ratelimit:booking:{user_id} (real
    # business rule, 10/hour/account) and ratelimit:grievance:{user_id} (3/24h
    # /account). Checked live: 1,102 patient accounts already sat at 4-18
    # toward the booking cap from earlier runs today before this flag existed,
    # which initially looked like a severe booking-success regression and was
    # actually pure test-fixture contamination. Never touches any other key
    # (no shared_class_budget/principal/session/OTP state, which all carry
    # short TTLs and self-expire within a run or two anyway) and only ever
    # targets this benchmark's own isolated Redis container, never a shared
    # or production instance.
    [bool]$ResetTestRateLimitState = $true
)

if (-not $CanonicalRestoreScript) {
    $CanonicalRestoreScript = Join-Path (Split-Path -Parent $PSCommandPath) 'restore_canonical_benchmark_baseline.ps1'
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
# Resolve to an absolute path ONCE: .NET APIs (StreamWriter, and anything
# inside a Start-Job, which runs in its own process with its own working
# directory) use Environment.CurrentDirectory, not PowerShell's provider-based
# $PWD -- a relative $OutDir silently resolved against the wrong directory in
# both of those places in an earlier version of this script.
$OutDir = (Resolve-Path $OutDir).ProviderPath

if ($SkipCanonicalRestore) {
    Write-Warning 'SkipCanonicalRestore passed -- running against WHATEVER state is already live. Do not treat this run as comparable evidence.'
} else {
    Write-Host '=== Mandatory canonical baseline restore (before any benchmark process starts) ==='
    & $CanonicalRestoreScript -PostgresContainer $PostgresContainer -RedisContainer $RedisContainer
    if ($LASTEXITCODE -ne 0) {
        throw "Canonical baseline restore failed or failed verification (exit $LASTEXITCODE) -- aborting before any benchmark process starts. See restore script output above for the specific reason."
    }
    Write-Host '=== Canonical baseline restore verified -- proceeding ==='
}

if ($SecretsEnvFile -and (Test-Path $SecretsEnvFile)) {
    Get-Content $SecretsEnvFile | ForEach-Object {
        if ($_ -match '^([A-Z0-9_]+)=(.*)$') { [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2]) }
    }
}
if (-not $env:JWT_ED25519_PRIVATE_KEY_B64 -or -not $env:WEBHOOK_HMAC_SECRET) {
    throw "JWT_ED25519_PRIVATE_KEY_B64 / WEBHOOK_HMAC_SECRET must be set (pass -SecretsEnvFile or export them) -- never printed by this script."
}
if ($Scenario -ne 'idle' -and (-not $env:WEB_ADMIN_EMAIL -or -not $env:WEB_ADMIN_PASSWORD -or -not $env:WEB_DOCTOR_EMAIL -or -not $env:WEB_DOCTOR_PASSWORD)) {
    throw "WEB_ADMIN_EMAIL, WEB_ADMIN_PASSWORD, WEB_DOCTOR_EMAIL, and WEB_DOCTOR_PASSWORD must be set for the real Admin/Doctor login preflight."
}

if ($ResetTestRateLimitState) {
    $resetCount = 0
    foreach ($pattern in @('ratelimit:booking:*', 'ratelimit:grievance:*')) {
        $keys = docker exec $RedisContainer redis-cli --scan --pattern $pattern
        if ($keys) {
            $keyList = $keys -split "`r?`n" | Where-Object { $_ }
            foreach ($batch in ($keyList | ForEach-Object -Begin { $i = 0; $acc = @() } -Process {
                $acc += $_; if ($acc.Count -ge 500) { , $acc; $acc = @() }
            } -End { if ($acc.Count -gt 0) { , $acc } })) {
                docker exec $RedisContainer redis-cli DEL @batch | Out-Null
                $resetCount += $batch.Count
            }
        }
    }
    Write-Host "Reset test-fixture rate-limit state: cleared $resetCount key(s) (ratelimit:booking:*, ratelimit:grievance:*) on $RedisContainer."
}

# Pre-test counters -- diffed against post-test below so each run's
# admission_rejected_total is attributable to THIS run, not cumulative history.
# Milestone 8 storage/stabilization pass (2026-11-05): now via the shared,
# retrying, completeness-checked, atomic-write Get-MetricsSnapshot (see
# lib_metrics_snapshot.ps1) instead of a bare Invoke-WebRequest -- a plain
# Out-File of a truncated scrape was confirmed to have silently produced an
# incomplete final counters file during the 60-minute endurance run (44KB vs
# ~127KB for earlier snapshots of the same growing metric set, cut off
# mid-line, no error raised). Required family is `admission_inflight` (a
# gauge touched by every single admitted request, including this script's
# own preflight call above) rather than `admission_rejected_total` --
# deliberately: a freshly-restored, perfectly healthy backend that hasn't
# rejected anything yet would not have registered that counter at all, and
# requiring it here would make a healthy pre-test state fail this check.
. (Join-Path $PSScriptRoot 'lib_metrics_snapshot.ps1')
Get-MetricsSnapshot -Uri $MetricsUrl -OutFile "$OutDir\pre_test_counters.txt" `
    -RequiredMetricFamilies @('admission_inflight') -LineFilter '^admission_'

$repoRoot = Resolve-Path "$PSScriptRoot\..\..\.."
$capacityExe = Join-Path $repoRoot 'services\backend\target\release\examples\capacity_test.exe'
$webExe = Join-Path $repoRoot 'services\backend\target\release\examples\web_workload.exe'

$sharedStartEpochMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + 4000

$capacityEnv = @{
    CAPACITY_MODE              = 'sessions'
    CAPACITY_PROFILE           = 'REALISTIC_RAMP'
    CAPACITY_TARGET_RPS        = "$CapacityTargetRps"
    CAPACITY_ACTIVE_SESSIONS   = "$CapacityActiveSessions"
    CAPACITY_RAMP_SECS         = '120'
    CAPACITY_THINK_MIN_SECS    = "$CapacityThinkMinSecs"
    CAPACITY_THINK_MAX_SECS    = "$CapacityThinkMaxSecs"
    CAPACITY_DURATION_SECS     = "$DurationSecs"
    CAPACITY_BASE_URL          = $BaseUrl
    CAPACITY_DATABASE_URL      = $CapacityDatabaseUrl
    # MANDATORY when CAPACITY_BASE_URL is a reverse proxy by IP:port rather
    # than the backend directly -- omitting this silently breaks ONLY the
    # booking -> webhook-confirm chain while most GET traffic still looks
    # fine, producing a misleadingly-normal-looking run with a real,
    # confirmed-by-hand 0% end-to-end booking success. See "Phase 2" in
    # docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md for the
    # concrete incident this comment exists to prevent recurring.
    CAPACITY_HOST_HEADER       = 'api.localhost'
}

# Fail-fast preflight for the API generator itself (mirrors web_workload.rs's
# own preflight) -- confirms the proxy actually reaches the real backend
# before spending any time on a full measurement window.
# Windows PowerShell 5.1 has no -SkipHttpErrorCheck (PS7+ only): Invoke-WebRequest
# throws on any non-2xx status, so a legitimate 401/403 (proof the real
# backend answered, not a blank Caddy fallback) must be caught explicitly
# rather than relying on the PS7 flag, which previously failed to bind,
# printed a non-terminating parameter error, and silently left $preflight
# $null -- $null.StatusCode is also $null, so the old check's "eq 0" never
# matched and this preflight was accidentally a no-op. Fixed to actually
# validate routing in both PS5.1 and PS7.
try {
    $preflightResponse = Invoke-WebRequest -Uri "$BaseUrl/api/v1/doctors" -Headers @{ Host = 'api.localhost' } -UseBasicParsing
    $preflightStatus = [int]$preflightResponse.StatusCode
    $preflightLength = $preflightResponse.RawContentLength
} catch [System.Net.WebException] {
    $preflightStatus = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
    $preflightLength = if ($_.Exception.Response) { $_.Exception.Response.ContentLength } else { 0 }
}
if ($preflightStatus -eq 0 -or $preflightLength -eq 0) {
    throw "capacity_test preflight failed: $BaseUrl with Host=api.localhost returned status=$preflightStatus length=$preflightLength -- routing is broken, aborting before the real run."
}
Write-Host "Preflight OK: status=$preflightStatus length=$preflightLength"

$webParamsByScenario = @{
    plausible = @{ WEB_ADMIN_WORKERS = '3';  WEB_DOCTOR_WORKERS = '5';  WEB_THINK_MIN_MS = '2000'; WEB_THINK_MAX_MS = '8000' }
    stress40  = @{ WEB_ADMIN_WORKERS = '20'; WEB_DOCTOR_WORKERS = '20'; WEB_THINK_MIN_MS = '100';  WEB_THINK_MAX_MS = '500' }
}

function Start-EnvProcess($exePath, [hashtable]$envVars, [string]$logPath) {
    # Two earlier approaches both lost data under real load and are not used
    # here anymore: (1) passing a live System.Diagnostics.Process into
    # Start-Job's -ArgumentList -- jobs run out-of-process and PowerShell
    # serializes job arguments, which silently breaks on a non-serializable
    # object like an open process handle; (2) Register-ObjectEvent +
    # BeginOutputReadLine -- .NET's own async read completes fine, but the
    # PowerShell-side -Action callbacks that write the captured lines are
    # queued for the engine's idle loop and are NOT guaranteed to have all
    # run by the time WaitForExit() returns; observed live, twice, losing a
    # growing (non-deterministic) tail of capacity_test's single end-of-run
    # print burst -- the exact BUSINESS_REJECTION_RATE/TRUE_SYSTEM_ERROR_RATE/
    # booking-success lines this script exists to capture.
    # Fixed with plain OS-level file redirection via Start-Process
    # (-RedirectStandardOutput/-RedirectStandardError): no PowerShell-side
    # callback queue involved at all, so there is nothing to race or lose.
    # Start-Process has no -Environment parameter in Windows PowerShell 5.1,
    # so the caller sets each var as a Process-scoped environment variable
    # (env:) immediately before calling this function; the child inherits
    # the parent's current environment at launch, which is why
    # CAPACITY_*/WEB_* must never collide (they don't -- different prefixes).
    foreach ($kv in $envVars.GetEnumerator()) { [System.Environment]::SetEnvironmentVariable($kv.Key, $kv.Value, 'Process') }
    $errPath = "$logPath.stderr"
    $proc = Start-Process -FilePath $exePath -NoNewWindow -PassThru `
        -RedirectStandardOutput $logPath -RedirectStandardError $errPath
    # Known Start-Process -PassThru gotcha: touching .Handle immediately forces
    # .NET to open a real handle to the process now, while it's still running.
    # Without this, a short-lived child can exit and be reaped by Windows
    # before anything queries it, leaving .ExitCode permanently $null even
    # after WaitForExit() returns -- confirmed live: a genuinely RUN_VALID
    # web_workload run was reported as "exited non-zero" purely because
    # $null -ne 0 is $true in PowerShell, not because of any real failure.
    $proc.Handle | Out-Null
    Write-Host "Started $([System.IO.Path]::GetFileName($exePath)) (PID $($proc.Id)) -> $logPath"
    return $proc
}

Write-Host "Scenario=$Scenario duration=${DurationSecs}s sharedStartEpochMs=$sharedStartEpochMs"

# Start synchronized monitoring before either workload process. The sampler
# remains active through the complete measurement window and drain period.
$instrumentationScript = Join-Path $PSScriptRoot 'sample_live_instrumentation.ps1'
$instrumentationProc = Start-Process -FilePath 'powershell.exe' -NoNewWindow -PassThru `
    -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$instrumentationScript`"",
        '-DurationSecs', $DurationSecs, '-OutDir', "`"$OutDir`"",
        '-PostgresContainer', $PostgresContainer, '-MetricsUrl', $MetricsUrl
    ) `
    -RedirectStandardOutput "$OutDir\instrumentation.log" -RedirectStandardError "$OutDir\instrumentation.log.stderr"

$capProc = Start-EnvProcess $capacityExe $capacityEnv "$OutDir\capacity_test.log"
if ($Scenario -ne 'idle') {
    $webEnv = $webParamsByScenario[$Scenario].Clone()
    $webEnv['WEB_DURATION_SECS'] = "$DurationSecs"
    $webEnv['WEB_BASE_URL'] = $BaseUrl
    $webEnv['WEB_DATABASE_URL'] = $WebDatabaseUrl
    $webEnv['WEB_START_EPOCH_MS'] = "$sharedStartEpochMs"
    $webEnv['WEB_ADMIN_EMAIL'] = $env:WEB_ADMIN_EMAIL
    $webEnv['WEB_ADMIN_PASSWORD'] = $env:WEB_ADMIN_PASSWORD
    $webEnv['WEB_DOCTOR_EMAIL'] = $env:WEB_DOCTOR_EMAIL
    $webEnv['WEB_DOCTOR_PASSWORD'] = $env:WEB_DOCTOR_PASSWORD
    $webProc = Start-EnvProcess $webExe $webEnv "$OutDir\web_workload.log"
}

$capProc.WaitForExit()
if ($Scenario -ne 'idle') { $webProc.WaitForExit() }
# .NET's Process.WaitForExit() does wait for async stdout/stderr reads to
# finish internally, but the Register-ObjectEvent -Action scriptblocks that
# write those bytes to our log files are POWERSHELL-side callbacks queued
# for the engine's idle loop -- they are not guaranteed to have been
# dequeued and executed yet the instant WaitForExit() returns. Observed
# live: the last ~1/3 of capacity_test's single end-of-run print burst (the
# tail of its time-series table plus the BUSINESS_REJECTION_RATE/
# TRUE_SYSTEM_ERROR_RATE/booking-success summary lines) was silently lost
# without this pause, while every line before that point was captured
# correctly. A short sleep yields to the engine's idle loop so any already-
# queued event actions actually run before this script reads the log files.
Start-Sleep -Seconds 2
# The instrumentation process paces itself off its own deadline
# (DurationSecs+15) independent of exactly when the two load generators
# exit, so briefly wait for it too rather than leaving it to finish in the
# background after this script has already moved on to reading its output.
$instrumentationProc.WaitForExit(($DurationSecs + 30) * 1000) | Out-Null

# Milestone 8 storage/stabilization pass (2026-11-05): this is the exact
# capture that was confirmed truncated (44KB vs ~127KB expected) during the
# 60-minute endurance run -- a plain Invoke-WebRequest + Out-File accepted a
# partial response with no error. Now via the same retrying,
# completeness-checked, atomic-write Get-MetricsSnapshot as the pre-test
# capture. Deliberately still only Warn (never throw) on ultimate failure
# here -- this runs after the real measurement window has already completed,
# and a post-test counters-file problem must not make an otherwise-successful
# multi-hour run look like it failed.
try {
    Get-MetricsSnapshot -Uri $MetricsUrl -OutFile "$OutDir\post_test_counters.txt" `
        -RequiredMetricFamilies @('admission_inflight') -LineFilter '^admission_'
} catch {
    Write-Warning "Post-test metrics snapshot failed after retries: $_ -- post_test_counters.txt was NOT written/overwritten; do not trust any file that may already exist there as this run's own post-test data."
}

Write-Host "Done. Evidence in $OutDir (capacity exit=$($capProc.ExitCode)$(if ($Scenario -ne 'idle') { ", web exit=$($webProc.ExitCode)" }))"
if ($Scenario -ne 'idle' -and $webProc.ExitCode -ne 0) {
    Write-Warning "web_workload exited non-zero -- check web_workload.log for RUN_INVALID before trusting this scenario's evidence."
}
