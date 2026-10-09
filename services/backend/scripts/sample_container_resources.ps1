<#
  Section 9 instrumentation: samples docker stats for the named benchmark
  containers every $IntervalSecs seconds for $DurationSecs seconds and
  writes one CSV row per sample per container. Read-only — never mutates
  anything. Run alongside (not instead of) a load-test invocation.

  Usage:
    ./sample_container_resources.ps1 -Prefix hellodoctor-bench-a -DurationSecs 120 -OutFile ../../docs/audit_evidence/pre_patient_benchmark_4x8/profileA_20rps.csv
#>
param(
    [string]$Prefix = "hellodoctor-bench-a",
    [int]$DurationSecs = 120,
    [int]$IntervalSecs = 5,
    [string]$OutFile = "container_samples.csv"
)

$containers = docker ps --format "{{.Names}}" | Where-Object { $_ -like "$Prefix*" }
if (-not $containers) {
    Write-Host "No running containers matched prefix '$Prefix'." -ForegroundColor Red
    exit 1
}

"timestamp_utc,container,cpu_percent,mem_usage,mem_limit,mem_percent,net_io,block_io,pids" | Out-File -FilePath $OutFile -Encoding utf8
$elapsed = 0
while ($elapsed -lt $DurationSecs) {
    $ts = (Get-Date).ToUniversalTime().ToString("o")
    foreach ($c in $containers) {
        $raw = docker stats $c --no-stream --format "{{.CPUPerc}},{{.MemUsage}},{{.MemPerc}},{{.NetIO}},{{.BlockIO}},{{.PIDs}}"
        if ($raw) {
            $parts = $raw -split ","
            $memUsage = ($parts[1] -split " / ")[0]
            $memLimit = ($parts[1] -split " / ")[1]
            "$ts,$c,$($parts[0]),$memUsage,$memLimit,$($parts[2]),$($parts[3]),$($parts[4]),$($parts[5])" | Add-Content -Path $OutFile
        }
    }
    Start-Sleep -Seconds $IntervalSecs
    $elapsed += $IntervalSecs
}
Write-Host "Wrote samples to $OutFile"
