<#
.SYNOPSIS
  Time-aligned live instrumentation for a six-service benchmark run: per-container
  CPU/memory, PostgreSQL pg_stat_activity state/wait-event counts, connection
  occupancy by role, and longest-running query age -- sampled every 5s for the
  duration of the run and written as it goes (never buffered in memory, so a
  killed/crashed sampler still leaves partial evidence, not nothing).

  Run via Start-Process -File (never Start-Job): an earlier version of the
  docker-stats sampler in run_six_service_web_comparison.ps1 used Start-Job,
  which runs in a separate child PowerShell process that does not reliably
  inherit the same Docker CLI context (PATH/engine context) as the calling
  session -- confirmed live: every prior run's docker_stats.csv contained only
  its header line, zero actual samples, across every benchmark this whole
  investigation ran. Started as a plain OS process instead, with the exact
  same simplicity that already proved reliable for capacity_test.exe/
  web_workload.exe.

.PARAMETER DurationSecs
  How long to keep sampling (should match the paired benchmark's duration).

.PARAMETER OutDir
  Absolute directory to write docker_stats.csv / pg_wait_events.csv /
  pg_connection_summary.csv into.

.PARAMETER PostgresContainer
  Name of the Postgres container to query via `docker exec`.

.PARAMETER ProjectContainerFilter
  Substring used to select which `docker stats` rows to keep.

.PARAMETER MetricsUrl
  Backend /metrics endpoint. Milestone 8 benchmark-methodology correction
  (2026-11-04): sampled alongside everything else specifically to capture the
  per-role `db_pool_size`/`db_pool_idle` gauges as a real time series -- these
  already existed in admission.rs but nothing had ever scraped them over time
  before, only pg_connection_summary.csv's AGGREGATE (not per-role) total.
  That gap is exactly why "peak total connections was lower in the warm run"
  could not rule out one specific role's pool being saturated while others sat
  idle -- see docs/performance-evidence/milestone-8-final/remediation/
  phase7_warm_repeat_200rps/WARM_REPEAT_REPORT.md section 6/8.
#>
param(
    [Parameter(Mandatory = $true)][int]$DurationSecs,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$PostgresContainer = 'hellodoctor-m8-final-local-postgres-1',
    [string]$ProjectContainerFilter = 'hellodoctor-m8-final-local',
    [string]$MetricsUrl = 'http://127.0.0.1:58280/metrics',
    [int]$IntervalSecs = 5
)

$dockerStatsPath = Join-Path $OutDir 'docker_stats.csv'
$pgWaitPath = Join-Path $OutDir 'pg_wait_events.csv'
$pgConnPath = Join-Path $OutDir 'pg_connection_summary.csv'
$pgConnByRolePath = Join-Path $OutDir 'pg_connection_summary_by_role.csv'
$dbPoolMetricsPath = Join-Path $OutDir 'db_pool_metrics_by_role.csv'
$depPressureIntervalPath = Join-Path $OutDir 'admission_dependency_pressure_interval.csv'

"timestamp,container,cpu_perc,mem_usage" | Set-Content -Path $dockerStatsPath -Encoding utf8
"timestamp,state,wait_event_type,wait_event,count" | Set-Content -Path $pgWaitPath -Encoding utf8
"timestamp,active,idle,idle_in_transaction,total,longest_query_secs" | Set-Content -Path $pgConnPath -Encoding utf8
"timestamp,usename,active,idle,idle_in_transaction,total,longest_query_secs" | Set-Content -Path $pgConnByRolePath -Encoding utf8
"timestamp,role,pool_size,pool_idle" | Set-Content -Path $dbPoolMetricsPath -Encoding utf8
# Milestone 8 pool-capacity experiment (2026-11-04): admission_dependency_
# pressure_pool_total{class,pool} already exists (admission.rs) with true
# event-time precision -- it increments synchronously at the exact moment of
# each rejection -- but until now was only ever read once before and once
# after a whole run, giving no visibility into WHICH 5-second window a brief
# spike actually fell in. Diagnostic-only: scrapes the same already-fetched
# /metrics body this script already pulls for db_pool_size/db_pool_idle
# below, and writes the DELTA since the previous tick per class+pool, so a
# bucket like "320-330s had an usually large error spike" can be matched
# directly against which pool pressure actually fired in that exact window,
# without needing a second counter-diff sweep after the fact. Never changes
# backend behavior -- read-only scraping of an existing counter.
"timestamp,class,pool,interval_count" | Set-Content -Path $depPressureIntervalPath -Encoding utf8
$prevPressureCounts = @{}

$deadline = (Get-Date).AddSeconds($DurationSecs + 15)
while ((Get-Date) -lt $deadline) {
    $ts = (Get-Date).ToString('o')

    # docker stats' {{.Container}} field is the container ID, not its name --
    # confirmed live: every prior attempt silently matched zero rows against
    # a name-substring filter because of exactly this. {{.Name}} (singular)
    # is the actual name field.
    docker stats --no-stream --format '{{.Name}},{{.CPUPerc}},{{.MemUsage}}' 2>$null |
        Where-Object { $_ -match [regex]::Escape($ProjectContainerFilter) } |
        ForEach-Object { "$ts,$_" } |
        Add-Content -Path $dockerStatsPath -Encoding utf8

    $waitSql = "SELECT COALESCE(state,'none'),COALESCE(wait_event_type,'none'),COALESCE(wait_event,'none'),count(*) FROM pg_stat_activity WHERE pid<>pg_backend_pid() GROUP BY 1,2,3;"
    docker exec $PostgresContainer psql -U postgres -d hellodoctor -t -A -F',' -c $waitSql 2>$null |
        Where-Object { $_ -match ',' } |
        ForEach-Object { "$ts,$_" } |
        Add-Content -Path $pgWaitPath -Encoding utf8

    $connSql = "SELECT count(*) FILTER (WHERE state='active'), count(*) FILTER (WHERE state='idle'), count(*) FILTER (WHERE state='idle in transaction'), count(*), COALESCE(MAX(EXTRACT(EPOCH FROM (clock_timestamp()-query_start))),0) FROM pg_stat_activity WHERE pid<>pg_backend_pid();"
    $connLine = docker exec $PostgresContainer psql -U postgres -d hellodoctor -t -A -F',' -c $connSql 2>$null
    if ($connLine) { "$ts,$connLine" | Add-Content -Path $pgConnPath -Encoding utf8 }

    # Per-role breakdown of the exact same shape -- this is what actually
    # answers "was ONE specific capability role's pool saturated while others
    # sat idle", which the aggregate-only query above cannot.
    $connByRoleSql = "SELECT usename, count(*) FILTER (WHERE state='active'), count(*) FILTER (WHERE state='idle'), count(*) FILTER (WHERE state='idle in transaction'), count(*), COALESCE(MAX(EXTRACT(EPOCH FROM (clock_timestamp()-query_start))),0) FROM pg_stat_activity WHERE pid<>pg_backend_pid() AND datname='hellodoctor' GROUP BY usename;"
    docker exec $PostgresContainer psql -U postgres -d hellodoctor -t -A -F',' -c $connByRoleSql 2>$null |
        Where-Object { $_ -match ',' } |
        ForEach-Object { "$ts,$_" } |
        Add-Content -Path $pgConnByRolePath -Encoding utf8

    # Per-role SQLx pool gauges (db_pool_size{role}/db_pool_idle{role}) --
    # these are set on every admitted request in admission.rs, but only ever
    # reflect whichever roles that one request's cost class happened to check
    # (see the `relevant_pools` match in admission.rs); under decent request
    # volume at 5s granularity all four roles get refreshed often enough for
    # this to be a meaningful time series, not just one role's snapshot.
    try {
        $metricsText = (Invoke-WebRequest -Uri $MetricsUrl -UseBasicParsing -TimeoutSec 3).Content
        $metricsLines = $metricsText -split "`r?`n"
        foreach ($line in $metricsLines) {
            if ($line -match '^db_pool_size\{role="([^"]+)"\}\s+([0-9.eE+-]+)$') {
                $role = $Matches[1]; $size = $Matches[2]
                $idleLine = $metricsLines | Where-Object { $_ -match "^db_pool_idle\{role=`"$role`"\}\s+" }
                $idle = if ($idleLine -match '\s+([0-9.eE+-]+)$') { $Matches[1] } else { '' }
                "$ts,$role,$size,$idle" | Add-Content -Path $dbPoolMetricsPath -Encoding utf8
            }
            if ($line -match '^admission_dependency_pressure_pool_total\{class="([^"]+)",pool="([^"]+)"\}\s+([0-9.eE+-]+)$') {
                $class = $Matches[1]; $pool = $Matches[2]; $cumulative = [double]$Matches[3]
                $key = "$class|$pool"
                $previous = if ($prevPressureCounts.ContainsKey($key)) { $prevPressureCounts[$key] } else { 0 }
                $delta = $cumulative - $previous
                $prevPressureCounts[$key] = $cumulative
                if ($delta -gt 0) {
                    "$ts,$class,$pool,$delta" | Add-Content -Path $depPressureIntervalPath -Encoding utf8
                }
            }
        }
    } catch {
        # Backend unreachable this tick -- skip, do not abort the whole sampler
        # over one missed scrape (matches this script's existing resilience
        # pattern for the docker/pg samples above, which also use 2>$null).
    }

    Start-Sleep -Seconds $IntervalSecs
}
