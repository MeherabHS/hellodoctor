$ErrorActionPreference = 'Stop'

# Disposable local PostgreSQL physical-backup/PITR verification.
# This script never discovers or touches existing project containers/volumes.
# It creates only resources with the unique m8-final-dr prefix and removes
# those resources in finally. The resulting JSON is retained as evidence.

$Prefix = 'hellodoctor-m8-final-dr'
$Network = "$Prefix-net"
$Source = "$Prefix-source"
$Target = "$Prefix-target"
$PgVolume = "$Prefix-pgdata"
$WalVolume = "$Prefix-wal"
$BackupVolume = "$Prefix-backup"
$Evidence = Join-Path $PSScriptRoot '..\..\docs\recovery-evidence\m8-final-local-dr'
$Password = 'M8-final-disposable-postgres-password'
$Database = 'hellodoctor'
$User = 'postgres'

New-Item -ItemType Directory -Force -Path $Evidence | Out-Null

function Invoke-Docker {
    param([Parameter(Mandatory)][string[]]$Arguments)
    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $output = @(& docker @Arguments 2>&1 | ForEach-Object { $_.ToString() })
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorAction
    if ($exitCode -ne 0) {
        throw "docker $($Arguments -join ' ') failed (exit $LASTEXITCODE): $output"
    }
    return @($output)
}

function Invoke-Sql {
    param([Parameter(Mandatory)][string]$Container, [Parameter(Mandatory)][string]$Sql)
    $lines = Invoke-Docker @('exec', $Container, 'psql', '-U', $User, '-d', $Database, '-v', 'ON_ERROR_STOP=1', '-At', '-c', $Sql)
    return (($lines | ForEach-Object { $_.ToString() }) -join "`n").Trim()
}

function Wait-Postgres {
    param([Parameter(Mandatory)][string]$Container)
    for ($i = 0; $i -lt 60; $i++) {
        $probe = & docker exec $Container pg_isready -U $User -d $Database 2>&1
        if ($LASTEXITCODE -eq 0) { return }
        Start-Sleep -Seconds 1
    }
    throw "PostgreSQL did not become ready: $Container"
}

$result = [ordered]@{
    fixture = $Prefix
    generated_at_utc = [DateTime]::UtcNow.ToString('o')
    checks = [ordered]@{}
}

try {
    Invoke-Docker @('network', 'create', $Network) | Out-Null
    Invoke-Docker @('volume', 'create', $PgVolume) | Out-Null
    Invoke-Docker @('volume', 'create', $WalVolume) | Out-Null
    Invoke-Docker @('volume', 'create', $BackupVolume) | Out-Null

    Invoke-Docker @('run', '-d', '--name', $Source, '--network', $Network,
        '-e', "POSTGRES_PASSWORD=$Password", '-e', "POSTGRES_DB=$Database",
        '-v', "${PgVolume}:/var/lib/postgresql/data",
        '-v', "${WalVolume}:/wal-archive",
        'postgres:16', 'postgres', '-c', 'wal_level=replica', '-c', 'archive_mode=on',
        '-c', 'archive_timeout=5s', '-c', "archive_command=test ! -f /wal-archive/%f && cp %p /wal-archive/%f") | Out-Null
    Wait-Postgres $Source
    Invoke-Docker @('exec', $Source, 'bash', '-lc', 'chmod 0777 /wal-archive') | Out-Null

    # The official image defaults to database access rules but does not add a
    # replication rule. Add one only inside this disposable source fixture.
    $hbaCommand = "echo 'host replication postgres 0.0.0.0/0 scram-sha-256' >> `$PGDATA/pg_hba.conf; psql -U postgres -d $Database -c 'SELECT pg_reload_conf();' -At"
    Invoke-Docker @('exec', $Source, 'bash', '-lc', $hbaCommand) | Out-Null

    Invoke-Sql $Source "CREATE TABLE IF NOT EXISTS dr_probe (id integer PRIMARY KEY, marker text NOT NULL); INSERT INTO dr_probe VALUES (1, 'before-pitr') ON CONFLICT (id) DO NOTHING;" | Out-Null
    Invoke-Sql $Source "SELECT pg_switch_wal();" | Out-Null

    # Online physical base backup. pg_verifybackup validates the manifest.
    Invoke-Docker @('run', '--rm', '--network', $Network, '-e', "PGPASSWORD=$Password",
        '-v', "${BackupVolume}:/backup", 'postgres:16', 'pg_basebackup', '-h', $Source,
        '-U', $User, '-D', '/backup', '-Fp', '-Xs', '-P', '-R') | Out-Null
    Invoke-Docker @('run', '--rm', '-v', "${BackupVolume}:/backup", 'postgres:16', 'pg_verifybackup', '/backup') | Out-Null
    $result.checks.physical_backup = 'GREEN: pg_basebackup completed and pg_verifybackup accepted the manifest'

    # A named restore point makes the PITR boundary deterministic.
    Invoke-Sql $Source "SELECT pg_create_restore_point('m8_final_before_second_marker');" | Out-Null
    Invoke-Sql $Source "INSERT INTO dr_probe VALUES (2, 'after-pitr-boundary'); SELECT pg_switch_wal();" | Out-Null

    $walCount = 0
    for ($i = 0; $i -lt 30 -and $walCount -lt 1; $i++) {
        Start-Sleep -Seconds 1
        $walCount = [int](Invoke-Docker @('run', '--rm', '-v', "${WalVolume}:/wal-archive:ro", 'postgres:16', 'bash', '-lc', "find /wal-archive -maxdepth 1 -type f -name '[0-9A-F]*' | wc -l") | Select-Object -Last 1)
    }
    if ($walCount -lt 1) { throw "No archived WAL segments found" }
    $result.checks.wal_archiving = "GREEN: $walCount archived WAL segment(s) observed"

    Invoke-Docker @('stop', $Source) | Out-Null
    Invoke-Docker @('rm', $Source) | Out-Null

    # pg_basebackup -R writes recovery settings; replace the target with a
    # deterministic named restore point and the disposable archive path.
    $recovery = "restore_command = 'cp /wal-archive/%f %p'`nrecovery_target_name = 'm8_final_before_second_marker'`nrecovery_target_action = 'promote'`n"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($recovery))
    Invoke-Docker @('run', '--rm', '--user', '0', '-v', "${BackupVolume}:/backup", 'postgres:16', 'bash', '-lc',
        "rm -f /backup/recovery.signal /backup/standby.signal /backup/postgresql.auto.conf; touch /backup/recovery.signal; echo $encoded | base64 -d > /backup/postgresql.auto.conf; chown -R 999:999 /backup") | Out-Null

    Invoke-Docker @('run', '-d', '--name', $Target, '--network', $Network,
        '-p', '127.0.0.1:55438:5432', '-v', "${BackupVolume}:/var/lib/postgresql/data",
        '-v', "${WalVolume}:/wal-archive:ro", 'postgres:16') | Out-Null
    Wait-Postgres $Target
    $marker1 = Invoke-Sql $Target "SELECT marker FROM dr_probe WHERE id=1;"
    $marker2 = Invoke-Sql $Target "SELECT count(*) FROM dr_probe WHERE id=2;"
    $inRecovery = Invoke-Sql $Target "SELECT pg_is_in_recovery();"
    if ($marker1 -ne 'before-pitr' -or $marker2 -ne '0' -or $inRecovery -ne 'f') {
        throw "PITR assertions failed: marker1=$marker1 marker2=$marker2 in_recovery=$inRecovery"
    }
    $result.checks.point_in_time_recovery = 'GREEN: target promoted at named restore point; pre-boundary row present and post-boundary row absent'
    $result.checks.integrity = 'GREEN: restored table/query state and promoted PostgreSQL target verified'

    # Configuration checks are intentionally explicit: the repository has no
    # production scheduler/retention worker or Azure backup credential path.
    $result.checks.retention = 'BLOCKED: no production backup scheduler/retention policy is implemented in this repository'
    $result.checks.encryption = 'BLOCKED: backup-at-rest encryption/key custody requires the approved external backup destination'
    $result.checks.azure_storage = 'BLOCKED: Azure subscription/Blob backup credentials unavailable'
    $result | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Evidence 'local-postgres-dr.json')
    Write-Output ($result | ConvertTo-Json -Depth 8)
}
catch {
    $result.failure = $_.Exception.Message
    $result | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Evidence 'local-postgres-dr.failed.json')
    throw
}
finally {
    foreach ($container in @($Target, $Source)) {
        try { & docker rm -f $container 2>$null | Out-Null } catch { }
    }
    try { & docker volume rm $BackupVolume $WalVolume $PgVolume 2>$null | Out-Null } catch { }
    try { & docker network rm $Network 2>$null | Out-Null } catch { }
}
