<#
  Source / running-binary / database three-way migration check.

  Why this exists (B-01, 2026-10-07 four-agent audit): a long-running
  `cargo run` backend kept serving traffic while the repository gained new
  migration files the already-running process could never know about, and
  separately while the database it was talking to fell behind the migrations
  that process itself expected. The running process cannot detect the first
  case by itself (it can't see files that didn't exist when it started), so
  this script exists specifically to catch that gap from outside the process.
  Read-only: never kills a process, never installs anything, never mutates
  the database.

  Usage: ./verify_backend_schema.ps1 [-BaseUrl http://127.0.0.1:8080]
  Exit 0 only when source == running binary == database. Non-zero otherwise,
  with a specific reason printed (BACKEND_UNREACHABLE / BACKEND_RESTART_REQUIRED
  / DATABASE_MIGRATION_REQUIRED).
#>
param(
    [string]$BaseUrl = "http://127.0.0.1:8080"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$MigrationsDir = Join-Path (Split-Path -Parent $ScriptDir) "migrations"

# ---- 1. SOURCE_LATEST_MIGRATION: newest *.up.sql version on disk ----
$upFiles = Get-ChildItem -Path $MigrationsDir -Filter "*.up.sql" -File -ErrorAction SilentlyContinue
if (-not $upFiles -or $upFiles.Count -eq 0) {
    Write-Host "No migration files found under $MigrationsDir." -ForegroundColor Red
    exit 1
}

# Filenames are "<version>_<description>.up.sql" with a fixed-width numeric
# timestamp version. Sort numerically (as [int64]) rather than assuming a
# fixed string length, so this still sorts correctly if the version-number
# width ever changes.
$sourceLatest = (
    $upFiles | ForEach-Object {
        if ($_.Name -match '^(\d+)_') { [PSCustomObject]@{ Version = [int64]$Matches[1]; Raw = $Matches[1] } }
    } | Sort-Object Version -Descending | Select-Object -First 1
).Raw

if (-not $sourceLatest) {
    Write-Host "Could not parse a version prefix out of any file under $MigrationsDir." -ForegroundColor Red
    exit 1
}

# ---- 2. Query the running backend's readiness endpoint ----
try {
    $response = Invoke-WebRequest -Uri "$BaseUrl/healthz" -UseBasicParsing -TimeoutSec 10 -ErrorAction Stop
}
catch {
    $webResponse = $_.Exception.Response
    if ($null -ne $webResponse) {
        # A non-2xx HTTP status (e.g. 503 not-ready) still throws in Windows
        # PowerShell's Invoke-WebRequest; recover the body so we can parse it
        # like a normal response instead of treating it as unreachable.
        $stream = $webResponse.GetResponseStream()
        $reader = New-Object System.IO.StreamReader($stream)
        $body = $reader.ReadToEnd()
        $response = [PSCustomObject]@{ Content = $body; StatusCode = [int]$webResponse.StatusCode }
    }
    else {
        Write-Host "BACKEND_UNREACHABLE: could not reach $BaseUrl/healthz ($($_.Exception.Message))." -ForegroundColor Red
        Write-Host "This is a different failure mode than a migration mismatch - the backend process may not be running at all." -ForegroundColor Yellow
        exit 1
    }
}

try {
    $json = $response.Content | ConvertFrom-Json
}
catch {
    Write-Host "BACKEND_UNREACHABLE: $BaseUrl/healthz did not return valid JSON (got: $($response.Content))." -ForegroundColor Red
    exit 1
}

$binaryLatest = $json.binary_latest_migration
$databaseLatest = $json.database_latest_migration

if (-not $binaryLatest) {
    Write-Host "BACKEND_UNREACHABLE: /healthz response did not include 'binary_latest_migration'. Is this an old build of the backend that predates this check?" -ForegroundColor Red
    exit 1
}

# ---- 3. Three-way comparison ----
Write-Host "SOURCE_LATEST_MIGRATION   = $sourceLatest"
Write-Host "BINARY_LATEST_MIGRATION   = $binaryLatest"
Write-Host "DATABASE_LATEST_MIGRATION = $databaseLatest"

if ([int64]$sourceLatest -ne [int64]$binaryLatest) {
    Write-Host ""
    Write-Host "BACKEND_RESTART_REQUIRED" -ForegroundColor Red
    Write-Host "The running backend process was compiled/started before migration $sourceLatest was added to the repository (it only knows about up to $binaryLatest). It cannot discover new migration files on disk on its own - restart it (cargo run) so sqlx::migrate! picks up the new file(s)." -ForegroundColor Yellow
    exit 1
}

$databaseLatestForCompare = if ($databaseLatest) { [int64]$databaseLatest } else { -1 }
if ($json.status -ne "ready" -or [int64]$binaryLatest -ne $databaseLatestForCompare) {
    Write-Host ""
    Write-Host "DATABASE_MIGRATION_REQUIRED" -ForegroundColor Red
    Write-Host "The running backend (binary=$binaryLatest) is current with the repository, but the database it is connected to is still at '$databaseLatest'. Restart the backend against the correct database so sqlx::migrate! can apply the missing migration(s), or confirm it is pointed at the intended Postgres instance." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "SCHEMA_VERIFIED: source == binary == database == $sourceLatest" -ForegroundColor Green
exit 0
