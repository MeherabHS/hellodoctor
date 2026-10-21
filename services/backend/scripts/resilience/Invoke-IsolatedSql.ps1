param([switch]$MigrationsOnly)
$ErrorActionPreference = 'Stop'
$resilienceRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
$migrationRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../migrations'))
# An existing local image is mandatory: no pull, install, host credential, or
# shared service is used. The fixture has no external network or published port.
docker image inspect postgres:16 --format '{{.Id}}' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Cached postgres:16 image is required.' }
$mode = if ($MigrationsOnly) { 'migrations' } else { 'all' }
$ErrorActionPreference = 'Continue'
$fixtureDockerOutput = docker run --rm --pull never --network none --ipc none --read-only --user postgres `
  --label hellodoctor.resilience.fixture=true `
  --cap-drop ALL --security-opt no-new-privileges --cpus 2 --memory 512m --memory-swap 512m `
  --pids-limit 64 --ulimit nofile=128:128 --ulimit fsize=67108864:67108864 --ulimit cpu=180:180 `
  --tmpfs /scratch:rw,nosuid,nodev,noexec,size=512m,uid=999,gid=999,mode=0700 `
  --tmpfs /dev/shm:ro,nosuid,nodev,noexec,size=1m,mode=0555 `
  --tmpfs /dev/mqueue:ro,nosuid,nodev,noexec,size=1m,mode=0555 `
  --mount "type=bind,source=$migrationRoot,target=/migrations,readonly" `
  --mount "type=bind,source=$resilienceRoot,target=/harness,readonly" `
  --entrypoint /usr/bin/timeout postgres:16 --kill-after=5s 180s `
  /usr/bin/env -i PATH=/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/bin:/bin HOME=/scratch TMPDIR=/scratch `
  PGDATA=/scratch/pgdata PGHOST=/scratch PGUSER=postgres PGDATABASE=postgres `
  /bin/sh /harness/run-sql.sh $mode 2>&1
$fixtureDockerExit = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
$fixtureDockerOutput | ForEach-Object { $_.ToString() }
if ($fixtureDockerExit -ne 0) { throw "Isolated SQL check failed (exit $fixtureDockerExit)." }
