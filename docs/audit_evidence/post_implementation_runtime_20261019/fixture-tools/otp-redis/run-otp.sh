#!/bin/sh
set -eu
umask 077
redis-server --port 0 --unixsocket /scratch/redis.sock --unixsocketperm 700 --save '' --appendonly no --daemonize yes --pidfile /scratch/redis.pid --logfile /scratch/redis.log --dir /scratch
trap 'redis-cli -s /scratch/redis.sock shutdown nosave >/dev/null 2>&1 || true' EXIT INT TERM
r() { redis-cli -s /scratch/redis.sock --raw "$@"; }
G='ratelimit:otp-delivery:global'
C='ratelimit:otp-delivery:cooldown:'
R='ratelimit:otp-delivery:rolling:'
D='ratelimit:otp-delivery:daily:'
reserve() { r --eval /harness/reserve.lua "$G" "$C$1" "$R$1" "$D$1"; }
eq() { if [ "$1" != "$2" ]; then echo "DEFENSE_FAIL expected=$2 actual=$1 label=$3"; exit 1; fi; }
ttl() { value=$(r TTL "$1"); if [ "$value" -le 0 ] || [ "$value" -gt "$2" ]; then echo "DEFENSE_FAIL ttl=$value maximum=$2 key=$1"; exit 1; fi; printf '%s=%s ' "$1" "$value"; }
four() { ttl "$G" 60; ttl "$C$1" 60; ttl "$R$1" 900; ttl "$D$1" 86400; echo; }

r FLUSHDB >/dev/null
eq "$(reserve same)" 1 seed
seq 1 100 | xargs -P 8 -I '{}' /bin/sh /harness/worker.sh same '{}' cooldown
eq "$(r GET "$G")" 1 cooldown-global
eq "$(r GET "${C}same")" 1 cooldown-recipient
eq "$(r GET "${R}same")" 1 cooldown-rolling
eq "$(r GET "${D}same")" 1 cooldown-daily
eq "$(cat /scratch/cooldown-*.result | grep -c '^0$')" 100 cooldown-response-count
four same
echo 'case=cooldown_storm requests=101 concurrent_retries=100 global=1 cooldown=1 rolling=1 daily=1 denied=100 verdict=DEFENSE_PASS'

r FLUSHDB >/dev/null
seq 1 100 | xargs -P 8 -I '{}' /bin/sh /harness/worker.sh 'distinct-{}' '{}' distinct
eq "$(r GET "$G")" 30 distinct-global
eq "$(r DBSIZE)" 91 distinct-key-count
accepted=0
rejected=0
for i in $(seq 1 100); do
    recipient="distinct-$i"
    if [ "$(r EXISTS "$C$recipient")" = 1 ]; then
        eq "$(r GET "$C$recipient")" 1 accepted-cooldown
        eq "$(r GET "$R$recipient")" 1 accepted-rolling
        eq "$(r GET "$D$recipient")" 1 accepted-daily
        accepted=$((accepted+1))
    else
        eq "$(r EXISTS "$R$recipient" "$D$recipient")" 0 denied-recipient-keys
        rejected=$((rejected+1))
    fi
done
eq "$accepted" 30 distinct-observed-accepted
eq "$rejected" 70 distinct-observed-rejected
eq "$(cat /scratch/distinct-*.result | grep -c '^1$')" 30 distinct-accepted-response-count
eq "$(cat /scratch/distinct-*.result | grep -c '^0$')" 70 distinct-denied-response-count
eq "$(reserve global-denied)" 0 global-denial
eq "$(r EXISTS "${C}global-denied" "${R}global-denied" "${D}global-denied")" 0 global-denied-created-no-keys
eq "$(r GET "$G")" 30 global-denial-unchanged
echo "case=distinct_recipients requests=101 concurrent_requests=100 observed_accepted=$accepted observed_denied=$rejected additional_global_denied=1 global=30 keys=91 verdict=DEFENSE_PASS"

r FLUSHDB >/dev/null
r SET "$G" 7 EX 60 >/dev/null
r SET "${R}rolling" 2 EX 900 >/dev/null
r SET "${D}rolling" 5 EX 86400 >/dev/null
eq "$(reserve rolling)" 1 rolling-boundary-allowed
eq "$(r GET "$G")" 8 rolling-global
eq "$(r GET "${R}rolling")" 3 rolling-third
eq "$(r GET "${D}rolling")" 6 rolling-daily
four rolling
r DEL "${C}rolling" >/dev/null
eq "$(reserve rolling)" 0 rolling-fourth-denied
eq "$(r GET "$G")" 8 rolling-rejection-global-unchanged
eq "$(r GET "${R}rolling")" 3 rolling-rejection-rolling-unchanged
eq "$(r GET "${D}rolling")" 6 rolling-rejection-daily-unchanged
eq "$(r EXISTS "${C}rolling")" 0 rolling-rejection-no-cooldown
echo 'case=rolling_boundary requests=2 seeded_rolling=2 allowed=1 denied=1 global=8 rolling=3 daily=6 cooldown_after_denial=absent verdict=DEFENSE_PASS'

r FLUSHDB >/dev/null
r SET "$G" 7 EX 60 >/dev/null
r SET "${R}daily" 1 EX 900 >/dev/null
r SET "${D}daily" 19 EX 86400 >/dev/null
eq "$(reserve daily)" 1 daily-boundary-allowed
eq "$(r GET "$G")" 8 daily-global
eq "$(r GET "${R}daily")" 2 daily-rolling
eq "$(r GET "${D}daily")" 20 daily-twentieth
four daily
r DEL "${C}daily" >/dev/null
eq "$(reserve daily)" 0 daily-twentyfirst-denied
eq "$(r GET "$G")" 8 daily-rejection-global-unchanged
eq "$(r GET "${R}daily")" 2 daily-rejection-rolling-unchanged
eq "$(r GET "${D}daily")" 20 daily-rejection-daily-unchanged
eq "$(r EXISTS "${C}daily")" 0 daily-rejection-no-cooldown
echo 'case=daily_boundary requests=2 seeded_daily=19 allowed=1 denied=1 global=8 rolling=2 daily=20 cooldown_after_denial=absent verdict=DEFENSE_PASS'

r FLUSHDB >/dev/null
r SET "$G" 7 EX 60 >/dev/null
r SET "${R}invalid" not-a-number EX 900 >/dev/null
eq "$(reserve invalid)" 'Invalid delivery budget counter' invalid-counter-error
eq "$(r GET "$G")" 7 invalid-global-unchanged
eq "$(r GET "${R}invalid")" not-a-number invalid-rolling-unchanged
eq "$(r EXISTS "${C}invalid" "${D}invalid")" 0 invalid-other-keys-not-created
echo 'case=invalid_counter requests=1 global=7 other_keys=absent verdict=DEFENSE_PASS'
echo 'OTP_LUA_REDIS_SCOPE=DEFENSE_PASS cases=5 reserve_calls=207 max_parallel_workers=8'
