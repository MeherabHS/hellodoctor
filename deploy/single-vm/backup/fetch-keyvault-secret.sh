#!/usr/bin/env bash
# Fetches one secret from Azure Key Vault using the VM/container's managed
# identity (via Azure Instance Metadata Service — no account key, SAS token,
# or service-principal secret ever touches this script or this image) and
# writes ONLY the secret value to the given output file, 0600, owned by
# postgres. A sibling "<output>.version" file records the secret's full
# versioned identifier (never its value) so a recovery manifest can cite
# exactly which Key Vault secret version produced a given backup, per
# docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md's custody requirements.
#
# This automates the *routine* case: the same long-lived VM restarting its
# containers, where the VM's managed identity has already been granted
# `get`/`list` on this one secret in Key Vault (least privilege — not
# Key Vault Contributor, not access to any other secret). It does not change
# who can grant that access in the first place, who can read/rotate the
# secret's value through Key Vault's own RBAC, or the manual procedure a
# *replacement* VM's new identity still needs granted during a real VM-loss
# recovery (docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md's "VM-loss recovery"
# section) — those remain exactly as documented there.
#
# Usage: fetch-keyvault-secret.sh <vault-name> <secret-name> <secret-version-or-empty> <output-file>
#
# Required: AZURE_CLIENT_ID may be set to select a user-assigned identity when
# the host has more than one identity attached; leave unset for the system-
# assigned identity (pgBackRest's own `repo1-azure-key-type=auto` has no
# client-ID selector today, so a user-assigned identity is untested for the
# repository itself — see docs/BACKUP_RESTORE_RUNBOOK.md's RBAC section. This
# script is independent of that and supports client_id regardless.)
set -Eeuo pipefail

vault_name="${1:?usage: fetch-keyvault-secret.sh <vault-name> <secret-name> <secret-version-or-empty> <output-file>}"
secret_name="${2:?usage: fetch-keyvault-secret.sh <vault-name> <secret-name> <secret-version-or-empty> <output-file>}"
secret_version="${3:-}"
output_file="${4:?usage: fetch-keyvault-secret.sh <vault-name> <secret-name> <secret-version-or-empty> <output-file>}"

imds_url="http://169.254.169.254/metadata/identity/oauth2/token?api-version=2018-02-01&resource=https%3A%2F%2Fvault.azure.net"
if [ -n "${AZURE_CLIENT_ID:-}" ]; then
  imds_url="${imds_url}&client_id=${AZURE_CLIENT_ID}"
fi

token_response="$(curl --fail --silent --show-error --max-time 10 -H 'Metadata: true' "$imds_url")" \
  || { echo "fetch-keyvault-secret: failed to reach Azure Instance Metadata Service for a managed-identity token. This only works on an Azure VM/VMSS with a managed identity attached and Key Vault access granted." >&2; exit 1; }

access_token="$(printf '%s' "$token_response" | jq -r '.access_token // empty')"
unset token_response
if [ -z "$access_token" ]; then
  echo "fetch-keyvault-secret: IMDS response did not contain an access_token." >&2
  exit 1
fi

secret_url="https://${vault_name}.vault.azure.net/secrets/${secret_name}"
if [ -n "$secret_version" ]; then
  secret_url="${secret_url}/${secret_version}"
fi
secret_url="${secret_url}?api-version=7.4"

secret_response="$(curl --fail --silent --show-error --max-time 10 -H "Authorization: Bearer ${access_token}" "$secret_url")"
rc=$?
unset access_token
if [ "$rc" -ne 0 ] || [ -z "$secret_response" ]; then
  echo "fetch-keyvault-secret: failed to retrieve secret '${secret_name}' from vault '${vault_name}' (curl exit ${rc}). Check that this identity has been granted 'Get' on this secret via Key Vault RBAC, and that the secret exists." >&2
  exit 1
fi

secret_value="$(printf '%s' "$secret_response" | jq -r '.value // empty')"
secret_id="$(printf '%s' "$secret_response" | jq -r '.id // empty')"
unset secret_response
if [ -z "$secret_value" ]; then
  echo "fetch-keyvault-secret: Key Vault response for '${secret_name}' did not contain a value." >&2
  exit 1
fi

install -m 0600 -o postgres -g postgres /dev/null "$output_file"
printf '%s' "$secret_value" > "$output_file"
unset secret_value

if [ -n "$secret_id" ]; then
  printf '%s\n' "$secret_id" > "${output_file}.version"
  chown postgres:postgres "${output_file}.version"
  chmod 0644 "${output_file}.version"
fi
unset secret_id

echo "fetch-keyvault-secret: wrote secret '${secret_name}' from vault '${vault_name}' to ${output_file} (version recorded in ${output_file}.version, value not logged)" >&2
