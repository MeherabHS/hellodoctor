<#
.SYNOPSIS
  Admission-control causal-diagnosis watcher (Phase 7 Step 1). Scrapes the
  backend's own /metrics endpoint repeatedly during a benchmark run and saves
  each raw scrape as a timestamped snapshot, so admission rejection counters
  can be correlated in time against db_acquire_seconds (actual DB
  connection-acquisition latency), db_pool_idle (pool occupancy), and
  http_request_duration_seconds (per-route wall-clock latency) -- all of
  which already exist as real backend metrics; this script only adds the
  missing time-series capture (previously only a single before/after
  snapshot was taken). Read-only HTTP GET against /metrics; no database or
  application state is touched.

.PARAMETER DurationSecs
  How long to keep sampling (match the paired benchmark's -DurationSecs).

.PARAMETER OutDir
  Directory to write metrics_NNN_<timestamp>.prom snapshots into.

.PARAMETER MetricsUrl
  The backend's /metrics endpoint.

.PARAMETER IntervalSecs
  Scrape interval (default 2s -- tight enough to catch short admission
  bursts without meaningfully adding load itself).
#>
param(
    [Parameter(Mandatory = $true)][int]$DurationSecs,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$MetricsUrl = 'http://127.0.0.1:48280/metrics',
    [int]$IntervalSecs = 2
)

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).ProviderPath

$deadline = (Get-Date).AddSeconds($DurationSecs + 15)
$index = 0
while ((Get-Date) -lt $deadline) {
    $index++
    $ts = Get-Date -Format 'yyyyMMdd_HHmmss_fff'
    $snapPath = Join-Path $OutDir ("metrics_{0:D4}_{1}.prom" -f $index, $ts)
    try {
        $resp = Invoke-WebRequest -Uri $MetricsUrl -UseBasicParsing -TimeoutSec 3
        "# scraped_at=$ts" | Set-Content -Path $snapPath -Encoding utf8
        $resp.Content | Add-Content -Path $snapPath -Encoding utf8
    } catch {
        "# scraped_at=$ts SCRAPE_FAILED: $($_.Exception.Message)" | Set-Content -Path $snapPath -Encoding utf8
    }
    Start-Sleep -Seconds $IntervalSecs
}
Write-Host "metrics_timeseries_watcher done: $index snapshots in $OutDir"
