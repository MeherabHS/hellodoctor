<#
.SYNOPSIS
  Silent, read-only causal-attribution watcher for a six-service Milestone 8
  benchmark run. Complements sample_live_instrumentation.ps1 (which captures
  aggregate CPU/mem/wait-event COUNTS) with QUERY-LEVEL evidence: which exact
  statement is running during a spike, and which statements accumulate the
  most execution time / disk-spill / cache-miss cost over the run.

  Never writes to the database except a one-time `pg_stat_statements_reset()`
  at the start of the window (so this run's statement costs aren't mixed with
  a previous run's), and never touches application state, Redis, or any
  table the backend owns. Every other operation is a plain SELECT against
  Postgres system catalogs/extension views. Safe to run against the
  disposable hellodoctor-m8-final-local stack; never point this at a shared
  or production database.

.PARAMETER DurationSecs
  How long to keep sampling (match the paired benchmark's -DurationSecs).

.PARAMETER OutDir
  Absolute directory to write pg_active_queries.csv, pg_db_stats.csv, and
  periodic pg_stat_statements_snapshot_*.csv files into.

.PARAMETER PostgresContainer
  Name of the Postgres container to query via `docker exec`.

.PARAMETER DbName
  Database name inside that container.

.PARAMETER ActiveQueryIntervalSecs
  How often to sample pg_stat_activity (default 3s -- tighter than the 5s
  aggregate sampler, since a spike can be short-lived).

.PARAMETER StatementsSnapshotIntervalSecs
  How often to dump a full ranked pg_stat_statements snapshot (default 30s).

.PARAMETER ResetStatementsAtStart
  Resets pg_stat_statements before sampling begins so this run's costs are
  not mixed with a previous run's cumulative totals. Default true. Set
  false only when deliberately measuring across multiple paired runs.
#>
param(
    [Parameter(Mandatory = $true)][int]$DurationSecs,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$PostgresContainer = 'hellodoctor-m8-final-local-postgres-1',
    [string]$DbName = 'hellodoctor',
    [int]$ActiveQueryIntervalSecs = 3,
    [int]$StatementsSnapshotIntervalSecs = 30,
    [bool]$ResetStatementsAtStart = $true
)

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).ProviderPath

function Invoke-Psql([string]$Sql) {
    docker exec $PostgresContainer psql -U postgres -d $DbName -t -A -F',' -c $Sql 2>$null
}

if ($ResetStatementsAtStart) {
    Invoke-Psql "SELECT pg_stat_statements_reset();" | Out-Null
    Write-Host "pg_stat_statements reset -- this window's costs start from zero."
}

$activeQueriesPath = Join-Path $OutDir 'pg_active_queries.csv'
$dbStatsPath = Join-Path $OutDir 'pg_db_stats.csv'

"timestamp,pid,state,wait_event_type,wait_event,query_age_secs,query" | Set-Content -Path $activeQueriesPath -Encoding utf8
"timestamp,xact_commit,xact_rollback,blks_hit,blks_read,temp_files,temp_bytes,deadlocks,conflicts" | Set-Content -Path $dbStatsPath -Encoding utf8

# Query text is parameterized ($1, $2, ...) by pg_stat_activity/pg_stat_statements
# for prepared/extended-protocol statements, which this backend uses via SQLx --
# no literal patient/doctor PHI values are expected in the captured text. Still
# truncated defensively to 300 chars so no unexpectedly large literal leaks into
# evidence files wholesale.
$activeSql = @'
SELECT clock_timestamp()::text,
       pid,
       COALESCE(state,'none'),
       COALESCE(wait_event_type,'none'),
       COALESCE(wait_event,'none'),
       COALESCE(EXTRACT(EPOCH FROM (clock_timestamp()-query_start)),0),
       REPLACE(LEFT(COALESCE(query,''),300), E'\n', ' ')
FROM pg_stat_activity
WHERE pid<>pg_backend_pid() AND state<>'idle' AND query IS NOT NULL AND query<>'';
'@

$dbStatsSql = @"
SELECT xact_commit, xact_rollback, blks_hit, blks_read, temp_files, temp_bytes, deadlocks, conflicts
FROM pg_stat_database WHERE datname='$DbName';
"@

$statementsSql = @'
SELECT queryid,
       calls,
       round(total_exec_time::numeric,2),
       round(mean_exec_time::numeric,2),
       rows,
       shared_blks_hit,
       shared_blks_read,
       shared_blks_dirtied,
       shared_blks_written,
       temp_blks_read,
       temp_blks_written,
       round(blk_read_time::numeric,2),
       round(blk_write_time::numeric,2),
       REPLACE(LEFT(query,300), E'\n', ' ')
FROM pg_stat_statements
WHERE query NOT ILIKE '%pg_stat_statements%'
ORDER BY total_exec_time DESC
LIMIT 60;
'@

$nextStatementsSnapshot = Get-Date
$snapshotIndex = 0
$deadline = (Get-Date).AddSeconds($DurationSecs + 15)

while ((Get-Date) -lt $deadline) {
    Invoke-Psql $activeSql |
        Where-Object { $_ -match ',' } |
        Add-Content -Path $activeQueriesPath -Encoding utf8

    $dbLine = Invoke-Psql $dbStatsSql
    if ($dbLine) { "$((Get-Date).ToString('o')),$dbLine" | Add-Content -Path $dbStatsPath -Encoding utf8 }

    if ((Get-Date) -ge $nextStatementsSnapshot) {
        $snapshotIndex++
        $snapPath = Join-Path $OutDir ("pg_stat_statements_snapshot_{0:D3}_{1}.csv" -f $snapshotIndex, (Get-Date -Format 'yyyyMMdd_HHmmss'))
        "queryid,calls,total_exec_time_ms,mean_exec_time_ms,rows,shared_blks_hit,shared_blks_read,shared_blks_dirtied,shared_blks_written,temp_blks_read,temp_blks_written,blk_read_time_ms,blk_write_time_ms,query" |
            Set-Content -Path $snapPath -Encoding utf8
        Invoke-Psql $statementsSql | Where-Object { $_ -match ',' } | Add-Content -Path $snapPath -Encoding utf8
        $nextStatementsSnapshot = (Get-Date).AddSeconds($StatementsSnapshotIntervalSecs)
    }

    Start-Sleep -Seconds $ActiveQueryIntervalSecs
}

Write-Host "pg_causal_watcher done: $activeQueriesPath, $dbStatsPath, $snapshotIndex pg_stat_statements snapshots in $OutDir"
