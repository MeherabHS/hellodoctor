<#
.SYNOPSIS
  Runs one performance-assurance scenario against the disposable
  hellodoctor-perf-baseline stack (deploy/single-vm/compose.perf-baseline.yml)
  and writes every artifact to docs/assurance/evidence/<Scenario>/.

  Fixed sequence, identical for every scenario:
    1. Restore the canonical 10k-user database + flush Redis + restart the
       backend (restore_canonical_benchmark_baseline.ps1, which aborts on any
       verification failure), then reset pg_stat_statements.
    2. Snapshot /metrics.
    3. Start the resource sampler (sample_full_telemetry.ps1): per-container
       CPU/memory from cgroup counters, DB pool size/idle, admission
       accept/reject deltas, Postgres active connections and lock waits, Redis.
    4. Drive load from the loadgen container (cores 4-11, never the four
       server cores). Optionally runs the Admin/Doctor web workload alongside.
    5. Snapshot /metrics, top statements, per-container peak memory and OOM
       events, row counts.

  Test-only tooling. Refuses to touch any container outside the perf stack.

.EXAMPLE
  ./Invoke-AssuranceScenario.ps1 -Scenario ramp -SamplerSecs 1000 -LoadEnv @{
      CAPACITY_RPS_STEPS = '50,100,150,200,250,300,400,500'; CAPACITY_STEP_SECS = '120' }
#>
param(
    [Parameter(Mandatory)] [string]$Scenario,
    [Parameter(Mandatory)] [hashtable]$LoadEnv,
    [Parameter(Mandatory)] [int]$SamplerSecs,
    [switch]$WithWebWorkload,
    [hashtable]$WebEnv = @{},
    [switch]$SkipRestore,
    # Login scenario: the auth harness runs from the fixed-address 'authgen'
    # service and prints a text report (no result.json).
    [string]$Service = 'loadgen',
    [string]$Binary = 'capacity_test',
    # Optional extra fixture applied after the verified restore (e.g. the
    # login scenario's email+password patients), with psql -v variables.
    [string]$SetupSql,
    [hashtable]$SetupVars = @{}
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).ProviderPath
$vm = Join-Path $repo 'deploy\single-vm'
$evidence = Join-Path $repo "docs\assurance\evidence\$Scenario"
$baselineDir = Join-Path $repo 'docs\assurance\evidence\baseline'
$prefix = 'hellodoctor-perf-baseline'
$pg = "$prefix-postgres-1"; $redis = "$prefix-redis-1"; $backend = "$prefix-backend-1"
$backendUrl = 'http://127.0.0.1:58380'
New-Item -ItemType Directory -Force $evidence | Out-Null
$compose = @('compose', '--env-file', (Join-Path $vm '.env.perf-baseline'),
    '-f', (Join-Path $vm 'compose.yml'), '-f', (Join-Path $vm 'compose.perf-baseline.yml'),
    '--profile', 'loadgen')

function Step($m) { Write-Host "[$((Get-Date).ToString('HH:mm:ss'))] $m" }
function Psql($sql) { docker exec $pg psql -U postgres -d hellodoctor -t -A -F',' -c $sql }

if (-not $SkipRestore) {
    Step 'Restoring canonical baseline'
    & (Join-Path $PSScriptRoot '..\restore_canonical_benchmark_baseline.ps1') -Force `
        -DumpPath (Join-Path $baselineDir 'canonical_baseline.dump') `
        -ManifestPath (Join-Path $baselineDir 'CANONICAL_BASELINE_MANIFEST.json') `
        -PostgresContainer $pg -RedisContainer $redis -BackendContainer $backend `
        -BackendHealthUrl "$backendUrl/healthz" *> (Join-Path $evidence 'restore.log')
    if ($LASTEXITCODE -ne 0) { throw "Baseline restore failed; see $evidence\restore.log" }
    # Azurite shares the backend's network namespace, which the restore's
    # backend restart replaces, and keeps blobs in memory: restart it and
    # recreate its two containers so every run starts with empty storage.
    Step 'Resetting Azurite blob storage'
    docker restart "$prefix-azurite-1" | Out-Null
    python -I (Join-Path $PSScriptRoot 'azurite_create_containers.py') *>> (Join-Path $evidence 'restore.log')
    if ($LASTEXITCODE -ne 0) { throw "Azurite container setup failed; see $evidence\restore.log" }
}
if ($SetupSql) {
    Step "Applying scenario fixture $(Split-Path $SetupSql -Leaf)"
    $vars = @(); foreach ($k in $SetupVars.Keys) { $vars += @('-v', "$k=$($SetupVars[$k])") }
    Get-Content $SetupSql -Raw | docker exec -i $pg psql -U postgres -d hellodoctor -v ON_ERROR_STOP=1 -q @vars *>> (Join-Path $evidence 'restore.log')
    if ($LASTEXITCODE -ne 0) { throw "Scenario fixture failed; see $evidence\restore.log" }
}
# Native tools (docker, psql) write progress to stderr; under PowerShell 5.1 that
# must not be treated as a terminating error. Exit codes are checked explicitly.
$ErrorActionPreference = 'Continue'
Psql 'CREATE EXTENSION IF NOT EXISTS pg_stat_statements;' | Out-Null
Psql 'SELECT pg_stat_statements_reset();' | Out-Null
(Invoke-WebRequest "$backendUrl/metrics" -UseBasicParsing).Content | Out-File -Encoding utf8 (Join-Path $evidence 'metrics_before.txt')

Step "Starting resource sampler for ${SamplerSecs}s"
# Start-Process does not quote array arguments on PowerShell 5.1, and this
# repository's path contains spaces, so paths are quoted explicitly.
$samplerScript = (Resolve-Path (Join-Path $PSScriptRoot '..\sample_full_telemetry.ps1')).ProviderPath
$sampler = Start-Process powershell -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput (Join-Path $evidence 'sampler.log') `
    -RedirectStandardError (Join-Path $evidence 'sampler.err.log') `
    -ArgumentList ("-NoProfile -ExecutionPolicy Bypass -File `"$samplerScript`" -Prefix $prefix " +
        "-BackendUrl $backendUrl -PgContainer $pg -PgUser postgres -PgDb hellodoctor " +
        "-RedisContainer $redis -DurationSecs $SamplerSecs -IntervalSecs 10 " +
        "-OutFile `"$(Join-Path $evidence 'telemetry.csv')`"")
Start-Sleep -Seconds 12   # let the sampler take its priming reading before load starts

$envArgs = @()
if ($Binary -eq 'capacity_test') { $envArgs += @('-e', "CAPACITY_RESULT_JSON=/evidence/$Scenario/result.json") }
foreach ($k in $LoadEnv.Keys) { $envArgs += @('-e', "$k=$($LoadEnv[$k])") }

$web = $null
if ($WithWebWorkload) {
    $webArgs = @()
    foreach ($k in $WebEnv.Keys) { $webArgs += @('-e', "$k=$($WebEnv[$k])") }
    Step 'Starting Admin/Doctor web workload'
    $web = Start-Process docker -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $evidence 'web_workload.log') `
        -RedirectStandardError (Join-Path $evidence 'web_workload.err.log') `
        -ArgumentList ((($compose + @('run', '--rm', '--no-deps') + $webArgs + @('loadgen', 'web_workload')) |
            ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }) -join ' ')
        # Start-Process does not quote array arguments on PowerShell 5.1; the
        # repository path contains spaces, so each argument is quoted here
        # (the 2026-11-17 soak's web workload failed without this).
}

Step "Driving API load: $(($LoadEnv.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ' ')"
& docker @compose run --rm --no-deps @envArgs $Service $Binary *> (Join-Path $evidence "$Binary.log")
$loadExit = $LASTEXITCODE
if ($web) { $web.WaitForExit() }

Step 'Collecting post-run evidence'
(Invoke-WebRequest "$backendUrl/metrics" -UseBasicParsing).Content | Out-File -Encoding utf8 (Join-Path $evidence 'metrics_after.txt')
Psql "SELECT calls, round(total_exec_time::numeric,1) AS total_ms, round(mean_exec_time::numeric,2) AS mean_ms, round(max_exec_time::numeric,1) AS max_ms, regexp_replace(left(query,160),'\s+',' ','g') FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 20;" |
    Out-File -Encoding utf8 (Join-Path $evidence 'top_statements.csv')
$mem = foreach ($c in (docker ps --format '{{.Names}}' | Where-Object { $_ -like "$prefix-*" -and $_ -notlike '*loadgen*' })) {
    $peak = (docker exec $c cat /sys/fs/cgroup/memory.peak 2>$null | Select-Object -Last 1)
    $max = (docker exec $c cat /sys/fs/cgroup/memory.max 2>$null | Select-Object -Last 1)
    $oom = (docker exec $c sh -c "grep '^oom_kill ' /sys/fs/cgroup/memory.events" 2>$null | Select-Object -Last 1)
    $restarts = docker inspect -f '{{.RestartCount}}' $c
    [pscustomobject]@{ container = $c; memory_peak_bytes = $peak; memory_max_bytes = $max; oom = $oom; restart_count = $restarts }
}
$mem | ConvertTo-Json | Out-File -Encoding utf8 (Join-Path $evidence 'container_memory.json')
Psql "SELECT 'appointments', count(*) FROM appointments UNION ALL SELECT 'chat_messages', count(*) FROM chat_messages UNION ALL SELECT 'notifications', count(*) FROM notifications;" |
    Out-File -Encoding utf8 (Join-Path $evidence 'row_counts_after.csv')

if (-not $sampler.HasExited) { Step 'Waiting for sampler to finish'; $sampler.WaitForExit() }
Step "Done (load generator exit code $loadExit). Evidence: $evidence"
exit $loadExit
