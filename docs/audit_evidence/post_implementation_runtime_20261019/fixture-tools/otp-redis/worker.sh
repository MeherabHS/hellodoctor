#!/bin/sh
set -eu
recipient="$1"
redis-cli -s /scratch/redis.sock --raw --eval /harness/reserve.lua ratelimit:otp-delivery:global "ratelimit:otp-delivery:cooldown:$recipient" "ratelimit:otp-delivery:rolling:$recipient" "ratelimit:otp-delivery:daily:$recipient" > "/scratch/$3-$2.result"
