# Azure VM Operator Runbook — ARM64 Registry Deployment

**Audience:** the operator executing commands via **1Panel Terminal** on the existing Azure ARM64 VM.
**Status:** instructions only. No command in this document has been executed. Every command requires explicit operator execution — nothing here runs automatically.
**Prerequisite:** the registry-publication steps in `docs/deployment/ARM64_GHCR_RELEASE_HANDOFF.md` (Git staging, GHCR workflow, digest collection) are a separate, already-authorized-pending process — this runbook assumes real digest-pinned images and a working read-only GHCR credential already exist by the time Phase A begins. It does not redesign that process.

**Current reference revisions (as of 2026-11-08, verified against the live GitHub API, not assumed):**
- **Current `main` tip** (the deployment baseline for direct git-to-VM checkout, per the standing decision to treat the existing merged release as the first synthetic-data deployment baseline): `aadcb77aa52311fa343f6947ba08397598b5caa9` (PR #7's application merge `b9d72dea` plus PR #8's CI-only revision-check fix).
- **Eligible `source_revision` input if `arm64-ghcr-publish.yml` is ever dispatched**: `b9d72deab79fe868b5d9af561bda341a2f2a8af6` — **not** the current `main` tip. Verified directly: the current tip's required ARM64/migration checks never ran against it (PR #8 touched only `.github/` files, so `arm64-compatibility.yml`'s path-filtered trigger correctly skipped it), while `b9d72dea` has all five required checks recorded as `success` and is confirmed to still be an ancestor of current `main`. Using the tip SHA instead would fail the workflow's own required-checks gate.
- These two values are deliberately different and must not be conflated. Re-verify both before relying on them if significant time has passed or `main` has moved since this note was written.

## Standing restrictions (apply to every phase below)

- **Never** modify 1Panel's own port bindings, its localhost-only binding, or the existing Cloudflare Tunnel's ingress rules for any service other than the one new rule this deployment adds.
- **Never** touch Azure NSG rules or the existing SSH/Bastion recovery path.
- **Never** run `docker compose down -v`, `docker system prune` (indiscriminately), or any destructive SQL (`DROP`/`TRUNCATE`/`DELETE`) against Postgres.
- **Never** install Rust, Node.js, PostgreSQL, or Redis directly on the VM — containers only.
- **Never** create a new Azure resource (Storage Account, Key Vault, ACS resource, container) without first running the "does it already exist" check given in that phase.
- Every command below is written assuming it is pasted into 1Panel's terminal one line at a time; `&&`-chained one-liners are used where a step is genuinely a single atomic action.
- `<REPO>` = wherever this repository is checked out on the VM. `.env.prod` = the real, non-tracked environment file populated from `deploy/single-vm/arm64-images.env.example`'s template — **its exact chosen path requires verification/confirmation from the operator**, shown here as `deploy/single-vm/.env.prod`.

---

## Phase A — Registry image pull and container startup

### A1. Authenticate to GHCR (read-only pull token)

1. **Working directory:** `<REPO>` (host-global, directory doesn't matter)
2. **Command:** `echo "$GHCR_READONLY_TOKEN" | docker login ghcr.io -u <github-username> --password-stdin`
3. **Config variables:** `GHCR_READONLY_TOKEN` — a GHCR PAT scoped to `read:packages` only, never push-capable. **Requires verification:** exact token value and storage location (expected: a root-only file or a 1Panel-managed secret, generated out-of-band — never invented here).
4. **Verify success:** the login prompt prints "Login Succeeded"; fully confirmed by A2's pull succeeding.
5. **Expected result / failure modes:** silent success expected. `401`/`403` means the token lacks `read:packages`, or the GHCR packages aren't yet shared with this identity (org SSO enforcement is a common cause — see the handoff doc §5).
6. **Rollback:** `docker logout ghcr.io` — local credential removal only, no effect on anything running.
7. **Azure Portal required?** No.
8. **Flags:** No container restart, no data change, no cost, no port exposure.

### A2. Pull the four digest-pinned images

1. **Working directory:** `<REPO>`
2. **Command:** `set -a && source deploy/single-vm/.env.prod && set +a && docker pull "$BACKEND_RUNTIME_IMAGE" && docker pull "$ADMIN_WEB_IMAGE" && docker pull "$DOCTOR_WEB_IMAGE" && docker pull "$POSTGRES_BACKUP_IMAGE"`
3. **Config variables:** `BACKEND_RUNTIME_IMAGE`, `ADMIN_WEB_IMAGE`, `DOCTOR_WEB_IMAGE`, `POSTGRES_BACKUP_IMAGE` — each a full `ghcr.io/...@sha256:<digest>` reference. **Requires verification:** confirm these are the real, reviewed digests from the publish workflow's output, not placeholders — the prior container-readiness audit found `POSTGRES_BACKUP_IMAGE`'s template value was still an unfilled placeholder; confirm it was replaced before running this.
4. **Verify success:** `docker image inspect --format '{{index .RepoDigests 0}}' "$BACKEND_RUNTIME_IMAGE"` (repeat per image) — printed digest must exactly match the value in `.env.prod`.
5. **Expected / failure modes:** four successful pulls expected. `manifest unknown` = digest never published or wrong repo name. `no matching manifest for linux/arm64` = image wasn't actually built for ARM64 — **stop immediately, do not proceed to A3** if this happens.
6. **Rollback:** none needed — a pull with no container started has no running-system effect. `docker rmi <image>` removes a bad pull if disk space matters.
7. **Azure Portal required?** No.
8. **Flags:** No restart/data change/cost beyond VM disk use (not Azure cost); no port exposure.

### A3. Confirm external volumes already exist (do not auto-create)

1. **Working directory:** `<REPO>`
2. **Command:** `docker volume inspect "$POSTGRES_VOLUME" "$REDIS_VOLUME" "$CADDY_DATA_VOLUME" "$CADDY_CONFIG_VOLUME" >/dev/null && echo VOLUMES_OK || echo VOLUMES_MISSING`
3. **Config variables:** `POSTGRES_VOLUME`, `REDIS_VOLUME`, `CADDY_DATA_VOLUME`, `CADDY_CONFIG_VOLUME` — `compose.yml` declares these `external: true`, so Compose will refuse to create them; they must already exist from prior provisioning. **Requires verification:** exact real volume names in use on this VM.
4. **Verify success:** output is `VOLUMES_OK`.
5. **Expected / failure modes:** all four found. `VOLUMES_MISSING` on a genuinely first-ever startup means volume provisioning is a separate, deliberate one-time step outside this runbook — confirm with the VM's original setup record before creating anything (the `external: true` guard exists specifically to prevent silently mounting an empty replacement).
6. **Rollback:** none — read-only inspection.
7. **Azure Portal required?** No.
8. **Flags:** No.

### A4. Set the ingress bind address/ports to avoid colliding with 1Panel

1. **Working directory:** `<REPO>`
2. **Command:** `grep -q '^PROXY_BIND_ADDRESS=' deploy/single-vm/.env.prod || echo 'PROXY_BIND_ADDRESS=127.0.0.1' >> deploy/single-vm/.env.prod; grep -q '^PROXY_HTTP_PORT=' deploy/single-vm/.env.prod || echo 'PROXY_HTTP_PORT=18080' >> deploy/single-vm/.env.prod; grep -q '^PROXY_HTTPS_PORT=' deploy/single-vm/.env.prod || echo 'PROXY_HTTPS_PORT=18443' >> deploy/single-vm/.env.prod`
3. **Config variables:** as above. **Requires verification before running:** `sudo ss -tlnp | grep -E ':(80|443|8888|9999)\b'` first, to confirm these specific replacement ports (`18080`/`18443`) are actually free and that 1Panel's real bindings are what's assumed — do not trust the assumption blindly.
4. **Verify success:** `grep -E '^PROXY_(BIND_ADDRESS|HTTP_PORT|HTTPS_PORT)=' deploy/single-vm/.env.prod` shows all three non-conflicting values.
5. **Expected / failure modes:** three lines present. The dangerous failure mode is *skipping* this step entirely, which leaves the compose default `0.0.0.0:80`/`:443` active and racing 1Panel for the port at container start.
6. **Rollback:** edit/remove the appended lines — nothing is running yet at this point.
7. **Azure Portal required?** No.
8. **Flags:** No container restart yet (nothing running); does not itself expose anything — only sets the value A5 will use.

### A5. Start the six-service stack for the first time

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml up -d`
3. **Config variables:** every `${VAR:?...}` in `compose.yml` must be set in `.env.prod` — `POSTGRES_USER`, `POSTGRES_DB`, `POSTGRES_PASSWORD`, `BACKEND_RUNTIME_IMAGE`, `BACKEND_ENV_FILE` (a separate absolute path to the backend's own secret env file — **requires verification** of its real path/contents), `API_HOST`, `ADMIN_HOST`, `DOCTOR_HOST`, `ADMIN_WEB_IMAGE`, `DOCTOR_WEB_IMAGE`, `WEB_PROXY_SECRET`, `TURNSTILE_SITE_KEY`, `TURNSTILE_SECRET_KEY`, `WEB_AUTH_ASSERTION_SECRET`, `ACME_EMAIL`, `ERASURE_JOURNAL_DIR`, plus the four `*_VOLUME` names from A3.
4. **Verify success:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml ps` — all six show `healthy`.
5. **Expected / failure modes:** `postgres`/`redis`/`backend` become healthy first (per the `depends_on: service_healthy` chain), then `admin-web`/`doctor-web`, then `proxy` last. A missing required env var aborts immediately with `variable is not set` naming the exact variable — read it, don't guess. `backend` staying unhealthy past its `start_period` usually means `BACKEND_ENV_FILE` is unreachable/invalid or the DB password doesn't match. Caddy failing ACME certificate issuance is *expected* before the Cloudflare Tunnel ingress rule (added in a manual step outside Docker, by whoever administers the tunnel) is updated to point at the new port — not an application bug.
6. **Rollback:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml down` (no `-v`) — stops/removes containers only; external volumes are untouched.
7. **Azure Portal required?** No.
8. **Flags:** **Starts six containers for the first time** (the main action of this phase). Does not modify any pre-existing Postgres/Redis data. No Azure cost beyond the VM you already pay for. Binds host ports 18080/18443 (or whatever A4 set) on `127.0.0.1` only — not publicly reachable from this step alone; the separate Cloudflare Tunnel ingress change is what makes it externally reachable.

---

## Phase B — Azure Blob + Key Vault + pgBackRest integration and a real backup/restore round-trip

### B1. Confirm the Storage Account and containers already exist (do not create)

1. **Working directory:** `<REPO>` or the operator's own workstation with Azure CLI against the VM's managed identity context — **requires verification which is intended**.
2. **Command:** `az storage container list --account-name "$AZURE_STORAGE_ACCOUNT_NAME" --auth-mode login --query "[].name" -o tsv`
3. **Config variables:** `AZURE_STORAGE_ACCOUNT_NAME`; expect to see the clinical-document and article-media container names already present.
4. **Verify success:** both expected container names appear.
5. **Expected / failure modes:** empty or permission-denied output means the identity lacks `Storage Blob Data Contributor`/`Reader` — an IAM fix, not a reason to create a new container.
6. **Rollback:** none — read-only.
7. **Azure Portal required?** Possibly — only to confirm/grant IAM role if this check fails. **Never** to create a new Storage Account/container.
8. **Flags:** No.

### B2. Confirm the backend's Blob config matches the real account

1. **Working directory:** `<REPO>`
2. **Command:** `grep -E '^(STORAGE_PROVIDER|AZURE_STORAGE_ACCOUNT_NAME|AZURE_STORAGE_AUTH_MODE|AZURE_STORAGE_CLINICAL_CONTAINER|AZURE_STORAGE_ARTICLE_CONTAINER|AZURE_STORAGE_ENDPOINT)=' "$BACKEND_ENV_FILE"`
3. **Config variables:** `STORAGE_PROVIDER=azure`, `AZURE_STORAGE_AUTH_MODE=managed_identity` (never `account_key` in production), `AZURE_STORAGE_CLINICAL_CONTAINER`, `AZURE_STORAGE_ARTICLE_CONTAINER`, `AZURE_STORAGE_ENDPOINT`.
4. **Verify success:** all six lines present with real, non-placeholder values.
5. **Expected / failure modes:** leftover template text (e.g. `<lowercase-account-name>`) committed verbatim is the common failure.
6. **Rollback:** edit the file; nothing restarts until B3.
7. **Azure Portal required?** No (reading a local file).
8. **Flags:** No.

### B3. Recreate the backend container to pick up any env-file correction

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml up -d --force-recreate backend`
3. **Config variables:** none beyond what's already in `BACKEND_ENV_FILE`.
4. **Verify success:** `docker compose ... ps backend` shows `healthy`.
5. **Expected / failure modes:** healthy within the 60s `start_period`. A repeat of B2's blob-config typo surfaces as repeated unhealthy restarts.
6. **Rollback:** run the same recreate command again after reverting the env file.
7. **Azure Portal required?** No.
8. **Flags:** **Restarts the backend container** (brief in-flight request interruption). No data modification, no cost, no new port exposure.

### B4. Dry-run the backup overlay configuration (no backup taken yet)

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml config --quiet && echo CONFIG_OK`
3. **Config variables:** `PGBACKREST_REPO1_TYPE=azure`, backup-repo Azure account/container names (**separate** from the application's own Blob containers in B1/B2), `PGBACKREST_REPO1_AZURE_KEY_TYPE=auto` (managed identity — leave at default), `PGBACKREST_REPO1_CIPHER_PASS_FILE` (preferred over the plain env-var variant, which is visible via `docker inspect`/`exec env`), `PGBACKREST_REPO1_RETENTION_FULL=7`, `PGBACKREST_REPO1_RETENTION_DIFF=14`, `POSTGRES_BACKUP_IMAGE`, the `PGBACKREST_*_VOLUME` names.
4. **Verify success:** prints `CONFIG_OK` with no YAML errors.
5. **Expected / failure modes:** a missing `${VAR:?message}` aborts naming the exact variable.
6. **Rollback:** none — config validation only, nothing started.
7. **Azure Portal required?** Possibly — only to confirm the backup repo's Storage Account/container and that the VM's managed identity has `Storage Blob Data Contributor` on it specifically.
8. **Flags:** No.

### B5. Apply the backup overlay (recreates `postgres`, starts the `backup` scheduler)

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml up -d`
3. **Config variables:** same as B4.
4. **Verify success:** `docker compose ... -f deploy/single-vm/compose.backup.yml ps postgres backup` both healthy/running.
5. **Expected / failure modes:** `postgres` is recreated onto the pgBackRest-enabled image using the *same* external data volume — an image swap, not a data migration, so existing data is preserved. If the new image's entrypoint can't read the existing data directory correctly, `postgres` will fail to become healthy — have the rollback below ready before running this.
6. **Rollback:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml up -d --force-recreate postgres` — drops back to the plain image without the backup overlay; does **not** touch the external `pgdata` volume itself.
7. **Azure Portal required?** No.
8. **Flags:** **Recreates the `postgres` container** (brief downtime — plan a maintenance window) and starts a new `backup` container. No data deletion. Real Azure egress cost begins only once B7 actually runs a backup. No new port exposure.

### B6. Run `pgbackrest check` (connectivity/config only, no backup yet)

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml exec backup pgbackrest --stanza=hellodoctor check`
3. **Config variables:** none beyond what's already configured.
4. **Verify success:** exits 0, no `ERROR` printed.
5. **Expected / failure modes:** `ERROR: [102]: unable to find a working repository` means the managed-identity role assignment on the backup container is missing, or the account/container name is wrong — an IAM/config check, not a code issue.
6. **Rollback:** none — read-only check.
7. **Azure Portal required?** Possibly, to fix IAM if this fails.
8. **Flags:** No.

### B7. Take one real full backup

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml exec backup pgbackrest --stanza=hellodoctor --type=full backup`
3. **Config variables:** none new.
4. **Verify success:** `... exec backup pgbackrest --stanza=hellodoctor info` lists the new full backup with a recent timestamp.
5. **Expected / failure modes:** completes with no error — this is the first real evidence of a genuine Azure-backed backup. A slow first upload is expected for a large database; let it run, do not interrupt.
6. **Rollback:** none needed — a backup is additive.
7. **Azure Portal required?** Not to configure, but **confirm you expect the real cost** — this incurs real Blob storage + egress billing.
8. **Flags:** **Incurs Azure cost.** No container restart. No existing data modified. No port exposure.

### B8. Verify restore mechanics against a disposable target only

**Deliberately not a one-liner.** A real restore drill requires a *separate*, disposable Postgres container and scratch volume — never the live `postgres` container or the real `$POSTGRES_VOLUME`. Follow `docs/BACKUP_RESTORE_RUNBOOK.md`'s documented restore procedure exactly, pointed at a throwaway volume, and tear the scratch container/volume down afterward.

**Flag — read before running any part of this step:** done correctly, this never touches live data. Done against the wrong target, it is catastrophic. **Triple-check the target volume name is the scratch one, never `$POSTGRES_VOLUME`, before executing anything in this step.** Azure Portal: not required for the restore mechanics themselves. No network port exposure.

---

## Phase C — Azure Communication Services Email integration and real delivery verification

### C1. Confirm ACS env vars are set with real values

1. **Working directory:** `<REPO>`
2. **Command:** `grep -E '^(AZURE_EMAIL_ENDPOINT|AZURE_EMAIL_SENDER|AZURE_EMAIL_AUTH_MODE)=' "$BACKEND_ENV_FILE"; grep -c '^AZURE_EMAIL_ACCESS_KEY=' "$BACKEND_ENV_FILE"`
3. **Config variables:** `AZURE_EMAIL_ENDPOINT` (ACS data-plane base URL, `https://`, no embedded credentials/port), `AZURE_EMAIL_ACCESS_KEY` (base64 HMAC signing key — never printed by this command, only counted), `AZURE_EMAIL_SENDER` (verified "from" address), `AZURE_EMAIL_AUTH_MODE` — **must be `access_key`**; only this mode is implemented. Do not set `managed_identity` — it is explicitly rejected at backend startup.
4. **Verify success:** endpoint/sender print real values; access-key count is `1`.
5. **Expected / failure modes:** one of the three set without the others means the backend refuses to start outside development/test — this is correct fail-closed behavior, not a bug to work around.
6. **Rollback:** edit the file; no container changes until C2.
7. **Azure Portal required?** Yes — confirm the ACS resource's verified sender domain and that the access key belongs to the intended ACS resource. **Never create a new ACS resource here.**
8. **Flags:** No.

### C2. Recreate the backend to pick up the ACS config

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml up -d --force-recreate backend`
3. **Config variables:** none new.
4. **Verify success:** `docker compose ... ps backend` healthy; `docker compose ... logs backend --since 2m | grep -i azure` shows no "not configured"/fatal lines.
5. **Expected / failure modes:** a malformed endpoint URL is rejected at startup with a specific panic message naming the exact problem — read it, don't guess.
6. **Rollback:** same as B3.
7. **Azure Portal required?** No.
8. **Flags:** **Restarts the backend container.** No data modification, no cost yet, no port exposure.

### C3. Send one real test email through the actual OTP path

1. **Working directory:** anywhere (uses `curl` against the already-running stack)
2. **Command:** `curl -s -X POST "https://${API_HOST}:${PROXY_HTTPS_PORT}/api/v1/auth/doctor/login" -H 'Content-Type: application/json' -d '{"email":"<a real doctor test account you control>","password":"<that account'"'"'s real password>"}'`
3. **Config variables:** a **real, already-provisioned** test doctor account the operator controls — **requires verification, do not invent one; do not use the seeded fixture accounts** (`sabrina.akter@hellodoctor.health`, etc.) since those cannot authenticate outside `development`/`test` by design.
4. **Verify success:** response is `200` with no `debug_otp_code` field present (confirms real production-shaped behavior), and the target mailbox actually receives the branded OTP email within the documented worst-case ~16.5s latency window.
5. **Expected / failure modes:** real email arrives, correctly branded, correct code. If the ACS sender domain isn't verified yet, this fails — fix in Azure Portal, not in code.
6. **Rollback:** none needed — a test email has no destructive side effect; the OTP expires on its own TTL.
7. **Azure Portal required?** Yes, if sender-domain verification is incomplete.
8. **Flags:** **Sends one real email** (negligible ACS send cost). No container restart beyond C2. No data modification beyond the normal OTP-challenge row this login creates. No port exposure.

---

## Phase D — Final security, health, persistence, and rollback verification

### D1. Confirm Postgres/Redis remain unreachable from outside the VM

1. **Working directory:** VM shell
2. **Command:** `sudo ss -tlnp | grep -E ':(5432|6379)\b' || echo NO_PUBLIC_DB_PORTS`
3. **Config variables:** none.
4. **Verify success:** output is `NO_PUBLIC_DB_PORTS`.
5. **Expected / failure modes:** if anything *is* listening on 5432/6379, it is not this Compose file's doing — investigate as a possible unrelated service before proceeding further.
6. **Rollback:** n/a — read-only check.
7. **Azure Portal required?** No.
8. **Flags:** No.

### D2. Confirm the proxy only listens where Phase A intended

1. **Working directory:** VM shell
2. **Command:** `sudo ss -tlnp | grep -E ":(${PROXY_HTTP_PORT:-80}|${PROXY_HTTPS_PORT:-443})\b"`
3. **Config variables:** same `PROXY_BIND_ADDRESS`/`PROXY_HTTP_PORT`/`PROXY_HTTPS_PORT` as A4.
4. **Verify success:** printed local address matches the intended bind (e.g. `127.0.0.1:18080`), never `0.0.0.0:80`.
5. **Expected / failure modes:** if A4 was skipped, the dangerous `0.0.0.0:80`/`:443` default is live — stop and fix before continuing.
6. **Rollback:** adjust `.env.prod`, then `docker compose ... up -d --force-recreate proxy`.
7. **Azure Portal required?** No.
8. **Flags:** a corrective `--force-recreate proxy` restarts only the proxy container — brief ingress interruption, no data impact.

### D3. Confirm 1Panel and Cloudflare Tunnel are both still functioning unchanged

1. **Working directory:** VM shell
2. **Command:** `systemctl status cloudflared --no-pager | head -5; curl -s -o /dev/null -w '1Panel HTTP: %{http_code}\n' http://127.0.0.1:<1panel-port>` (1Panel's real local port **requires verification** — do not guess)
3. **Config variables:** none.
4. **Verify success:** `cloudflared` shows `active (running)`; 1Panel responds with its normal status code.
5. **Expected / failure modes:** both should be entirely unaffected, since no step in Phases A-C touches either service. Any issue here is unrelated to this deployment and should be investigated as such.
6. **Rollback:** n/a — this runbook never modifies either service.
7. **Azure Portal required?** Possibly, only to re-confirm NSG rules were never changed (they weren't, by design).
8. **Flags:** No.

### D4. Confirm data survived the Phase B image swap

1. **Working directory:** `<REPO>`
2. **Command:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml exec postgres psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c '\dt' | head -20`
3. **Config variables:** `POSTGRES_USER`, `POSTGRES_DB` (already set).
4. **Verify success:** existing application tables are listed, not an empty schema.
5. **Expected / failure modes:** an empty list would mean the pgBackRest image swap somehow mounted a different data directory — **stop immediately and do not take any further backup/restore action until root-caused** if this happens.
6. **Rollback:** n/a — read-only query.
7. **Azure Portal required?** No.
8. **Flags:** No.

### D5. Confirm the rollback path is understood (do not execute unless actually rolling back)

1. **Working directory:** `<REPO>`
2. **Command (reference only — do not run except for a real rollback):** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml down`
3. **Config variables:** none. **Never add `-v` to this command.**
4. **Verify success (when actually used):** `docker compose ... ps` shows no containers; `docker volume ls | grep -E "$POSTGRES_VOLUME|$REDIS_VOLUME"` shows the external volumes still exist, untouched.
5. **Expected / failure modes:** clean stop, zero data loss, since `-v` is never used.
6. **Rollback of the rollback:** `docker compose --env-file deploy/single-vm/.env.prod -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml up -d` brings everything back on the same external volumes.
7. **Azure Portal required?** No.
8. **Flags:** **This command stops all six containers.** Never pass `-v`. Never run `docker system prune`. Never run destructive SQL as part of any rollback in this runbook.

### D6. Confirm SSH/Bastion recovery path is untouched

1. **Working directory:** VM shell (test is to use the path itself)
2. **Command:** exit the current 1Panel-terminal session and reconnect via the normal SSH/Bastion route used before this deployment.
3. **Config variables:** none — procedural check.
4. **Verify success:** successful reconnection.
5. **Expected / failure modes:** unaffected — nothing in this runbook touches SSH/NSG/Bastion configuration.
6. **Rollback:** n/a.
7. **Azure Portal required?** Possibly, only to re-confirm NSG rules were never changed.
8. **Flags:** No.

---

## Open items requiring verification before this runbook is executed

- Real `.env.prod` file path and contents (assumed `deploy/single-vm/.env.prod` above — confirm).
- Real `BACKEND_ENV_FILE` path and contents.
- Real external volume names (`POSTGRES_VOLUME`, `REDIS_VOLUME`, `CADDY_DATA_VOLUME`, `CADDY_CONFIG_VOLUME`).
- Real VM port usage (`ss -tlnp`) before finalizing `PROXY_HTTP_PORT`/`PROXY_HTTPS_PORT`.
- Real Cloudflare Tunnel config location/method for adding the one new ingress rule.
- Real 1Panel local port, for the D3 health check.
- A real test doctor account (not a seeded fixture) for Phase C3.
- Confirmation that `POSTGRES_BACKUP_IMAGE`'s digest is real and published (see the companion release-handoff document, §2).
