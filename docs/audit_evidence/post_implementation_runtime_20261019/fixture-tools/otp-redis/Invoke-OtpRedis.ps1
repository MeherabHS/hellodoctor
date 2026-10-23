$ErrorActionPreference = 'Stop'
$otpHarnessDirectory = [IO.Path]::GetFullPath($PSScriptRoot)
$otpRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$otpEvidenceDirectory = Join-Path $otpRoot 'docs/audit_evidence/post_implementation_runtime_20261019'
$otpSourcePath = Join-Path $otpRoot 'services/backend/src/rate_limit.rs'
$otpExtraction = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'extraction.json') -Raw | ConvertFrom-Json
$otpSourceHash = (Get-FileHash -LiteralPath $otpSourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
$otpLuaHash = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'reserve.lua') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($otpSourceHash -ne $otpExtraction.source_sha256 -or $otpLuaHash -ne $otpExtraction.lua_sha256) { throw 'Source or extracted Lua changed after extraction.' }
$otpRunId='otp-lua-' + (Get-Date -Format 'yyyyMMddTHHmmssfff')
$otpImageId=docker image inspect redis:7-alpine --format '{{.Id}}'
if ($LASTEXITCODE -ne 0) { throw 'Cached redis:7-alpine image unavailable.' }
$otpStarted=[DateTimeOffset]::UtcNow
$otpTimer=[Diagnostics.Stopwatch]::StartNew()
$ErrorActionPreference = 'Continue'
$otpOutput=docker run --rm --pull never --network none --ipc none --read-only --user redis `
 --label hellodoctor.acceptance.otp.fixture=true --cap-drop ALL --security-opt no-new-privileges `
 --cpus 1 --memory 128m --memory-swap 128m --pids-limit 32 `
 --ulimit nofile=128:128 --ulimit fsize=16777216:16777216 --ulimit cpu=120:120 `
 --tmpfs /scratch:rw,nosuid,nodev,noexec,size=32m,uid=999,gid=999,mode=0700 `
 --tmpfs /dev/shm:ro,nosuid,nodev,noexec,size=1m,mode=0555 `
 --tmpfs /dev/mqueue:ro,nosuid,nodev,noexec,size=1m,mode=0555 `
 --mount "type=bind,source=$otpHarnessDirectory,target=/harness,readonly" `
 --entrypoint /bin/busybox redis:7-alpine timeout -s KILL 120 /usr/bin/env -i `
 PATH=/usr/local/bin:/usr/bin:/bin HOME=/scratch TMPDIR=/scratch /bin/sh /harness/run-otp.sh 2>&1
$otpExit=$LASTEXITCODE
$otpTimer.Stop()
$ErrorActionPreference = 'Stop'
$otpOutput | ForEach-Object { $_.ToString() } | Set-Content -LiteralPath (Join-Path $otpEvidenceDirectory ($otpRunId + '.log')) -Encoding UTF8
$otpMeta=[ordered]@{
 run_id=$otpRunId
 command='powershell -NoProfile -ExecutionPolicy Bypass -File scratch/acceptance-runtime-20261019/otp-redis/Invoke-OtpRedis.ps1'
 started_utc=$otpStarted.ToString('o')
 duration_seconds=$otpTimer.Elapsed.TotalSeconds
 exit_code=$otpExit
 cached_image_id=$otpImageId
 source_sha256=$otpSourceHash
 lua_sha256=$otpLuaHash
 observer_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'run-otp.sh') -Algorithm SHA256).Hash.ToLowerInvariant()
 worker_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'worker.sh') -Algorithm SHA256).Hash.ToLowerInvariant()
 max_parallel_workers=8
 planned_reserve_calls=207
 planned_cases=5
 profile='new Redis; Unix socket only; --network none; no published port; read-only root/harness; empty allowlisted child environment;1 CPU;128MiB memory;no swap;32 PIDs;32MiB scratch;120s deadline'
 claim_scope='Production Lua Redis-scope only; not Rust service, provider delivery, per-account HTTP, dependency deadline or load-capacity acceptance'
 boundary_setup='Rolling/daily limits seeded immediately below threshold in fixture; fixture DEL cooldown models earlier window expiry. Actual 15min/day clock expiry not tested.'
}
$otpMeta | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $otpEvidenceDirectory ($otpRunId + '.json')) -Encoding UTF8
$otpOutput | ForEach-Object { $_.ToString() }
$otpMeta | ConvertTo-Json -Depth 4
exit $otpExit
