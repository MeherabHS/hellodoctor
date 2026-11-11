<#
  Milestone 8.0 full-telemetry sampler (2026-10-31). Captures, every
  IntervalSecs for DurationSecs, in one combined CSV correlated by
  timestamp: per-container CPU/memory (same cgroup-counter-delta method as
  sample_container_resources.ps1 -- not duplicated as a library here only
  because that script's own top-level body runs immediately on dot-source;
  the handful of helper functions are small enough to keep self-contained),
  the backend's own DB connection-pool gauges and admission accept/reject
  counters (scraped from /metrics, Prometheus counters are cumulative since
  process start so this script also prints each tick's DELTA against the
  previous tick, which is what you actually want for "did this happen during
  the burst or steady-state" questions), PostgreSQL's own active-connection
  and lock-wait counts (pg_stat_activity / pg_locks), and Redis's own
  ops/sec and client count (INFO). Never mutates anything: every exec is a
  read-only query, cat, or INFO/GET command.

  Usage (run from services/backend/scripts, stack already up):
    ./sample_full_telemetry.ps1 -Prefix hellodoctor-confa -BackendUrl http://localhost:8280 `
        -PgContainer hellodoctor-confa-postgres -RedisContainer hellodoctor-confa-redis `
        -DurationSecs 300 -IntervalSecs 10 `
        -OutFile ../../../docs/performance-evidence/milestone-8-0/configA/full_telemetry.csv
#>
param(
    [string]$Prefix = "hellodoctor-confa",
    [string]$BackendUrl = "http://localhost:8280",
    [string]$PgContainer = "hellodoctor-confa-postgres",
    [string]$PgUser = "hellodoctor",
    [string]$PgDb = "hellodoctor",
    [string]$RedisContainer = "hellodoctor-confa-redis",
    [int]$DurationSecs = 300,
    [int]$IntervalSecs = 10,
    [string]$OutFile = "full_telemetry.csv"
)

function Get-CgroupVersion {
    param([string]$Container)
    $probe = docker exec $Container sh -c "if [ -f /sys/fs/cgroup/cpu.stat ]; then echo v2; elif [ -f /sys/fs/cgroup/cpuacct/cpuacct.usage ]; then echo v1; else echo none; fi" 2>$null
    return ($probe | Select-Object -Last 1).Trim()
}

function Get-CpuCount {
    param([string]$Container, [string]$CgroupVersion)
    if ($CgroupVersion -eq "v2") {
        $raw = (docker exec $Container cat /sys/fs/cgroup/cpu.max 2>$null | Select-Object -Last 1).Trim()
        if ($raw -and $raw -notlike "max *") {
            $parts = $raw -split " "
            if ([double]$parts[1] -gt 0) { return [double]([double]$parts[0] / [double]$parts[1]) }
        }
    }
    $nproc = (docker exec $Container nproc 2>$null | Select-Object -Last 1).Trim()
    if ($nproc -match '^\d+$') { return [double]$nproc }
    return 1.0
}

function Get-CpuUsageUsec {
    param([string]$Container, [string]$CgroupVersion)
    if ($CgroupVersion -eq "v2") {
        $line = docker exec $Container sh -c "grep '^usage_usec' /sys/fs/cgroup/cpu.stat" 2>$null | Select-Object -Last 1
        if ($line -match '^usage_usec\s+(\d+)') { return [double]$Matches[1] }
    } elseif ($CgroupVersion -eq "v1") {
        $raw = (docker exec $Container cat /sys/fs/cgroup/cpuacct/cpuacct.usage 2>$null | Select-Object -Last 1).Trim()
        if ($raw -match '^\d+$') { return [double]$raw / 1000.0 }
    }
    return $null
}

function Get-MemCurrent {
    param([string]$Container, [string]$CgroupVersion)
    if ($CgroupVersion -eq "v2") {
        $used = (docker exec $Container cat /sys/fs/cgroup/memory.current 2>$null | Select-Object -Last 1).Trim()
    } else {
        $used = (docker exec $Container cat /sys/fs/cgroup/memory/memory.usage_in_bytes 2>$null | Select-Object -Last 1).Trim()
    }
    if ($used -match '^\d+$') { return [double]$used }
    return $null
}

function Get-MetricValue {
    param([string[]]$MetricsLines, [string]$Pattern)
    $line = $MetricsLines | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if ($line -and $line -match '([\d.]+)\s*$') { return [double]$Matches[1] }
    return 0.0
}

$containers = docker ps --format "{{.Names}}" | Where-Object { $_ -like "$Prefix*" }
if (-not $containers) {
    Write-Host "No running containers matched prefix '$Prefix'." -ForegroundColor Red
    exit 1
}
$cgroupVersion = @{}
$cpuCount = @{}
foreach ($c in $containers) {
    $v = Get-CgroupVersion -Container $c
    $cgroupVersion[$c] = $v
    $cpuCount[$c] = Get-CpuCount -Container $c -CgroupVersion $v
}
$prevUsage = @{}
$prevTs = @{}

$header = @(
    "timestamp_utc"
    ($containers | ForEach-Object { "cpu_pct_$_" })
    ($containers | ForEach-Object { "mem_bytes_$_" })
    "db_pool_size_api", "db_pool_idle_api"
    "db_pool_size_auth", "db_pool_idle_auth"
    "db_pool_size_payment", "db_pool_idle_payment"
    "db_pool_size_finance", "db_pool_idle_finance"
    "admitted_normal_delta", "admitted_payment_delta", "admitted_webhook_delta"
    "rejected_payment_lane_concurrency_delta", "rejected_payment_shared_budget_delta", "rejected_payment_lane_rate_delta"
    "rejected_normal_lane_concurrency_delta", "rejected_normal_dependency_pressure_delta"
    "pg_active_connections", "pg_waiting_locks"
    "redis_ops_per_sec", "redis_connected_clients"
) -join ","
$header | Out-File -FilePath $OutFile -Encoding utf8

$prevAdmitted = @{}
$prevRejected = @{}

# Prime the admission-counter baseline before the loop starts, so every
# WRITTEN row is a true per-interval delta -- never the full cumulative
# total masquerading as one (which is what the very first tick would show
# without this, since Prometheus counters are cumulative since process
# start, not reset per sampler run).
$primeRaw = try { (Invoke-WebRequest -Uri "$BackendUrl/metrics" -UseBasicParsing -TimeoutSec 5).Content -split "`n" } catch { @() }
$prevAdmitted["admittedNormal"] = Get-MetricValue $primeRaw 'admission_accepted_total\{class="normal"\}'
$prevAdmitted["admittedPayment"] = Get-MetricValue $primeRaw 'admission_accepted_total\{class="payment"\}'
$prevAdmitted["admittedWebhook"] = Get-MetricValue $primeRaw 'admission_accepted_total\{class="webhook"\}'
$prevRejected["rejPaymentLaneConc"] = Get-MetricValue $primeRaw 'admission_rejected_total\{class="payment",reason="lane_concurrency"\}'
$prevRejected["rejPaymentSharedBudget"] = Get-MetricValue $primeRaw 'admission_rejected_total\{class="payment",reason="shared_class_budget"\}'
$prevRejected["rejPaymentLaneRate"] = Get-MetricValue $primeRaw 'admission_rejected_total\{class="payment",reason="lane_rate_limit"\}'
$prevRejected["rejNormalLaneConc"] = Get-MetricValue $primeRaw 'admission_rejected_total\{class="normal",reason="lane_concurrency"\}'
$prevRejected["rejNormalDepPressure"] = Get-MetricValue $primeRaw 'admission_rejected_total\{class="normal",reason="dependency_pressure"\}'

$elapsed = 0
while ($elapsed -lt $DurationSecs) {
    $now = Get-Date
    $ts = $now.ToUniversalTime().ToString("o")

    $cpuVals = @()
    $memVals = @()
    foreach ($c in $containers) {
        $ver = $cgroupVersion[$c]
        $usage = Get-CpuUsageUsec -Container $c -CgroupVersion $ver
        $mem = Get-MemCurrent -Container $c -CgroupVersion $ver
        $cpuPct = "NA"
        if ($null -ne $usage) {
            if ($prevUsage.ContainsKey($c)) {
                $deltaUsec = $usage - $prevUsage[$c]
                $deltaSecs = ($now - $prevTs[$c]).TotalSeconds
                if ($deltaSecs -gt 0) {
                    $cpuPct = [math]::Round((($deltaUsec / 1e6) / ($deltaSecs * $cpuCount[$c])) * 100.0, 2)
                }
            }
            $prevUsage[$c] = $usage
            $prevTs[$c] = $now
        }
        $cpuVals += $cpuPct
        $memStr = "NA"
        if ($null -ne $mem) { $memStr = $mem }
        $memVals += $memStr
    }

    $metricsRaw = try { (Invoke-WebRequest -Uri "$BackendUrl/metrics" -UseBasicParsing -TimeoutSec 5).Content -split "`n" } catch { @() }

    $poolApiSize = Get-MetricValue $metricsRaw 'db_pool_size\{role="api"\}'
    $poolApiIdle = Get-MetricValue $metricsRaw 'db_pool_idle\{role="api"\}'
    $poolAuthSize = Get-MetricValue $metricsRaw 'db_pool_size\{role="auth"\}'
    $poolAuthIdle = Get-MetricValue $metricsRaw 'db_pool_idle\{role="auth"\}'
    $poolPaymentSize = Get-MetricValue $metricsRaw 'db_pool_size\{role="payment"\}'
    $poolPaymentIdle = Get-MetricValue $metricsRaw 'db_pool_idle\{role="payment"\}'
    $poolFinanceSize = Get-MetricValue $metricsRaw 'db_pool_size\{role="finance"\}'
    $poolFinanceIdle = Get-MetricValue $metricsRaw 'db_pool_idle\{role="finance"\}'

    $admittedNormal = Get-MetricValue $metricsRaw 'admission_accepted_total\{class="normal"\}'
    $admittedPayment = Get-MetricValue $metricsRaw 'admission_accepted_total\{class="payment"\}'
    $admittedWebhook = Get-MetricValue $metricsRaw 'admission_accepted_total\{class="webhook"\}'
    $rejPaymentLaneConc = Get-MetricValue $metricsRaw 'admission_rejected_total\{class="payment",reason="lane_concurrency"\}'
    $rejPaymentSharedBudget = Get-MetricValue $metricsRaw 'admission_rejected_total\{class="payment",reason="shared_class_budget"\}'
    $rejPaymentLaneRate = Get-MetricValue $metricsRaw 'admission_rejected_total\{class="payment",reason="lane_rate_limit"\}'
    $rejNormalLaneConc = Get-MetricValue $metricsRaw 'admission_rejected_total\{class="normal",reason="lane_concurrency"\}'
    $rejNormalDepPressure = Get-MetricValue $metricsRaw 'admission_rejected_total\{class="normal",reason="dependency_pressure"\}'

    function DeltaOf($key, $current, [hashtable]$store) {
        $prev = if ($store.ContainsKey($key)) { $store[$key] } else { 0.0 }
        $store[$key] = $current
        return [math]::Round($current - $prev, 2)
    }
    $admittedNormalDelta = DeltaOf "admittedNormal" $admittedNormal $prevAdmitted
    $admittedPaymentDelta = DeltaOf "admittedPayment" $admittedPayment $prevAdmitted
    $admittedWebhookDelta = DeltaOf "admittedWebhook" $admittedWebhook $prevAdmitted
    $rejPaymentLaneConcDelta = DeltaOf "rejPaymentLaneConc" $rejPaymentLaneConc $prevRejected
    $rejPaymentSharedBudgetDelta = DeltaOf "rejPaymentSharedBudget" $rejPaymentSharedBudget $prevRejected
    $rejPaymentLaneRateDelta = DeltaOf "rejPaymentLaneRate" $rejPaymentLaneRate $prevRejected
    $rejNormalLaneConcDelta = DeltaOf "rejNormalLaneConc" $rejNormalLaneConc $prevRejected
    $rejNormalDepPressureDelta = DeltaOf "rejNormalDepPressure" $rejNormalDepPressure $prevRejected

    $pgActive = (docker exec $PgContainer psql -U $PgUser -d $PgDb -t -A -c "SELECT count(*) FROM pg_stat_activity WHERE state='active' AND pid <> pg_backend_pid();" 2>$null | Select-Object -Last 1)
    $pgWaiting = (docker exec $PgContainer psql -U $PgUser -d $PgDb -t -A -c "SELECT count(*) FROM pg_locks WHERE NOT granted;" 2>$null | Select-Object -Last 1)

    $redisInfo = docker exec $RedisContainer redis-cli INFO stats 2>$null
    $redisOps = ($redisInfo | Where-Object { $_ -match "instantaneous_ops_per_sec:" }) -replace "instantaneous_ops_per_sec:", "" -replace "`r", ""
    $redisClientsInfo = docker exec $RedisContainer redis-cli INFO clients 2>$null
    $redisClients = ($redisClientsInfo | Where-Object { $_ -match "connected_clients:" }) -replace "connected_clients:", "" -replace "`r", ""

    $row = @(
        $ts
        $cpuVals
        $memVals
        $poolApiSize, $poolApiIdle
        $poolAuthSize, $poolAuthIdle
        $poolPaymentSize, $poolPaymentIdle
        $poolFinanceSize, $poolFinanceIdle
        $admittedNormalDelta, $admittedPaymentDelta, $admittedWebhookDelta
        $rejPaymentLaneConcDelta, $rejPaymentSharedBudgetDelta, $rejPaymentLaneRateDelta
        $rejNormalLaneConcDelta, $rejNormalDepPressureDelta
        $(if ($pgActive) { $pgActive.Trim() } else { "NA" })
        $(if ($pgWaiting) { $pgWaiting.Trim() } else { "NA" })
        $(if ($redisOps) { $redisOps.Trim() } else { "NA" })
        $(if ($redisClients) { $redisClients.Trim() } else { "NA" })
    ) -join ","
    Add-Content -Path $OutFile -Value $row

    Start-Sleep -Seconds $IntervalSecs
    $elapsed += $IntervalSecs
}
Write-Host "Wrote full telemetry to $OutFile"
