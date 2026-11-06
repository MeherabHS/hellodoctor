#!/usr/bin/env bash
set -Eeuo pipefail

# Read-only preflight for the fixed Azure target. It does not install packages,
# change firewall rules, create volumes, or start application services.

fail() { echo "FAIL: $*" >&2; exit 1; }
require_cmd() { command -v "$1" >/dev/null 2>&1 || fail "missing command: $1"; }

test "$(uname -m)" = "aarch64" || fail "host architecture is not aarch64"
test -r /etc/os-release || fail "/etc/os-release is unavailable"
# shellcheck disable=SC1091
. /etc/os-release
test "${ID:-}" = "ubuntu" || fail "expected Ubuntu, found ${ID:-unknown}"
test "${VERSION_ID:-}" = "24.04" || fail "expected Ubuntu 24.04, found ${VERSION_ID:-unknown}"

require_cmd docker
docker info >/dev/null 2>&1 || fail "Docker daemon is unavailable"
docker compose version >/dev/null 2>&1 || fail "Docker Compose plugin is unavailable"

docker_arch="$(docker info --format '{{.Architecture}}')"
case "$docker_arch" in
  aarch64|arm64) ;;
  *) fail "Docker reports $docker_arch instead of an ARM64 architecture" ;;
esac

docker buildx version >/dev/null 2>&1 || fail "Docker Buildx is unavailable"

# Leave at least 12 GiB free for image layers, PostgreSQL WAL, and backups.
test -d /var/lib/docker || fail "/var/lib/docker is not present"
free_kib="$(df -Pk /var/lib/docker 2>/dev/null | awk 'NR==2 {print $4}')"
[[ "$free_kib" =~ ^[0-9]+$ ]] || fail "could not read free space for /var/lib/docker"
test "$free_kib" -ge 12582912 || fail "/var/lib/docker has less than 12 GiB free"

echo "PASS: Ubuntu ${VERSION_ID} aarch64 with Docker ARM64 runtime is ready for image and Compose preflight"
