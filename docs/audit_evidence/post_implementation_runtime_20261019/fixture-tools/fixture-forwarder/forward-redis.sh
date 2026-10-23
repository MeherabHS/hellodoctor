#!/bin/sh
exec /bin/busybox nc -w 900 hd-acceptance-redis-20261019 6379
