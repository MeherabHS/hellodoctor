# Milestone 9 — Audit B: ARM64 Container and Adapter Readiness

**Audit date:** 2026-11-07
**Scope:** `deploy/single-vm/` production Compose topology, `services/backend/Dockerfile`, `apps/admin-web/Dockerfile`, `apps/doctor-web/Dockerfile`, `deploy/single-vm/backend.Dockerfile`, `deploy/single-vm/backup/pgbackrest.Dockerfile`, `deploy/single-vm/compose.yml`, `deploy/single-vm/compose.backup.yml`, `deploy/single-vm/Caddyfile`.
**Method:** Direct file review plus **real local verification performed in this session** — `docker buildx build --platform linux/arm64` against the actual, unmodified Dockerfiles (QEMU emulation, not native ARM64 silicon), followed by actually running each resulting container and executing a real command inside it. This is stronger than a registry-metadata-only check (the prior 2026-11-05 assessment's method) but weaker than native-hardware evidence — every item below says exactly which kind of evidence it has.

This audit is read-only: no file was modified to produce it, except for one pre-existing, already-applied fix from the immediately preceding session (`services/backend/Dockerfile`'s `TARGETARCH` argument — see Audit B §1) and a dated addendum to `docs/UBUNTU_2404_ARM64_DEPLOYMENT_READINESS.md`, both already in the working tree before this audit began. Nothing was built, pushed, or deployed as part of writing this report beyond images already produced in the prior session and still present in this machine's local Docker image cache.

---

## 1. Required images and build stages

| Component | Base image(s) | Build stages | ARM64 evidence |
|---|---|---|---|
| Rust backend | `rust:1-bookworm@sha256:c9155318d07341301a78b45ecdd81af2e43b8b297d64e8fd4c5e80d7735607c3` (builder) → `debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3` (runtime) | 2-stage (`services/backend/Dockerfile`) | **VERIFIED** — full `cargo build --release --bin hellodoctor-backend` completed successfully under `linux/arm64` emulation (includes Axum/Tokio/SQLx/Argon2/Ed25519/Rustls/`aws-lc-rs`). `docker image inspect --format '{{.Architecture}}'` → `arm64`. Running the compiled binary with no config produced real Rust panics from genuine startup-validation code (`src/config.rs:418`, then `:423` with partial config supplied) — a real aarch64 ELF executing real logic, not an exec-format error. |
| Backend production wrapper | `${BACKEND_IMAGE}` (the image above) | 1-stage, adds `curl` only (`deploy/single-vm/backend.Dockerfile:1-6`) | Not independently rebuilt in this session (trivial, low-risk — single `apt-get install curl` on an already-verified base); CI workflow (`arm64-compatibility.yml`) builds and architecture-checks it. |
| Admin Web | `node@sha256:4b150b48b4116e2a22f2c39cc27025fbf4f153e2078fd3b14d159fdb5c7279da` (`node:22-bookworm-slim`) for both build and runtime stages | 2-stage, `output: 'standalone'` (`apps/admin-web/Dockerfile`) | **VERIFIED** — built clean; `docker image inspect` → `arm64`; `docker run --rm --entrypoint node <image> -p 'process.arch'` executed inside the emulated container and printed `arm64`. |
| Doctor Web | Same `node:22-bookworm-slim` digest | 2-stage, identical shape (`apps/doctor-web/Dockerfile`) | **VERIFIED** — identical method and result to Admin Web. |
| PostgreSQL | `postgres:16-bookworm@sha256:e8a07d2c7579b038938cdba6c539e580e5001caf9ea203d397b5cc83e76bd938` | n/a (official image, used directly) | **PARTIALLY VERIFIED** — registry manifest confirms a `linux/arm64/v8` child (`docker buildx imagetools inspect`, both this session and the 2026-11-05 assessment). The image itself was not pulled and run under emulation in this session (no new information beyond the prior metadata check). |
| Redis | `redis:7-alpine@sha256:8da19c7399dca900c6822a1cf30f9d2285585e2c4b457ff4095b0f4500054d3f` | n/a | **PARTIALLY VERIFIED** — same as Postgres: registry metadata confirms `linux/arm64/v8`; not pulled/run this session. |
| Caddy (proxy) | `caddy:2.11.7-alpine@sha256:e70b164da81ef698b3ad19af799ea6086ce6d9e48ed4079eb9fe26f6fbf14ee5` | n/a | **PARTIALLY VERIFIED** — registry metadata only; this session independently confirmed `caddy:2-alpine`'s generic tag also has a `linux/arm64/v8` child (not the exact pinned digest above, which was only checked via registry metadata, not pulled). |
| pgBackRest/backup | `postgres:16-bookworm@sha256:e8a07d2c7579b038938cdba6c539e580e5001caf9ea203d397b5cc83e76bd938` + `apt-get install pgbackrest=2.60.0-1.pgdg12+1 ca-certificates cron python3 curl jq` (`deploy/single-vm/backup/pgbackrest.Dockerfile`) | 1-stage | **VERIFIED** — built clean under `linux/arm64` emulation; apt resolved and installed the pinned `pgbackrest` version from Debian's own `arm64` PGDG repository (confirmed live via `apt-cache policy` in a separate emulated container before the full build). Running the built image: `dpkg --print-architecture` → `arm64`; `pgbackrest version` → `pgBackRest 2.60.0` (real executable output, not a manifest check). |

**Finding, Audit B §1 (carried forward from the immediately preceding session, documented here for completeness):** the first `linux/arm64` build of `services/backend/Dockerfile` failed instantly with `unexpected Rust host aarch64-unknown-linux-gnu for TARGETARCH=amd64`. Root cause: `ARG TARGETARCH=amd64` was declared *after* `FROM` with a hardcoded default, which (per Docker's documented ARG-scoping behavior) overrides Buildx's automatic per-platform value instead of inheriting it — and the exact same invocation shape is what `.github/workflows/arm64-compatibility.yml`'s `rust-backend-arm64` job uses, with no extra `--build-arg`. **This was fixed in the prior session** (bare `ARG TARGETARCH`, no default) and verified for both `linux/arm64` (matches the real CI invocation exactly) and native `linux/amd64` (no regression). The fix is already present in the working tree as of this audit. No other Dockerfile in this repository exhibited this pattern.

---

## 2. Build and runtime resource requirements

Per `deploy/single-vm/compose.yml`, aggregated against the fixed 4 vCPU / 8 GiB target:

| Service | Memory limit | CPU shares (relative, same `cpuset: 0-3` pool) |
|---|---:|---:|
| PostgreSQL | 3072 MiB (`compose.yml:42`) | 3000 (`compose.yml:44`) |
| Redis | 384 MiB (`compose.yml:58`) | 500 (`compose.yml:60`) |
| Backend | 1536 MiB (`compose.yml:95`) | 6000 (`compose.yml:97`) |
| Admin Web | 448 MiB (`compose.yml:121`) | 750 (`compose.yml:123`) |
| Doctor Web | 384 MiB (`compose.yml:136`) | 750 (`compose.yml:138`) |
| Caddy | 128 MiB (`compose.yml:159`) | 500 (`compose.yml:161`) |
| **Application total** | **5952 MiB (5.8125 GiB)** | shared 4-core pool |

Against 8 GiB, this leaves ~2.1875 GiB for the Ubuntu OS, Docker daemon, page cache, and (once activated) the separate `backup` container (256 MiB, `compose.backup.yml:114`) and the `postgres` service's `shm_size: 256m` (`compose.yml:45`). **Status: VERIFIED as a configuration calculation only.** No measurement of actual memory/CPU pressure under real or simulated load on ARM64 hardware exists — this is the same gap the 2026-11-05 assessment already identified, unchanged by this session's emulated builds (emulation does not produce meaningful resource-pressure data; QEMU's own overhead makes any such measurement unrepresentative of real silicon).

CPU budget note: all six services share one `cpuset: "0-3"` pool (`compose.yml:9`, inherited via the `x-common` anchor) — there is no per-service CPU pinning beyond relative `cpu_shares`, which only matters once all four cores are saturated. This is an explicit, documented design choice (`compose.yml:1-2`), not an oversight.

---

## 3. Container security and privileges

All six services inherit `security_opt: ["no-new-privileges:true"]` from the shared `x-common` anchor (`compose.yml:4-13`). Beyond that baseline:

| Service | `read_only` | `cap_drop` | Runs as | Notes |
|---|---|---|---|---|
| Backend | `true` (`compose.yml:89`) | `[ALL]` (`compose.yml:90`) | `hellodoctor:hellodoctor`, uid/gid 10001 (`services/backend/Dockerfile:44-46,50`) | `tmpfs` at `/tmp` only (`compose.yml:91`). |
| Admin Web / Doctor Web | `true` (inherited via `x-web` anchor, `compose.yml:16`) | `[ALL]` (`compose.yml:17`) | `node` (standard Node image user) (`apps/admin-web/Dockerfile:19`, `apps/doctor-web/Dockerfile:19`) | `tmpfs` at `/tmp` and `/app/.next/cache` (`compose.yml:18`). |
| Caddy | `true` (`compose.yml:155`) | not explicitly dropped at the `proxy` service level (only the `x-common` baseline applies) | default Caddy image user | `tmpfs` at `/tmp` (`compose.yml:156`). **Minor inconsistency**: unlike backend/web, `proxy` has no explicit `cap_drop: [ALL]` of its own — it may need `NET_BIND_SERVICE` or similar to bind 80/443 as non-root depending on the base image's own user, so this may be intentional rather than an oversight, but it is not explicitly justified in a comment the way other choices in this file are. **Verification needed**, not yet confirmed which capabilities the Caddy image's default user actually requires. |
| PostgreSQL / Redis | not `read_only`, no `cap_drop` override | — | official image defaults | These two are the only services without the hardened `x-web`/explicit backend treatment — reasonable, since they need to write their own data directories, but this means they're the least-hardened containers in the stack. Not flagged as a defect (databases legitimately need write access to their data volume), but worth an operator's explicit acknowledgment. |

`deploy/single-vm/backend.Dockerfile:3,5` uses `USER root` transiently (to `apt-get install curl` for the healthcheck) and switches back to `hellodoctor:hellodoctor` before the image's final layer — correct practice, the running container never executes as root.

The `backup` container (`compose.backup.yml:118`) also carries `security_opt: [no-new-privileges:true]` independently (it is not part of the main `compose.yml` stack, so it does not inherit the `x-common` anchor) — consistent treatment.

---

## 4. Ports and internal networking

- `app` network: static IPs (`172.30.84.10-13`), Caddy at `.10`, Doctor Web at `.12`, Admin Web at `.11`, Backend at `.13` (`compose.yml:92-93,118-119,134-135,157-158,171-174`).
- `data` network: `internal: true` (`compose.yml:175-176`) — **PostgreSQL and Redis have no published host ports anywhere in `compose.yml`** (confirmed by absence of any `ports:` key under either service). Only Caddy publishes `80`/`443` to the host (`compose.yml:151-153`), bound to a configurable address defaulting to `0.0.0.0`.
- Backend is on *both* `app` (to be reachable by Caddy) and `data` (to reach Postgres/Redis) (`compose.yml:92-94`) — correctly scoped. Admin Web is also on both `app` and `data` (`compose.yml:118-120`), consistent with its `REDIS_URL: redis://redis:6379` env var (`compose.yml:116`) — correctly wired, re-verified by direct re-read after an initial misreading during drafting of this audit. Doctor Web is only on `app` (`compose.yml:134-135`), consistent with it having no `REDIS_URL` override in its own environment block — no Redis dependency is declared for it, so omitting `data` network membership is correct, not an oversight.

**Status: VERIFIED by static Compose review** (`docker compose config --quiet` was run successfully against this file in the 2026-11-05 assessment with placeholder values; not re-run in this session). The Admin Web/`data` network finding above is a **static-review finding, not yet runtime-confirmed** — a real startup attempt would either fail fast (if Admin Web's code requires Redis at boot) or silently never use it (if Redis is optional/unused by Admin Web), and only a real six-service startup test can distinguish the two.

---

## 5. Persistent volumes

All production data volumes are declared `external: true` (`compose.yml:181-192`): `pgdata`, `redisdata`, `caddydata`, `caddyconfig`, each requiring an operator-supplied, pre-provisioned volume name via `${..._VOLUME:?...}`. This is a deliberate safety property — Compose cannot silently create (and on `down -v`, destroy) these volumes; an operator must have explicitly provisioned and named them first. The backup-specific volumes (`pgbackrest-repo`, `-config`, `-logs`, `-socket`) follow the same `external: true` pattern in `compose.backup.yml:122-133`; only the WAL spool volume is non-external (`compose.backup.yml:140-141`), with an explicit, documented rationale (spooled-but-unpushed WAL is PostgreSQL-recoverable, so it doesn't need the same operator-provisioning friction).

**Status: VERIFIED by static review.** No volume was actually created or mounted under a real container in this session (the emulated builds/runs did not use the `deploy/single-vm/` compose files at all — they ran each image standalone, with no volumes attached). A real six-service compose startup is required to confirm the external-volume contract works as intended end to end (correct ownership/permissions on first mount, no silent auto-creation).

---

## 6. Secrets and configuration

Every secret-shaped value in `compose.yml` uses Compose's `${VAR:?message}` syntax (fails the whole `compose config`/`up` invocation loudly if unset) — confirmed for: `WEB_PROXY_SECRET`, `TURNSTILE_SITE_KEY`, `TURNSTILE_SECRET_KEY`, `WEB_AUTH_ASSERTION_SECRET` (Admin Web only), `POSTGRES_PASSWORD`, `BACKEND_RUNTIME_IMAGE`/`ADMIN_WEB_IMAGE`/`DOCTOR_WEB_IMAGE` (image references, not credentials, but same fail-loud treatment), `BACKEND_ENV_FILE`, `ACME_EMAIL`, `ERASURE_JOURNAL_DIR`, every `*_VOLUME` name, and the four `${API_HOST}`/`${ADMIN_HOST}`/`${DOCTOR_HOST}` hostnames. The backend's own application secrets (JWT keys, webhook secrets, DB role passwords, Azure/FCM/Agora credentials) are **not** inlined in `compose.yml` at all — they're required to live in a separate, operator-managed file referenced by `${BACKEND_ENV_FILE:?...}` (`compose.yml:70`), kept outside the repository.

The backup stack's cipher-key handling is more nuanced (`compose.backup.yml:14-20,93-97`): both a plain env var (`PGBACKREST_REPO1_CIPHER_PASS`) and a file-based variant (`PGBACKREST_REPO1_CIPHER_PASS_FILE`) are supported, with an explicit comment noting the file variant is preferred because it never appears in `docker inspect`/`docker exec ... env` output the way a plain env var does. Enforcement that *one* of the two is actually set is done inside `backup-entrypoint.sh`/`postgres-entrypoint.sh`, not at the Compose level (Compose cannot express "one of these two" as a single `:?` guard) — see Audit C for whether that enforcement is actually implemented correctly.

**Status: VERIFIED by static review** that the fail-loud contract exists at the Compose layer for everything Compose itself can guard; **NOT TESTED** whether the shell-script-level "one of two" enforcement for the cipher pass actually works (that belongs to Audit C).

---

## 7. Health and readiness checks

Every one of the six services has a `healthcheck:` block, and `depends_on` conditions chain correctly: `postgres`/`redis` → `backend` (`compose.yml:105-107`) → `admin-web`/`doctor-web` (`compose.yml:124-125,139-140`) → `proxy` (`compose.yml:167-169`). Backend's healthcheck (`compose.yml:99-104`) does a real TCP+HTTP `GET /healthz` via `/dev/tcp` bash redirection, checking for a literal `200 OK` substring — this is the same `/healthz` endpoint documented elsewhere in this repo as verifying live migration state (not just "process is up"), so a healthy backend container is a reasonably strong signal. Admin/Doctor Web healthchecks are a bare TCP-connect check on port 3000 (`apps/admin-web/Dockerfile:22-23`, `apps/doctor-web/Dockerfile:22-23`) — this proves the Node process is listening, not that the app is actually serving correct responses (their own Dockerfile comment says as much: "Full ingress/auth checks are performed by the verification harness").

**Status: VERIFIED by static review that the healthcheck definitions exist and are structurally sound.** None of the six services' healthchecks were exercised against a real running container with real dependencies in this session — the standalone image runs performed here (see §1) used no healthcheck orchestration at all (bare `docker run`, no Compose). A real Compose startup is required to confirm the `depends_on: condition: service_healthy` chain actually gates startup order correctly end to end.

---

## 8. Database migration strategy

No separate migration step or container exists anywhere in `deploy/single-vm/`. Per this project's own established, documented convention (`CLAUDE.md`'s "Migration discipline" section, and confirmed by this session's own direct use of `sqlx migrate run`/the backend's embedded `sqlx::migrate!` macro), migrations are applied automatically, exactly once, the moment the backend binary's process starts — so in this containerized topology, a fresh `backend` container restart is precisely equivalent to the "new migration file on disk → must restart the process" rule this project already enforces for bare-metal dev. This is actually a *safer* property in the containerized world than on a long-running bare-metal dev server: a container restart (which Compose's `depends_on`/health-check-gated startup order already requires whenever the backend image is replaced) is exactly the event that makes new migrations take effect — there is no way to "forget" to restart, the way the bare-metal incident (`docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md` §3, referenced in `CLAUDE.md`) describes.

**Status: VERIFIED by design/code-path review.** Not independently re-run under ARM64 emulation in this session (the standalone backend container run in §1 had no database configured at all, by design, to isolate the TARGETARCH/architecture question — it never reached the migration step). A real Postgres container is required to observe this end to end on ARM64.

---

## 9. Logging and rotation

Every service inherits `logging: {driver: json-file, options: {max-size: "10m", max-file: "3"}}` from the `x-common` anchor (`compose.yml:11-13`) — a bounded 30 MiB ceiling per service's local log files, consistent across all six. The existing readiness doc (`docs/UBUNTU_2404_ARM64_DEPLOYMENT_READINESS.md`, host-hardening §6) separately recommends monitoring `/var/lib/docker` independent of this per-container cap, since `json-file` rotation bounds each container's own logs but not the Docker daemon's own disk usage in aggregate.

**Status: VERIFIED by static review.** No log volume or rotation behavior was exercised under real sustained load in this session.

---

## 10. Startup and shutdown behavior

All six services use `restart: unless-stopped` (via `x-common`, `compose.yml:5`) and the backend additionally sets `stop_grace_period: 30s` (`compose.yml:98`) — giving in-flight requests/connections a window to drain before SIGKILL. No other service overrides the default (Compose's own default grace period, 10s). Given the backend is the only service with meaningfully long-lived state (open HTTP connections, background scheduler tasks including the new Phase 1B transactional-email dispatch worker and the existing push-notification delivery loop), this asymmetry seems appropriate but was not stress-tested.

**Status: NOT TESTED.** No restart-under-load or graceful-shutdown-timing test was performed, under emulation or otherwise, in this session or the prior one.

---

## 11. Backup compatibility

`deploy/single-vm/compose.backup.yml` is a Compose *overlay* (same `name: hellodoctor-vm` project, `compose.backup.yml:1`), not a separate stack — it replaces the plain `postgres` service's image/entrypoint/command with a pgBackRest-capable variant (`${POSTGRES_BACKUP_IMAGE}`, built from the same `pgbackrest.Dockerfile` the standalone `backup` service also uses) and adds `archive_mode=on`/`archive_command=pgbackrest ... archive-push %p` to the Postgres command line (`compose.backup.yml:44`). This is architecturally sound — the same image serves both "the database engine with WAL archiving wired in" and "the separate scheduler container that runs backup/check/monitor passes" — and it correctly shares `pgdata` with the main stack's volume while adding backup-specific volumes (`compose.backup.yml:46-49,107-112`).

**Status: VERIFIED by static review that the container/volume/network wiring is internally consistent** (same volume names as `compose.yml`'s `pgdata`, correctly placed on the `data` network, correct `depends_on: service_healthy` on `postgres`). The correctness of the backup/restore *logic itself* (encryption, retention, Key Vault integration, real Azure Blob access) is Audit C's scope, not re-verified here.

---

## 12. Failure and rollback procedures

Already fully documented in `docs/UBUNTU_2404_ARM64_DEPLOYMENT_READINESS.md` ("Rollback procedure" section): keep previous digest-pinned images outside the release directory, re-point the `${..._IMAGE}` env vars to them, `config --quiet` then `up -d`, verify health, and a separate database-schema-rollback procedure for migration-involving releases. This is a **procedure**, not an automated mechanism — it depends entirely on an operator having retained the previous digests and following the documented steps correctly.

**Status: PASS (procedure documented); NOT TESTED (no rollback has ever been executed, simulated or real).**

---

## Summary classification

| Area | Status |
|---|---|
| Backend image build (ARM64) | **VERIFIED** (real emulated build + real execution) |
| Admin Web / Doctor Web image build (ARM64) | **VERIFIED** (real emulated build + real execution) |
| pgBackRest image build (ARM64) | **VERIFIED** (real emulated build + real execution) |
| PostgreSQL / Redis / Caddy ARM64 compatibility | **PARTIALLY VERIFIED** (registry metadata only, not pulled/run) |
| Resource budget | **VERIFIED** (calculation only; no real/ARM64 load measurement) |
| Container security/privileges | **VERIFIED** (static review); one inconsistency flagged (Caddy's `cap_drop`) |
| Networking | **VERIFIED** (static review) |
| Persistent volumes | **VERIFIED** (static review only) |
| Secrets/configuration | **VERIFIED** (Compose-level fail-loud contract); cipher-pass dual-source enforcement is Audit C's to confirm |
| Health/readiness checks | **VERIFIED** (static review only; never exercised together) |
| Migration strategy | **VERIFIED** (design review; matches existing project convention) |
| Logging/rotation | **VERIFIED** (static review only) |
| Startup/shutdown | **BLOCKED** (not tested) |
| Backup container integration | **VERIFIED** (static review of wiring only) |
| Rollback procedure | **PASS** (documented); **BLOCKED** (never executed) |

## Findings requiring action before the readiness matrix is finalized

1. **Caddy's capability drop is inconsistent with the other application-tier services** (`compose.yml`'s `x-web` anchor drops `ALL` for Admin/Doctor Web and the backend does the same explicitly; `proxy` only inherits the common `no-new-privileges` baseline). Needs an explicit decision and comment, not necessarily a change — Caddy binding to privileged-adjacent ports 80/443 as a non-root user may genuinely need a capability the others don't.
2. **No six-service integration test has ever been run under ARM64** (emulated or native) — every "VERIFIED" item above is either a per-image build/run check or a static Compose-file review. `deploy/single-vm/compose.local-verification.yml` exists specifically for this and is the correct next artifact to exercise.
