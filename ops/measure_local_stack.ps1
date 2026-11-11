$ErrorActionPreference = 'Stop'
$OutDir = Join-Path $PSScriptRoot '..\docs\performance-evidence\milestone-8-workstream-4'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$compose = @('-f','deploy/single-vm/compose.yml','-f','deploy/single-vm/compose.local-verification.yml','--env-file','deploy/single-vm/local-verification.env')
$names = @('hellodoctor-m8-final-local-postgres-1','hellodoctor-m8-final-local-redis-1','hellodoctor-m8-final-local-backend-1','hellodoctor-m8-final-local-admin-web-1','hellodoctor-m8-final-local-doctor-web-1','hellodoctor-m8-final-local-proxy-1')
$rows = [System.Collections.Generic.List[object]]::new()
for ($i = 0; $i -lt 8; $i++) {
    $timestamp = [DateTime]::UtcNow.ToString('o')
    $stats = docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.MemPerc}}' 2>$null
    $snapshot = [ordered]@{timestamp_utc=$timestamp; services=@{}}
    foreach ($line in $stats) {
        $p = $line -split '\|', 4
        if ($names -contains $p[0]) { $snapshot.services[$p[0]] = [ordered]@{cpu=$p[1]; memory=$p[2]; memory_pct=$p[3]} }
    }
    $snapshot | ConvertTo-Json -Compress | Add-Content (Join-Path $OutDir 'local-stack-resource-samples.ndjson')
    $rows.Add($snapshot)
    Start-Sleep -Seconds 2
}

function Measure-Endpoint([string]$url, [hashtable]$headers) {
    $samples = @()
    1..30 | ForEach-Object {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        try { $r = Invoke-WebRequest -Uri $url -Headers $headers -UseBasicParsing -TimeoutSec 10; $statusCode = [int]$r.StatusCode } catch { $statusCode = 0 }
        $sw.Stop()
        $samples += [pscustomobject]@{ms=[math]::Round($sw.Elapsed.TotalMilliseconds,2);statusCode=$statusCode}
    }
    $ordered = @($samples | ForEach-Object { $_.ms } | Sort-Object)
    return [ordered]@{count=$ordered.Count;p50=$ordered[[math]::Floor(($ordered.Count-1)*0.50)];p95=$ordered[[math]::Floor(($ordered.Count-1)*0.95)];p99=$ordered[[math]::Floor(($ordered.Count-1)*0.99)];max=$ordered[-1];statuses=@($samples | Group-Object statusCode | ForEach-Object { @{status=$_.Name;count=$_.Count} })}
}

$result = [ordered]@{
    measured_at_utc = [DateTime]::UtcNow.ToString('o')
    topology = 'six-service disposable Compose stack; cpuset 0-3; service memory limits from compose.local-verification.yml'
    health = (Invoke-WebRequest -Uri 'http://127.0.0.1:58080/healthz' -Headers @{Host='api.localhost'} -UseBasicParsing).Content | ConvertFrom-Json
    endpoints = [ordered]@{
        backend_health = Measure-Endpoint 'http://127.0.0.1:58080/healthz' @{Host='api.localhost'}
        admin_login_page = Measure-Endpoint 'http://127.0.0.1:58080/login' @{Host='admin.localhost'}
        doctor_root = Measure-Endpoint 'http://127.0.0.1:58080/' @{Host='doctor.localhost'}
    }
    restart_counts = @{}
}
foreach ($name in $names) {
    $inspect = docker inspect $name | ConvertFrom-Json
    $result.restart_counts[$name] = $inspect[0].RestartCount
}
$result | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $OutDir 'local-stack-focused-latency.json')
$result | ConvertTo-Json -Depth 12
