<#
.SYNOPSIS
  Milestone 8 endurance-run safety watchdog (2026-11-04). Runs ALONGSIDE the
  60-minute capacity_test.exe/web_workload.exe processes (never modifies
  them), polling resource and container-health signals that a short 6-minute
  run never had time to hit, and force-stops the benchmark processes if a
  genuinely unsafe condition is detected -- so an unattended hour doesn't
  run straight into a host-level failure the way this session's own earlier
  Docker Desktop crash (under low host memory) already did once.

  Does NOT touch admission limits, pool sizes, security controls, or the
  benchmark workload itself -- purely an external, read-only (until it
  decides to abort) monitor.

.PARAMETER DurationSecs
  Should match the benchmark's own duration (plus some margin for drain).

.PARAMETER OutDir
  Where to write watchdog_log.txt and (only if triggered) ABORT_REASON.txt.
#>
param(
    [Parameter(Mandatory = $true)][int]$DurationSecs,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$PostgresContainer = 'hellodoctor-m8-final-local-postgres-1',
    [string]$BackendContainer = 'hellodoctor-m8-final-local-backend-1',
    [string]$RedisContainer = 'hellodoctor-m8-final-local-redis-1',
    [int]$MinFreeMemoryMB = 1024,
    [int]$MinFreeDiskGB = 5,
    [int]$IntervalSecs = 15
)

$logPath = Join-Path $OutDir 'watchdog_log.txt'
$abortPath = Join-Path $OutDir 'ABORT_REASON.txt'
"timestamp,free_mem_mb,free_disk_gb,postgres_health,backend_health,redis_health" | Set-Content -Path $logPath -Encoding utf8

function Abort-Run($reason) {
    Write-Host "WATCHDOG ABORT: $reason" -ForegroundColor Red
    $reason | Set-Content -Path $abortPath -Encoding utf8
    Get-Process -Name 'capacity_test', 'web_workload' -ErrorAction SilentlyContinue |
        ForEach-Object {
            try { Stop-Process -Id $_.Id -Force -ErrorAction Stop; Write-Host "Stopped $($_.Name) (PID $($_.Id))" }
            catch { Write-Host "Could not stop PID $($_.Id): $_" }
        }
    exit 1
}

$deadline = (Get-Date).AddSeconds($DurationSecs + 30)
while ((Get-Date) -lt $deadline) {
    $ts = (Get-Date).ToString('o')

    $freeMemMB = [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1KB, 0)
    $freeDiskGB = [math]::Round((Get-PSDrive C).Free / 1GB, 1)

    $pgHealth = (docker inspect --format '{{.State.Health.Status}}' $PostgresContainer 2>$null)
    $backendHealth = (docker inspect --format '{{.State.Health.Status}}' $BackendContainer 2>$null)
    $redisHealth = (docker inspect --format '{{.State.Status}}' $RedisContainer 2>$null)

    "$ts,$freeMemMB,$freeDiskGB,$pgHealth,$backendHealth,$redisHealth" | Add-Content -Path $logPath -Encoding utf8

    if ($freeMemMB -lt $MinFreeMemoryMB) {
        Abort-Run "Free memory dropped to ${freeMemMB}MB, below the ${MinFreeMemoryMB}MB safety floor -- this exact condition caused an unplanned Docker Desktop crash earlier this session. Aborting before the host becomes unresponsive, not after."
    }
    if ($freeDiskGB -lt $MinFreeDiskGB) {
        Abort-Run "Free disk dropped to ${freeDiskGB}GB, below the ${MinFreeDiskGB}GB safety floor (WAL/log growth over a 60-minute run can be substantial) -- aborting before disk exhaustion corrupts anything."
    }
    if ($pgHealth -and $pgHealth -ne 'healthy') {
        Abort-Run "Postgres container health = '$pgHealth' (expected 'healthy') -- aborting rather than continuing to measure against a database that may not be serving correctly."
    }
    if ($backendHealth -and $backendHealth -ne 'healthy') {
        Abort-Run "Backend container health = '$backendHealth' (expected 'healthy') -- aborting rather than measuring against a backend that may have crashed or stalled."
    }
    if ($redisHealth -ne 'running') {
        Abort-Run "Redis container status = '$redisHealth' (expected 'running') -- aborting; admission's fail-closed Redis dependency means this would otherwise silently degrade every request's fairness/abuse checks for the rest of the run."
    }

    Start-Sleep -Seconds $IntervalSecs
}
Write-Host "Watchdog completed its full monitoring window with no abort triggered."
