$ErrorActionPreference = 'Stop'
$pfx = "m8-restore-diagnose-$([guid]::NewGuid().ToString('N').Substring(0,8))"
$net="$pfx-net"; $data="$pfx-data"; $repo="$pfx-repo"; $sock="$pfx-sock"; $rest="$pfx-rest"; $src="$pfx-src"; $dst="$pfx-dst"; $image='hellodoctor-pgbackrest:m8-local'; $key='diagnostic-key'
function D([string[]]$a){ & docker @a; if($LASTEXITCODE -ne 0){throw "docker failed: $($a -join ' ')"} }
try {
  D @('network','create',$net); foreach($v in @($data,$repo,$sock,$rest)){D @('volume','create',$v)}
  D @('run','-d','--name',$src,'--network',$net,'--entrypoint','/usr/local/bin/postgres-entrypoint.sh','-e','POSTGRES_PASSWORD=testpass','-e','POSTGRES_USER=postgres','-e','POSTGRES_DB=postgres',"-e","PGBACKREST_REPO1_CIPHER_PASS=$key",'-v',"${data}:/var/lib/postgresql/data",'-v',"${repo}:/var/lib/pgbackrest/repo1",'-v',"${sock}:/var/run/postgresql",$image,'postgres','-c','archive_mode=on','-c','archive_command=pgbackrest --stanza=hellodoctor archive-push %p','-c','unix_socket_directories=/var/run/postgresql') | Out-Null
  Start-Sleep 8
  $cfg="[global]`nrepo1-path=/var/lib/pgbackrest/repo1`nrepo1-cipher-type=aes-256-cbc`nrepo1-cipher-pass=$key`nstart-fast=y`n[hellodoctor]`npg1-path=/var/lib/postgresql/data`npg1-socket-path=/var/run/postgresql`n"
  $b64=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($cfg))
  $sql="create table restore_marker(v text primary key); insert into restore_marker values ('before');"
  $sqlB64=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($sql))
  $cmd="echo $b64 | base64 -d >/etc/pgbackrest/pgbackrest.conf; echo $sqlB64 | base64 -d >/tmp/restore.sql; chown -R postgres:postgres /etc/pgbackrest /var/lib/pgbackrest/repo1; chmod 640 /etc/pgbackrest/pgbackrest.conf; runuser -u postgres -- pgbackrest --stanza=hellodoctor stanza-create; runuser -u postgres -- psql -v ON_ERROR_STOP=1 -f /tmp/restore.sql; runuser -u postgres -- pgbackrest --stanza=hellodoctor --type=full backup"
  & docker run --rm --network $net --entrypoint bash -v "${data}:/var/lib/postgresql/data" -v "${repo}:/var/lib/pgbackrest/repo1" -v "${sock}:/var/run/postgresql" $image -lc $cmd
  $restoreCmd="echo $b64 | base64 -d >/etc/pgbackrest/pgbackrest.conf; chown -R postgres:postgres /etc/pgbackrest /var/lib/pgbackrest/repo1; mkdir -p /restore; chown postgres:postgres /restore; runuser -u postgres -- pgbackrest --stanza=hellodoctor --pg1-path=/restore restore"
  D @('run','--rm','--entrypoint','bash','-v',"${repo}:/var/lib/pgbackrest/repo1",'-v',"${rest}:/restore",$image,'-lc',$restoreCmd)
  D @('run','-d','--name',$dst,'--network',$net,'--user','999:999','--entrypoint','postgres','-v',"${rest}:/var/lib/postgresql/data",'postgres:16','-D','/var/lib/postgresql/data','-c','listen_addresses=','-c','archive_command=/bin/true','-c','restore_command=') | Out-Null
  Start-Sleep 8
  $old=$ErrorActionPreference; $ErrorActionPreference='Continue'
  $inspect=& docker inspect $dst 2>&1
  $logs=& docker logs $dst 2>&1
  $exit=& docker inspect -f '{{.State.ExitCode}}' $dst 2>&1
  $meta=& docker run --rm --entrypoint bash -v "${rest}:/restore" $image -lc 'stat -c "%u:%g %a %n" /restore /restore/PG_VERSION; cat /restore/postgresql.auto.conf 2>/dev/null || true; ls -la /restore | head -30' 2>&1
  $ErrorActionPreference=$old
  [pscustomobject]@{prefix=$pfx; exit_code=$exit; inspect=($inspect -join "`n"); logs=($logs -join "`n"); restored_metadata=($meta -join "`n")} | ConvertTo-Json -Depth 5 | Set-Content docs/recovery-evidence/m8-pgbackrest-local/restore-diagnosis.json
} finally {
  $ErrorActionPreference='SilentlyContinue'; docker rm -f $src $dst 2>$null | Out-Null; docker volume rm $data $repo $sock $rest 2>$null | Out-Null; docker network rm $net 2>$null | Out-Null
}
