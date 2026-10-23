#!/bin/sh
exec /bin/busybox nc -w 900 hd-acceptance-pg-20261019 5432
