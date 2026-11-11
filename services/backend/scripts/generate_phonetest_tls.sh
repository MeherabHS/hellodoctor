#!/usr/bin/env bash
# Generates the self-signed TLS cert/key used by Caddyfile.phonetest for the
# two-phone physical Agora test's HTTPS endpoint. A static, explicitly-loaded
# cert (rather than Caddy's `tls internal` automatic SNI-matched cert) is
# required here because a client connecting to a bare IP address sends no
# SNI (RFC 6066 forbids IP literals in SNI), and Caddy's no-SNI matching
# falls back to the connection's local socket address - inside Docker that
# is the container's own bridge IP, never the host's LAN IP the cert needs
# to cover, so automatic matching always fails for this use case.
#
# Re-run this whenever the test host's LAN IP changes (edit LAN_IP below and
# in Caddyfile.phonetest to match). 7-day validity - this is throwaway local
# test tooling, never a real certificate authority.
set -euo pipefail
LAN_IP="192.168.0.103"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$SCRIPT_DIR/../phonetest-tls"
mkdir -p "$OUT_DIR"

cat > "$OUT_DIR/openssl.cnf" << EOF
[req]
distinguished_name = req_distinguished_name
x509_extensions = v3_ca
prompt = no

[req_distinguished_name]
CN = HelloDoctor Phone Test Local CA

[v3_ca]
basicConstraints = critical,CA:TRUE,pathlen:0
keyUsage = critical,keyCertSign,cRLSign,digitalSignature
subjectAltName = IP:$LAN_IP
EOF

openssl req -x509 -newkey rsa:2048 -keyout "$OUT_DIR/phonetest.key" -out "$OUT_DIR/phonetest.crt" \
    -days 7 -nodes -config "$OUT_DIR/openssl.cnf"

echo "Generated $OUT_DIR/phonetest.crt (IP SAN: $LAN_IP) and phonetest.key"
echo "Restart the caddy service in docker-compose.phonetest.yml to pick it up."
