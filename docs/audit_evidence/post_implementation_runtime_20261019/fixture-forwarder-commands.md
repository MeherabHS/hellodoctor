Fixture networking commands executed on 2026-10-19, after ownership-label verification.
All named resources belong to `hellodoctor.acceptance.run=runtime-20261019`.
The internal network and initial empty PG/Redis containers were created by the parent run.
No shared containers, volumes or networks were changed.

The initial internal-network dynamic published ports did not materialize. The
following explicit-port retry also produced `NetworkSettings.Ports={"5432/tcp":[]}`
and the host loopback TCP probe timed out. The removed PG container was empty,
newly created for this campaign, and its ownership label was verified first.

```powershell
docker rm -f hd-acceptance-pg-20261019
docker run -d --name hd-acceptance-pg-20261019 --pull never --network hd-acceptance-20261019 --label hellodoctor.acceptance.run=runtime-20261019 --read-only --user postgres --cap-drop ALL --security-opt no-new-privileges --cpus 1 --memory 512m --memory-swap 512m --pids-limit 64 --shm-size 32m --tmpfs /var/lib/postgresql/data:rw,nosuid,nodev,noexec,size=384m,uid=999,gid=999,mode=0700 --tmpfs /var/run/postgresql:rw,nosuid,nodev,noexec,size=1m,uid=999,gid=999,mode=0700 -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres -p 127.0.0.1:15432:5432 postgres:16
docker inspect hd-acceptance-pg-20261019 --format '{{json .NetworkSettings.Ports}}'
docker exec hd-acceptance-pg-20261019 pg_isready -U postgres
docker exec hd-acceptance-redis-20261019 /bin/busybox nc --help
```

Parent authorized the following fixture-owned forwarding route after the direct
internal-network publishing failure. `forwarderRoot` resolves to the new scratch
fixture inputs in this repository. Two fixed-destination scripts connect only to
the owned PG/Redis aliases. BusyBox `nc -lk -e` forks for concurrent connections.

```powershell
$forwarderRoot=Join-Path (Get-Location) 'scratch/acceptance-runtime-20261019/fixture-forwarder'
docker network create --driver bridge --label hellodoctor.acceptance.run=runtime-20261019 hd-acceptance-forwarding-20261019
docker create --name hd-acceptance-forwarder-20261019 --pull never --network hd-acceptance-forwarding-20261019 --label hellodoctor.acceptance.run=runtime-20261019 --read-only --user 999:1000 --cap-drop ALL --security-opt no-new-privileges --cpus 1 --memory 128m --memory-swap 128m --pids-limit 64 --ulimit nofile=128:128 --ulimit fsize=1048576:1048576 --ulimit cpu=1800:1800 --tmpfs /dev/shm:ro,nosuid,nodev,noexec,size=1m,mode=0555 --tmpfs /dev/mqueue:ro,nosuid,nodev,noexec,size=1m,mode=0555 --mount "type=bind,source=$forwarderRoot,target=/harness,readonly" -p 127.0.0.1:15432:5432 -p 127.0.0.1:16379:6379 --entrypoint /bin/busybox redis:7-alpine timeout -s KILL 1800 /usr/bin/env -i PATH=/usr/local/bin:/usr/bin:/bin /bin/sh -c '/bin/busybox nc -lk -p 5432 -e /harness/forward-pg.sh & /bin/busybox nc -lk -p 6379 -e /harness/forward-redis.sh & wait'
docker network connect hd-acceptance-20261019 hd-acceptance-forwarder-20261019
docker start hd-acceptance-forwarder-20261019
docker inspect hd-acceptance-forwarder-20261019 --format '{{json .NetworkSettings.Ports}}'
docker port hd-acceptance-forwarder-20261019
python -I -S scratch/acceptance-runtime-20261019/fixture-forwarder/probe.py
```

Final probe: PASS. Eight simultaneous PG connections were independently counted
from a ninth PG SQL connection; Redis returned PONG. Forwarder started at
`2026-10-19T14:14:38.455557826Z` and has a 1800-second execution deadline.
PG/Redis remain on the internal bridge only. The forwarding bridge is an ordinary
Docker bridge; this composite fixture is **not** a network-none sandbox. Only the
trusted forwarder is dual-attached, and host ports bind to literal loopback.

This agent leaves the owned fixture running for the parent's SQLx/application
tests. Cleanup must verify ownership labels and remove only these named campaign
resources after their consumers terminate. No cleanup of shared resources is
authorized by this log.
