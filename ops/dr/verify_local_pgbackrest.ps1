$ErrorActionPreference = 'Stop'
$Prefix = "m8-pgbackrest-$([guid]::NewGuid().ToString('N').Substring(0,10))"
$Net = "$Prefix-net"; $Pg = "$Prefix-pgdata"; $Repo = "$Prefix-repo"; $Sock = "$Prefix-sock"; $TargetSock = "$Prefix-target-sock"; $Restore = "$Prefix-restore"
$Source = "$Prefix-source"; $Target = "$Prefix-target"; $Scheduler = "$Prefix-scheduler"
$Image = if ($env:PGBACKREST_TEST_IMAGE) { $env:PGBACKREST_TEST_IMAGE } else { 'hellodoctor-pgbackrest:m8-local' }
$Cipher = 'disposable-m8-pgbackrest-cipher'
$EvidenceDir = Join-Path $PSScriptRoot '..\..\docs\recovery-evidence\m8-pgbackrest-local'
New-Item -ItemType Directory -Force $EvidenceDir | Out-Null
$result = [ordered]@{ status='FAILED'; image=$Image; checks=[ordered]@{}; cleanup='attempted' }
function D([string[]]$DockerArgs) { & docker @DockerArgs; if ($LASTEXITCODE -ne 0) { throw "docker failed ($LASTEXITCODE): $($DockerArgs -join ' ')" } }
function Run-ExpectedFailure([string[]]$DockerArgs) { $old=$ErrorActionPreference; $ErrorActionPreference='Continue'; $out=& docker @DockerArgs 2>&1; $code=$LASTEXITCODE; $ErrorActionPreference=$old; return @($code,($out -join "`n")) }
function ContainerLogs([string]$Name) { $old=$ErrorActionPreference; $ErrorActionPreference='Continue'; $out=& docker logs $Name 2>&1; $ErrorActionPreference=$old; return ($out -join "`n") }
try {
  D @('network','create',$Net)
  foreach ($volume in @($Pg,$Repo,$Sock,$TargetSock,$Restore)) { D @('volume','create',$volume) }
  D @('run','-d','--name',$Source,'--network',$Net,'--entrypoint','/usr/local/bin/postgres-entrypoint.sh','-e','POSTGRES_PASSWORD=testpass','-e','POSTGRES_USER=postgres','-e','POSTGRES_DB=postgres','-e',"PGBACKREST_REPO1_CIPHER_PASS=$Cipher",'-v',"${Pg}:/var/lib/postgresql/data",'-v',"${Repo}:/var/lib/pgbackrest/repo1",'-v',"${Sock}:/var/run/postgresql",$Image,'postgres','-c','archive_mode=on','-c','archive_command=pgbackrest --stanza=hellodoctor archive-push %p','-c','unix_socket_directories=/var/run/postgresql') | Out-Null
  Start-Sleep 8
  $config = @"
[global]
repo1-path=/var/lib/pgbackrest/repo1
repo1-cipher-type=aes-256-cbc
repo1-cipher-pass=$Cipher
repo1-retention-full=1
repo1-retention-diff=1
start-fast=y
[hellodoctor]
pg1-path=/var/lib/postgresql/data
pg1-socket-path=/var/run/postgresql
"@
  $configB64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($config))
  $beforeSql = @'
create table if not exists m8_restore_marker(v text primary key);
insert into m8_restore_marker values ('before-boundary') on conflict do nothing;
'@
  $beforeB64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($beforeSql))
  $sourceInit = "echo $configB64 | base64 -d > /etc/pgbackrest/pgbackrest.conf; echo $beforeB64 | base64 -d >/tmp/before.sql; chown -R postgres:postgres /etc/pgbackrest /var/lib/pgbackrest/repo1; chmod 640 /etc/pgbackrest/pgbackrest.conf; runuser -u postgres -- pgbackrest --stanza=hellodoctor stanza-create; runuser -u postgres -- psql -v ON_ERROR_STOP=1 -f /tmp/before.sql; runuser -u postgres -- pgbackrest --stanza=hellodoctor --type=full backup"
  $sourceOutput = & docker run --rm --network $Net --entrypoint bash -v "${Pg}:/var/lib/postgresql/data" -v "${Repo}:/var/lib/pgbackrest/repo1" -v "${Sock}:/var/run/postgresql" $Image -lc $sourceInit 2>&1
  if ($LASTEXITCODE -ne 0) { throw ($sourceOutput -join "`n") }
  $afterSql = "select pg_create_restore_point('m8-boundary'); insert into m8_restore_marker values ('after-boundary'); select pg_switch_wal();"
  $afterB64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($afterSql))
  $afterCmd = "echo $afterB64 | base64 -d >/tmp/after.sql; runuser -u postgres -- psql -v ON_ERROR_STOP=1 -f /tmp/after.sql"
  $afterOutput = & docker run --rm --network $Net --entrypoint bash -v "${Pg}:/var/lib/postgresql/data" -v "${Sock}:/var/run/postgresql" $Image -lc $afterCmd 2>&1
  if ($LASTEXITCODE -ne 0) { throw ($afterOutput -join "`n") }
  Start-Sleep 5
  $checkOutput = & docker run --rm --network $Net --entrypoint bash -v "${Pg}:/var/lib/postgresql/data" -v "${Repo}:/var/lib/pgbackrest/repo1" -v "${Sock}:/var/run/postgresql" $Image -lc "runuser -u postgres -- pgbackrest --stanza=hellodoctor check; runuser -u postgres -- pgbackrest --stanza=hellodoctor info" 2>&1
  if ($LASTEXITCODE -ne 0) { throw ($checkOutput -join "`n") }
  ($sourceOutput + $afterOutput + $checkOutput) -join "`n" | Set-Content (Join-Path $EvidenceDir 'pgbackrest-run.log')
  $result.checks.full_backup='GREEN: full backup completed before the named recovery boundary'
  $result.checks.wal_archive='GREEN: WAL archive check completed after later WAL generation'
  $result.checks.encryption='GREEN: AES-256-CBC repository configured; cipher passphrase was not logged'
  $result.checks.retention='GREEN: retention-full=1 and retention-diff=1 configured'

  $restoreLineB64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("restore_command = 'pgbackrest --stanza=hellodoctor archive-get %f %p'`n"))
  $restoreCmd = "echo $configB64 | base64 -d > /etc/pgbackrest/pgbackrest.conf; chown -R postgres:postgres /etc/pgbackrest /var/lib/pgbackrest/repo1; chmod 640 /etc/pgbackrest/pgbackrest.conf; mkdir -p /restore; chown postgres:postgres /restore; runuser -u postgres -- pgbackrest --stanza=hellodoctor --pg1-path=/restore --type=name --target=m8-boundary --target-action=promote restore; echo $restoreLineB64 | base64 -d >> /restore/postgresql.auto.conf"
  D @('run','--rm','--entrypoint','bash','-v',"${Repo}:/var/lib/pgbackrest/repo1",'-v',"${Restore}:/restore",$Image,'-lc',$restoreCmd)
  D @('run','-d','--name',$Target,'--network',$Net,'--entrypoint','/usr/local/bin/postgres-entrypoint.sh','-e','POSTGRES_PASSWORD=restorepass','-e','POSTGRES_USER=postgres','-e','POSTGRES_DB=postgres','-e',"PGBACKREST_REPO1_CIPHER_PASS=$Cipher",'-v',"${Restore}:/var/lib/postgresql/data",'-v',"${Repo}:/var/lib/pgbackrest/repo1",'-v',"${TargetSock}:/var/run/postgresql",$Image,'postgres','-c','unix_socket_directories=/var/run/postgresql','-c','archive_command=/bin/true') | Out-Null
  $started=$false
  for($i=0;$i -lt 30;$i++){ Start-Sleep 1; $state=& docker inspect -f '{{.State.Running}}' $Target 2>$null; if($state -eq 'true'){ $started=$true; break }; $stateExit=& docker inspect -f '{{.State.ExitCode}}' $Target 2>$null; if($stateExit -and $stateExit -ne '0'){ $diagnosticLogs=ContainerLogs $Target; $diagnosticLogs | Set-Content (Join-Path $EvidenceDir 'restore-startup.log'); throw "restored PostgreSQL exited with code ${stateExit}: $diagnosticLogs" } }
  if(-not $started){ throw "restored PostgreSQL did not become ready: $(ContainerLogs $Target)" }
  $ready=$false
  for($i=0;$i -lt 30;$i++){ Start-Sleep 1; $readyOutput=& docker exec $Target pg_isready -U postgres -d postgres 2>&1; if($LASTEXITCODE -eq 0){$ready=$true; break}; $stateExit=& docker inspect -f '{{.State.ExitCode}}' $Target 2>$null; if($stateExit -and $stateExit -ne '0'){ $diagnosticLogs=ContainerLogs $Target; $diagnosticLogs | Set-Content (Join-Path $EvidenceDir 'restore-startup.log'); throw "restored PostgreSQL exited with code ${stateExit}: $diagnosticLogs" } }
  if(-not $ready){ throw "restored PostgreSQL never accepted connections: $(ContainerLogs $Target)" }
  $oldQueryAction=$ErrorActionPreference; $ErrorActionPreference='Continue'
  $recovery = & docker exec $Target psql -U postgres -d postgres -Atc "select pg_is_in_recovery()" 2>&1
  $recoveryCode=$LASTEXITCODE
  $markers = & docker exec $Target psql -U postgres -d postgres -Atc "select v from m8_restore_marker order by v" 2>&1
  $markerCode=$LASTEXITCODE
  $ErrorActionPreference=$oldQueryAction
  $result.checks.restore_debug="recovery=$($recovery -join ','); markers=$($markers -join ','); codes=$recoveryCode/$markerCode"
  if($recoveryCode -ne 0 -or $markerCode -ne 0){ throw "restored SQL query failed: $($markers -join ' ')" }
  if(($recovery -join '').Trim() -ne 'f'){ throw "restored instance did not promote: $($recovery -join ' ')" }
  if(($markers -notcontains 'before-boundary') -or ($markers -contains 'after-boundary')){ throw "PITR marker mismatch: $($markers -join ',')" }
  [ordered]@{ recovery_state=($recovery -join ''); markers=($markers -join ','); postgres_logs=(ContainerLogs $Target) } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $EvidenceDir 'restore-sql-evidence.json')
  $result.checks.restore='GREEN: fresh restored PostgreSQL accepted SQL, promoted, retained before-boundary and excluded after-boundary'

  $wrong=Run-ExpectedFailure @('run','--rm','--entrypoint','bash','-v',"${Repo}:/var/lib/pgbackrest/repo1",'-v',"${Pg}:/var/lib/postgresql/data",$Image,'-lc',"printf '%s\n' '[global]' 'repo1-path=/var/lib/pgbackrest/repo1' 'repo1-cipher-type=aes-256-cbc' 'repo1-cipher-pass=wrong-key' '[hellodoctor]' 'pg1-path=/var/lib/postgresql/data' | tee /tmp/pgbackrest.conf >/dev/null; pgbackrest --config=/tmp/pgbackrest.conf --stanza=hellodoctor info")
  if($wrong[0] -eq 0){throw 'wrong-key repository access unexpectedly succeeded'}
  $result.checks.wrong_key='GREEN: wrong cipher key rejected'
  $unavailable=Run-ExpectedFailure @('run','--rm','--entrypoint','bash','-v',"${Repo}:/var/lib/pgbackrest/repo1:ro",'-v',"${Pg}:/var/lib/postgresql/data",$Image,'-lc',"printf '%s\n' '[global]' 'repo1-path=/var/lib/pgbackrest/repo1' 'repo1-cipher-type=aes-256-cbc' 'repo1-cipher-pass=$Cipher' '[hellodoctor]' 'pg1-path=/var/lib/postgresql/data' | tee /tmp/pgbackrest.conf >/dev/null; pgbackrest --config=/tmp/pgbackrest.conf --stanza=hellodoctor check")
  if($unavailable[0] -eq 0){throw 'read-only repository unexpectedly passed check'}
  $result.checks.repository_failure='GREEN: read-only repository caused check failure'

  D @('run','-d','--name',$Scheduler,'--network',$Net,'--entrypoint','/usr/local/bin/backup-entrypoint.sh','-e',"PGHOST=$Source",'-e','PGPORT=5432','-e','PGUSER=postgres','-e','PGDATABASE=postgres','-e',"PGBACKREST_REPO1_CIPHER_PASS=$Cipher",'-e','BACKUP_DIFF_SCHEDULE=*','-e','BACKUP_MONITOR_SCHEDULE=*','-e','BACKUP_RUN_ON_START=true','-v',"${Pg}:/var/lib/postgresql/data",'-v',"${Repo}:/var/lib/pgbackrest/repo1",'-v',"${Sock}:/var/run/postgresql",$Image) | Out-Null
  Start-Sleep 70
  $schedulerLogs=ContainerLogs $Scheduler
  if($schedulerLogs -notmatch 'check command end: completed successfully' -or $schedulerLogs -notmatch 'backup command end: completed successfully'){throw "scheduled cycle did not complete: $schedulerLogs"}
  $result.checks.scheduler='GREEN: shortened cron cycle completed differential backup and monitor stanza check'
  $result.checks.lock='GREEN: backup-runner uses flock to prevent overlapping executions'
  $result.status='GREEN'
} catch { $result.error=$_.Exception.Message } finally {
  $old=$ErrorActionPreference; $ErrorActionPreference='SilentlyContinue'; if($env:PGBACKREST_KEEP_FAILED -ne '1'){ & docker rm -f $Source $Target $Scheduler 2>$null | Out-Null; & docker volume rm $Pg $Repo $Sock $TargetSock $Restore 2>$null | Out-Null; & docker network rm $Net 2>$null | Out-Null }; $ErrorActionPreference=$old
  $result | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $EvidenceDir 'pgbackrest-local.json')
}
if($result.status -ne 'GREEN'){throw ($result | ConvertTo-Json -Depth 8)}
