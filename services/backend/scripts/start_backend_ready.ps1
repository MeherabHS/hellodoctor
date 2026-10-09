<#
  The one canonical "start the dev backend, and don't call it ready until it
  actually is" command (B-01, 2026-10-07 four-agent audit). Replaces ad-hoc
  `cargo run &`/background-job patterns that report success the moment the
  process launches, before migrations have applied or the port is bound -
  exactly the gap that let a stale-schema backend silently serve traffic.

  Only ever prints the literal line HELLODOCTOR_BACKEND_READY on genuine
  success (port bound AND verify_backend_schema.ps1 passes). Does not install
  Postgres, Redis, MinIO, or any other host software - it only checks that
  the existing Docker containers are up and reports clearly if they are not.

  Usage: ./start_backend_ready.ps1 [-Port 8080] [-TimeoutSeconds 90]
#>
param(
    [int]$Port = 8080,
    [int]$TimeoutSeconds = 90
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BackendDir = Split-Path -Parent $ScriptDir
$BaseUrl = "http://127.0.0.1:$Port"

function Fail($message) {
    Write-Host $message -ForegroundColor Red
    exit 1
}

# ---- 1. Required dependencies must already be running - never start/install them here ----
Write-Host "Checking required containers..." -ForegroundColor Cyan
$containers = @("hellodoctor-postgres", "hellodoctor-redis")
foreach ($name in $containers) {
    $status = (docker ps --filter "name=^/${name}$" --format "{{.Status}}" 2>$null)
    if (-not $status) {
        Fail "Required container '$name' is not running. This script does not start/install Docker, Postgres, or Redis - start them yourself (e.g. 'docker compose up -d' in services\backend) and re-run."
    }
    if ($status -match "unhealthy") {
        Fail "Container '$name' is running but reports unhealthy ($status). Investigate before continuing - this script will not proceed against a known-bad dependency."
    }
    Write-Host "  $name - $status" -ForegroundColor Green
}

# ---- 2. Stop any existing backend process (known file-lock hazard: a stale ----
#         hellodoctor-backend.exe blocks cargo build/run from replacing it)
$existing = Get-Process -Name "hellodoctor-backend" -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Stopping existing hellodoctor-backend process(es): $($existing.Id -join ', ')" -ForegroundColor Yellow
    $existing | Stop-Process -Force
    Start-Sleep -Seconds 1
}

# ---- 3. Start the backend as a background process this script can poll ----
$logDir = Join-Path $BackendDir "scripts"
$stdout = Join-Path $logDir "start_backend_ready.out.log"
$stderr = Join-Path $logDir "start_backend_ready.err.log"
Write-Host "Starting backend (cargo run) - this recompiles if source changed, which can take up to a minute..." -ForegroundColor Cyan
$process = Start-Process -FilePath "cargo" -ArgumentList "run" -WorkingDirectory $BackendDir `
    -RedirectStandardOutput $stdout -RedirectStandardError $stderr -WindowStyle Hidden -PassThru

# ---- 4. Poll /healthz until it returns HTTP 200, or time out ----
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$ready = $false
while ((Get-Date) -lt $deadline) {
    if ($process.HasExited) {
        Fail "Backend process exited early (code $($process.ExitCode)) before becoming ready. See $stderr for details."
    }
    try {
        $resp = Invoke-WebRequest -Uri "$BaseUrl/healthz" -UseBasicParsing -TimeoutSec 5 -ErrorAction Stop
        if ($resp.StatusCode -eq 200) { $ready = $true; break }
    }
    catch {
        # Not up yet (connection refused) or returned a non-2xx (e.g. 503
        # not-ready) - either way, keep polling until the timeout.
    }
    Start-Sleep -Seconds 2
}

if (-not $ready) {
    Fail "Timed out after ${TimeoutSeconds}s waiting for $BaseUrl/healthz to return 200. Check $stdout / $stderr."
}
Write-Host "Backend is listening and reports ready." -ForegroundColor Green

# ---- 5. Require the three-way schema verification to pass too ----
$verifyScript = Join-Path $ScriptDir "verify_backend_schema.ps1"
& $verifyScript -BaseUrl $BaseUrl
if ($LASTEXITCODE -ne 0) {
    Fail "Backend is up but schema verification failed (see output above) - not reporting ready."
}

# ---- 6. Only now is it safe to say so ----
Write-Host ""
Write-Host "HELLODOCTOR_BACKEND_READY"
exit 0
