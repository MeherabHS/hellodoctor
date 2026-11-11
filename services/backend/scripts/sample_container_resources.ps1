<#
  Section 9 instrumentation: samples CPU/memory for the named benchmark
  containers every $IntervalSecs seconds for $DurationSecs seconds and
  writes one CSV row per sample per container. Read-only -- never mutates
  anything (never execs a mutating command, only cats cgroup pseudo-files
  and runs nproc/docker inspect, none of which change container state). Run
  alongside (not instead of) a load-test invocation.

  Milestone 8.0 remediation (2026-10-31): the previous version of this script
  called "docker stats CONTAINER --no-stream" once per container per tick
  and returned implausible near-zero CPU percent for all three
  Configuration-A containers throughout an actively-served ~250 RPS sweep
  (see docs/MILESTONE_8_0R_ADMISSION_CONTROL_REMEDIATION.md, section 7's
  disclosed limitation). Root cause: each --no-stream invocation is a fresh
  one-shot call to the Docker Engine's stats API; under load (frequent
  invocations, host contention) the engine's own internal "current" and
  "pre" cgroup reads can land close enough in time that the delta is ~0,
  which depending on the Engine/CLI version either prints 0.00% or an
  unstable value -- a known class of issue with one-shot docker stats, not a
  problem with the containers' real CPU usage.

  Fix: never ask docker stats to compute a delta for us. Instead, read the
  cumulative CPU-usage cgroup counter directly off each container (docker
  exec CONTAINER cat /sys/fs/cgroup/cpu.stat's usage_usec field, cgroup v2 --
  falling back to the cgroup v1 path cpuacct.usage in nanoseconds if v2 is
  absent) at every sampling tick, and compute CPU percent OURSELVES from the
  delta between THIS tick's cumulative counter and the PREVIOUS tick's,
  divided by the wall-clock time actually elapsed between the two ticks and
  by the container's own visible CPU count. This is the same mechanism
  docker stats / cadvisor / cgroup-aware monitoring tools use internally --
  reading it directly removes the one-shot timing failure mode entirely,
  since our two samples are seconds apart (one full IntervalSecs), never
  milliseconds apart. Memory is read the same direct way (memory.current /
  memory.max), which was never delta-based and was not actually unreliable
  before, but is now sourced identically for consistency so this script has
  no remaining dependency on docker stats output parsing at all.

  A container's visible CPU count is read once at start from cpu.max (v2
  quota/period, when a hard CPU quota is configured) or, when no quota is
  set (Configuration A uses cpuset: pinning, not --cpus quotas), from nproc
  inside the container, which correctly reflects cpuset.cpus affinity
  (verified: 4 inside the confA backend/redis containers pinned to cpuset
  "0-3", 2 inside the confA postgres container pinned to "4-5").

  Usage (Configuration A, already-running stack; run from services/backend/scripts):
    ./sample_container_resources.ps1 -Prefix hellodoctor-confa -DurationSecs 240 `
        -OutFile ../../../docs/performance-evidence/milestone-8-0/configA/resource_samples.csv

  Legacy usage (unchanged):
    ./sample_container_resources.ps1 -Prefix hellodoctor-bench-a -DurationSecs 120 -OutFile ../../docs/audit_evidence/pre_patient_benchmark_4x8/profileA_20rps.csv
#>
param(
    [string]$Prefix = "hellodoctor-bench-a",
    [int]$DurationSecs = 120,
    [int]$IntervalSecs = 5,
    [string]$OutFile = "container_samples.csv"
)

function Get-ContainerCgroupVersion {
    param([string]$Container)
    $probe = docker exec $Container sh -c "if [ -f /sys/fs/cgroup/cpu.stat ]; then echo v2; elif [ -f /sys/fs/cgroup/cpuacct/cpuacct.usage ]; then echo v1; else echo none; fi" 2>$null
    return ($probe | Select-Object -Last 1).Trim()
}

function Get-ContainerCpuCount {
    param([string]$Container, [string]$CgroupVersion)
    if ($CgroupVersion -eq "v2") {
        $raw = (docker exec $Container cat /sys/fs/cgroup/cpu.max 2>$null | Select-Object -Last 1).Trim()
        if ($raw -and $raw -notlike "max *") {
            $parts = $raw -split " "
            $quota = [double]$parts[0]
            $period = [double]$parts[1]
            if ($period -gt 0) { return [double]($quota / $period) }
        }
    }
    $nproc = (docker exec $Container nproc 2>$null | Select-Object -Last 1).Trim()
    if ($nproc -match '^\d+$') { return [double]$nproc }
    return 1.0
}

# Cumulative CPU time actually consumed, in microseconds, since container
# start. Never an instantaneous/already-averaged percentage -- the whole
# point is that WE take the delta, across a known, multi-second wall-clock
# gap, not Docker's own single-shot one.
function Get-ContainerCpuUsageUsec {
    param([string]$Container, [string]$CgroupVersion)
    if ($CgroupVersion -eq "v2") {
        $line = docker exec $Container sh -c "grep '^usage_usec' /sys/fs/cgroup/cpu.stat" 2>$null | Select-Object -Last 1
        if ($line -match '^usage_usec\s+(\d+)') { return [double]$Matches[1] }
        return $null
    } elseif ($CgroupVersion -eq "v1") {
        $raw = (docker exec $Container cat /sys/fs/cgroup/cpuacct/cpuacct.usage 2>$null | Select-Object -Last 1).Trim()
        if ($raw -match '^\d+$') { return [double]$raw / 1000.0 } # ns -> us
        return $null
    }
    return $null
}

function Get-ContainerMemory {
    param([string]$Container, [string]$CgroupVersion)
    if ($CgroupVersion -eq "v2") {
        $used = (docker exec $Container cat /sys/fs/cgroup/memory.current 2>$null | Select-Object -Last 1).Trim()
        $limitRaw = (docker exec $Container cat /sys/fs/cgroup/memory.max 2>$null | Select-Object -Last 1).Trim()
    } else {
        $used = (docker exec $Container cat /sys/fs/cgroup/memory/memory.usage_in_bytes 2>$null | Select-Object -Last 1).Trim()
        $limitRaw = (docker exec $Container cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>$null | Select-Object -Last 1).Trim()
    }
    $usedBytes = $null
    if ($used -match '^\d+$') { $usedBytes = [double]$used }
    $limitBytes = $null
    # cgroup v1 reports an effectively-unlimited cap as a huge sentinel
    # (commonly 2^63-1 rounded down to the page size); cgroup v2 reports the
    # literal string "max". Both mean "no container-specific ceiling set",
    # not "unlimited RAM" -- reported as such rather than a misleading number.
    if ($limitRaw -match '^\d+$' -and [double]$limitRaw -lt 1e15) { $limitBytes = [double]$limitRaw }
    return @{ UsedBytes = $usedBytes; LimitBytes = $limitBytes }
}

$containers = docker ps --format "{{.Names}}" | Where-Object { $_ -like "$Prefix*" }
if (-not $containers) {
    Write-Host "No running containers matched prefix '$Prefix'." -ForegroundColor Red
    exit 1
}

Write-Host "Resolving cgroup version and CPU count for: $($containers -join ', ')"
$cgroupVersion = @{}
$cpuCount = @{}
foreach ($c in $containers) {
    $v = Get-ContainerCgroupVersion -Container $c
    $cgroupVersion[$c] = $v
    if ($v -eq "none") {
        Write-Host "WARNING: $c exposes neither cgroup v2 cpu.stat nor v1 cpuacct.usage -- CPU for this container will be NOT_AVAILABLE." -ForegroundColor Yellow
        $cpuCount[$c] = 1.0
    } else {
        $cpuCount[$c] = Get-ContainerCpuCount -Container $c -CgroupVersion $v
        Write-Host "  ${c}: cgroup=$v visible_cpus=$($cpuCount[$c])"
    }
}

"timestamp_utc,container,cpu_percent,cpu_count,mem_used_bytes,mem_limit_bytes,mem_percent" |
    Out-File -FilePath $OutFile -Encoding utf8

# Previous cumulative-usage reading and its exact wall-clock timestamp, per
# container, so CPU% is always computed from a real multi-second delta, not
# from a single Docker-internal snapshot.
$prevUsageUsec = @{}
$prevTimestamp = @{}
$cpuSum = @{}
$cpuSamples = @{}
foreach ($c in $containers) { $cpuSum[$c] = 0.0; $cpuSamples[$c] = 0 }

$elapsed = 0
while ($elapsed -lt $DurationSecs) {
    $now = Get-Date
    $ts = $now.ToUniversalTime().ToString("o")
    foreach ($c in $containers) {
        $ver = $cgroupVersion[$c]
        $usageUsec = Get-ContainerCpuUsageUsec -Container $c -CgroupVersion $ver
        $mem = Get-ContainerMemory -Container $c -CgroupVersion $ver

        $cpuPercentStr = "NOT_AVAILABLE_FIRST_SAMPLE"
        if ($null -ne $usageUsec) {
            if ($prevUsageUsec.ContainsKey($c)) {
                $deltaUsec = $usageUsec - $prevUsageUsec[$c]
                $deltaWallSecs = ($now - $prevTimestamp[$c]).TotalSeconds
                if ($deltaWallSecs -gt 0) {
                    $cpuPercent = ($deltaUsec / 1e6) / ($deltaWallSecs * $cpuCount[$c]) * 100.0
                    $cpuPercentStr = [math]::Round($cpuPercent, 2)
                    $cpuSum[$c] += $cpuPercent
                    $cpuSamples[$c] += 1
                }
            }
            $prevUsageUsec[$c] = $usageUsec
            $prevTimestamp[$c] = $now
        } else {
            $cpuPercentStr = "NOT_AVAILABLE"
        }

        $memUsedStr = if ($null -ne $mem.UsedBytes) { $mem.UsedBytes } else { "NOT_AVAILABLE" }
        $memLimitStr = if ($null -ne $mem.LimitBytes) { $mem.LimitBytes } else { "UNLIMITED" }
        $memPercentStr = "NOT_AVAILABLE"
        if ($null -ne $mem.UsedBytes -and $null -ne $mem.LimitBytes -and $mem.LimitBytes -gt 0) {
            $memPercentStr = [math]::Round(($mem.UsedBytes / $mem.LimitBytes) * 100.0, 2)
        }

        "$ts,$c,$cpuPercentStr,$($cpuCount[$c]),$memUsedStr,$memLimitStr,$memPercentStr" | Add-Content -Path $OutFile
    }
    Start-Sleep -Seconds $IntervalSecs
    $elapsed += $IntervalSecs
}

Write-Host "Wrote samples to $OutFile"
Write-Host "--- Per-container average CPU% (cgroup-counter-delta based, excludes the unavailable first sample) ---"
$appTierSum = 0.0; $appTierCount = 0
$dbTierSum = 0.0; $dbTierCount = 0
foreach ($c in $containers) {
    $avg = if ($cpuSamples[$c] -gt 0) { [math]::Round($cpuSum[$c] / $cpuSamples[$c], 2) } else { "NOT_AVAILABLE" }
    Write-Host "  $c : avg_cpu_percent=$avg (n=$($cpuSamples[$c]) samples, visible_cpus=$($cpuCount[$c]))"
    if ($cpuSamples[$c] -gt 0) {
        if ($c -like "*postgres*") {
            $dbTierSum += $cpuSum[$c]; $dbTierCount += $cpuSamples[$c]
        } else {
            $appTierSum += $cpuSum[$c]; $appTierCount += $cpuSamples[$c]
        }
    }
}
# Milestone 8.0's sections 3/7 requirement: application and PostgreSQL CPU recorded
# SEPARATELY, never blended into one number that could hide which tier is
# actually under pressure.
$appAvg = if ($appTierCount -gt 0) { [math]::Round($appTierSum / $appTierCount, 2) } else { "NOT_AVAILABLE" }
$dbAvg = if ($dbTierCount -gt 0) { [math]::Round($dbTierSum / $dbTierCount, 2) } else { "NOT_AVAILABLE" }
Write-Host "APPLICATION_TIER_AVG_CPU_PERCENT=$appAvg (backend+redis, $appTierCount samples)"
Write-Host "POSTGRESQL_TIER_AVG_CPU_PERCENT=$dbAvg ($dbTierCount samples)"
