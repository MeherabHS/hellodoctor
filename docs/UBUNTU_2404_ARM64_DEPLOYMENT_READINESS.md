# HelloDoctor.Asia Ubuntu 24.04 ARM64 deployment readiness

**Assessment date:** 2026-11-05
**Fixed target:** Microsoft Azure, South Africa North, `Standard_D4pls_v6`, ARM64/AArch64, Ubuntu Server 24.04 LTS Generation 2, 4 vCPU, 8 GiB RAM, existing six-service Compose deployment.

Microsoft documents Dplsv6 as Azure Cobalt 100 ARM64 with 2 GiB per vCPU and lists `Standard_D4pls_v6` at 4 vCPU/8 GiB: [Dplsv6 sizes](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/general-purpose/dplsv6-series). Microsoft’s region list marks South Africa North as a restricted-access region, so subscription access and the exact SKU must still be confirmed with `az vm list-skus --location southafricanorth --resource-type virtualMachines` before an operator schedules the internal deployment: [Azure regions](https://learn.microsoft.com/en-us/azure/reliability/regions-list).

## Decision

The repository is **prepared for a controlled internal ARM64 deployment**, but final native-runtime acceptance is **BLOCKED** until the ARM64 Buildx workflow runs on a native Linux ARM64 runner. This Windows x64 workstation has no running Docker Linux daemon, so no ARM64 container build or runtime test was represented as verified.

**See the 2026-11-07 addendum at the end of this document** — a later session, on a workstation with a working Docker daemon and Buildx/QEMU `linux/arm64` emulation, closed most of the "NOT TESTED" image-build/runtime gates below with real (though emulated, not native-silicon) evidence, and found and fixed one real build bug in `services/backend/Dockerfile`. The native-runtime acceptance gate stated above is otherwise unchanged: emulated evidence is not native ARM64 hardware evidence, and nothing was deployed, committed, or pushed.

No unresolved architecture-specific issue is marked PASS solely from x86 execution. The remaining blockers are evidence gates: native image builds, six-service startup, migration/RLS checks, restart persistence, HTTPS/authentication, and ARM64 backup/PITR.

## Validation branch state

The working tree is now on `test/ubuntu-arm64-validation`. It contains the existing uncommitted Milestone 8 work unchanged. There are no staged files, no commit, no push, no image publication, and no workflow run. The ARM64 publication candidate is a selective 26-file set; the broader working tree contains 324 existing changes and must not be swept into an ARM64 commit.

## Changes implemented

| Change | Status | Evidence |
|---|---|---|
| Backend Rust builder moved from an amd64 child digest to the reviewed `rust:1-bookworm` OCI manifest digest | PASS | `services/backend/Dockerfile` now pins `sha256:c9155318d07341301a78b45ecdd81af2e43b8b297d64e8fd4c5e80d7735607c3`; registry inspection showed a Linux ARM64 child. |
| Backend build rejects an unexpected Rust host for `TARGETARCH` | PASS | `services/backend/Dockerfile` checks `aarch64-unknown-linux-gnu` for ARM64 and `x86_64-unknown-linux-gnu` for amd64. |
| Direct backend `reqwest` path uses Rustls instead of the default native-tls/OpenSSL path | PASS | `services/backend/Cargo.toml` and `Cargo.lock`; `cargo tree --locked -e features -i reqwest:0.12.28` shows Rustls features. This removes the direct Linux OpenSSL build dependency while retaining `aws-lc-rs` through `object_store`. |
| Admin/Doctor Docker builds retain optional ARM64 SWC and Sharp packages | PASS | Both web Dockerfiles use `npm ci --include=optional`; both lockfiles contain Linux ARM64 SWC and Sharp/libvips packages. |
| Production Compose pins official multi-platform Postgres/Redis/Caddy manifests and forces ARM64 | PASS | `deploy/single-vm/compose.yml` has `platform: linux/arm64` and reviewed defaults. |
| pgBackRest image uses the reviewed PostgreSQL 16 Bookworm manifest | PASS | `deploy/single-vm/backup/pgbackrest.Dockerfile` pins `postgres:16-bookworm@sha256:e8a07d2c...`. |
| ARM64 image environment contract | PASS | `deploy/single-vm/arm64-images.env.example` requires digest-pinned application and backup artifacts; mutable tags are not accepted as production values. |
| Native ARM64 CI build | PASS (workflow prepared) / NOT TESTED (execution) | `.github/workflows/arm64-compatibility.yml` targets `ubuntu-24.04-arm`, runs native Rust unit/security tests, builds the backend, loads it into the native runner daemon for the production wrapper, builds both web images and the backup image with `--platform linux/arm64`, and checks image architecture and native dependencies. |
| Ubuntu host preflight | PASS (script prepared) / NOT TESTED (execution) | `deploy/single-vm/verify-ubuntu-arm64-host.sh` is read-only and checks AArch64, Ubuntu 24.04, Docker Engine, Compose, Buildx, Docker architecture, and free disk. |

No clinical, payment, identity, authorization, RLS, migration, or business-logic source was changed.
No Azure resource was provisioned, no paid API or provider operation was called, and no Git staging, commit, push, database deletion, or production deployment was performed.

## Registry compatibility evidence

`docker buildx imagetools inspect` was run from the Windows client without a Docker daemon. The following manifest-list digests were observed on 2026-11-05, and each output contained a `linux/arm64/v8` child:

| Image | Manifest digest used | ARM64 child observed |
|---|---|---|
| `rust:1-bookworm` | `sha256:c9155318d07341301a78b45ecdd81af2e43b8b297d64e8fd4c5e80d7735607c3` | `sha256:bfd089cebae971d14a19f56b8a755838c616341c5d73df368305521189fcbb1e` |
| `debian:bookworm-slim` | `sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3` | `sha256:7ccb8b5b60c7654f5b795d94ee7d751cd0811500803f0b09cefcd555e755a55a` |
| `node:22-bookworm-slim` | `sha256:4b150b48b4116e2a22f2c39cc27025fbf4f153e2078fd3b14d159fdb5c7279da` | `sha256:9a422d62743ffb1e6eb282549f4eae7ab89f7b926db1f136f4cba8be60e4ecbc` |
| `postgres:16-bookworm` | `sha256:e8a07d2c7579b038938cdba6c539e580e5001caf9ea203d397b5cc83e76bd938` | `sha256:c7d8cf162cb365b478761fa957ff4e58c2cd0ee748c6084056955de0dda9abf4` |
| `redis:7-alpine` | `sha256:8da19c7399dca900c6822a1cf30f9d2285585e2c4b457ff4095b0f4500054d3f` | `sha256:c1945f0eeb2d503f8421952da953a3804d38e4f004d4ef11d57826da59d2c332` |
| `caddy:2.11.7-alpine` | `sha256:e70b164da81ef698b3ad19af799ea6086ce6d9e48ed4079eb9fe26f6fbf14ee5` | `sha256:cc447f149425221422153c6cdf33a123b3f458839679f950298f68a690a54a5b` |

These are registry metadata checks, not image builds or runtime checks.

## Compatibility status by service

| Service or surface | Status | Finding |
|---|---|---|
| Rust backend | PASS for source/dependency portability; NOT TESTED natively | Axum, Tokio, SQLx/PostgreSQL, Argon2, Ed25519, AES-GCM, hashing, and compression are portable. The direct HTTP client now uses Rustls. `aws-lc-rs` remains through Azure/AWS object storage and requires the ARM build job. |
| PostgreSQL 16 | PASS for image metadata; NOT TESTED runtime | Official PostgreSQL 16 Bookworm manifest has an ARM64 child. `pg_stat_statements` is part of the image. Migration application, role grants, RLS, and query behavior remain unverified on ARM64. |
| Redis 7 | PASS for image metadata; NOT TESTED runtime | Official Redis 7 Alpine manifest has an ARM64 child. AOF configuration is unchanged. Persistence and restart recovery remain unverified on ARM64. |
| Admin Web | PASS for lockfile/Dockerfile portability; NOT TESTED build/runtime | Next.js SWC ARM64 and Sharp/libvips ARM64 entries are locked, and optional packages are retained by `npm ci`. Native build, image optimization, login, refresh, and cookie behavior require the ARM runner. |
| Doctor Web | PASS for lockfile/Dockerfile portability; NOT TESTED build/runtime | Same ARM64 SWC/Sharp treatment as Admin Web. Native build, authentication, refresh, and proxy behavior require the ARM runner. |
| Caddy | PASS for image/config portability; NOT TESTED HTTPS | Caddy ARM64 manifest is verified. Existing TLS, host routing, forwarded-header rewriting, and liveness configuration were preserved. Certificate issuance and secure-cookie behavior require a Linux HTTPS test. |
| pgBackRest | PASS for source/image preparation; BLOCKED for acceptance | Shell scripts use Linux tools (`bash`, `cron`, `flock`, `runuser`, `pgbackrest`) and the PostgreSQL base image is ARM-capable. ARM package installation, encrypted backup, WAL archive, Azure Blob repository, retention, and PITR restore were not run. |
| Compose networking | PASS static | `app` is isolated from `data`; `data` is `internal: true`; PostgreSQL and Redis have no host port mappings; only Caddy publishes 80/443. All production services resolve `platform: linux/arm64`. |
| Compose resource controls | PASS static; NOT TESTED under target host pressure | Existing limits and CPU shares were preserved. The ARM host’s actual memory pressure, swap, disk, and Docker overhead remain unmeasured. |
| Azure Blob contract tests | NOT TESTED | Rust contract tests exist, but no authorized ARM runtime or Azure credentials were available. No provider call is claimed. |
| Disaster-recovery PowerShell tools | NOT A PRODUCTION ARM BLOCKER | `ops/dr/*.ps1` are Windows verification helpers. Production recovery uses the Linux pgBackRest shell path; the PowerShell tools are not represented as Ubuntu-compatible. |
| GitHub Actions | PASS workflow preparation; NOT TESTED hosted execution | Native ARM runner workflow is present. GitHub runner availability, quotas, Docker service behavior, and build duration must be observed in CI. |

## CI execution preparation

The build workflow is split across four independent native ARM64 jobs because GitHub documents private `ubuntu-24.04-arm` runners as 2 vCPU, 8 GiB RAM, and 14 GiB SSD. Each job checks out the same revision and builds only one image family:

| Job | Work | Limit |
|---|---|---:|
| `rust-backend-arm64` | Native Rust library tests, backend image, production wrapper | 45 minutes |
| `admin-web-arm64` | Admin Web image, Node architecture check | 35 minutes |
| `doctor-web-arm64` | Doctor Web image, Node architecture check | 35 minutes |
| `pgbackrest-arm64` | PostgreSQL/pgBackRest image, Debian architecture and pgBackRest check | 35 minutes |

This prevents the large Rust, Node/SWC/Sharp, and PostgreSQL/pgBackRest layers from accumulating on one 14 GiB runner. The jobs use `contents: read`, contain no application or production credentials, and use only synthetic build checks. They do not push images.

Every job records `git rev-parse HEAD` and requires a clean checkout immediately after `actions/checkout`. Pull request runs validate the workflow’s event revision; a manual dispatch validates the revision on the default branch. This prevents a stale runner workspace or an older committed application revision from being mistaken for the current source.

The disposable integration path is separate in `.github/workflows/arm64-disposable-integration.yml`. It accepts reviewed digest-pinned ARM64 application image references as manual inputs, writes a synthetic backend environment under `/tmp`, and uses `deploy/single-vm/compose.arm64-ci.yml`. That overlay changes CPU affinity to `0-1` for the two-vCPU private ARM runner, loopback ports, and CI image names. The existing memory limits, networks, read-only roots, dropped capabilities, `no-new-privileges`, health checks, and application settings remain inherited. Compose configuration was rendered successfully on this workstation with synthetic values.

The integration workflow is deliberately separate from image building because local Docker images do not survive between hosted runners. It requires application images to have been published to a registry or supplied through an approved image transport. Registry authentication, if needed, must be a narrowly scoped CI package credential; no production runtime secret belongs in Actions.

## Publication and default-branch prerequisites

The required workflow publication set is currently uncommitted. GitHub will not expose `workflow_dispatch` from a workflow that exists only in an unpushed working tree; the workflow file must be present on the repository’s default branch before manual dispatch is available. Publishing also requires the repository to have ARM64 hosted runner access and an Actions plan that permits the `ubuntu-24.04-arm` runner.

The exact files required for the ARM64 CI path are:

- `.github/workflows/arm64-compatibility.yml`
- `.github/workflows/arm64-disposable-integration.yml`
- `docs/ARM64_GHCR_PUBLICATION_HANDOFF.md`
- `services/backend/Dockerfile`
- `services/backend/Cargo.toml`
- `services/backend/Cargo.lock`
- `apps/admin-web/Dockerfile`
- `apps/admin-web/package.json`
- `apps/admin-web/package-lock.json`
- `apps/doctor-web/Dockerfile`
- `apps/doctor-web/package.json`
- `apps/doctor-web/package-lock.json`
- `deploy/single-vm/backend.Dockerfile`
- `deploy/single-vm/compose.yml`
- `deploy/single-vm/compose.local-verification.yml`
- `deploy/single-vm/compose.arm64-ci.yml`
- `deploy/single-vm/arm64-images.env.example`
- `deploy/single-vm/verify-ubuntu-arm64-host.sh`
- `deploy/single-vm/Caddyfile.local-verification`
- `deploy/single-vm/backup/pgbackrest.Dockerfile`
- `deploy/single-vm/backup/backup-entrypoint.sh`
- `deploy/single-vm/backup/backup-monitor.sh`
- `deploy/single-vm/backup/backup-runner.sh`
- `deploy/single-vm/backup/alert-notify.sh`
- `deploy/single-vm/backup/postgres-entrypoint.sh`
- `deploy/docker-compose.vm.example.yml`

The existing uncommitted changes in those files must be included together so the workflow builds the same ARM64-capable source it validates. The repository also contains many unrelated uncommitted application and documentation changes; they should not be included in the ARM64 publication unless they are part of the separately reviewed source revision. No staging, commit, push, workflow dispatch, image push, or billable Actions run has occurred.

`services/backend/Cargo.toml`, `services/backend/Cargo.lock`, `apps/admin-web/package-lock.json`, `apps/doctor-web/package.json`, and `apps/doctor-web/package-lock.json` already contain broader uncommitted dependency or application changes. They are build inputs, so the CI revision must include a reviewed final form of them, but the ARM64 branch must not be committed by blindly sweeping every current hunk into one commit.

Exclude `deploy/single-vm/.env.m8-final-local`, `deploy/single-vm/compose.pool-experiment.yml`, and `deploy/single-vm/Caddyfile.local-verification.orig-backup` from publication. They are local verification artifacts or historical backups and are not required by the ARM64 build or integration workflow.

The publication sequence is:

1. Review the exact file list and exclude unrelated working-tree changes.
2. Publish the ARM64 Docker/Compose changes and both workflow files to the default branch through the separately authorized Git process.
3. Confirm Actions sees `ubuntu-24.04-arm`; manually dispatch `Native Ubuntu ARM64 image compatibility`.
4. Review all four build jobs, image architecture checks, native Rust test results, disk/time usage, and the pre-existing `cargo fmt --check` issue.
5. Publish reviewed digest-pinned application images through an explicitly authorized registry step.
6. Dispatch `Disposable Ubuntu ARM64 six-service integration` with those image digests.
7. Retain logs and synthetic test evidence. Only after the integration and separate pgBackRest/PITR checks pass should the internal deployment gate be reconsidered.

## Tests and verification performed

| Check | Result | Limitation |
|---|---|---|
| `cargo check --all-targets --offline` | PASS | Ran on Windows x64 only; not an ARM binary build. |
| `cargo check --all-targets --locked --offline` | PASS | Confirms the committed lockfile matches the Rustls feature change. |
| `cargo test --lib --offline` | PASS — 135 passed, 0 failed | Ran on Windows x64; exercises application unit/security logic but not ARM binaries or provider/runtime state. |
| `cargo tree --locked -e features -i reqwest:0.12.28` | PASS | Confirms Rustls feature selection; does not compile ARM code. |
| `cargo tree --locked -e features -i aws-lc-rs` | PASS | Confirms `aws-lc-rs` remains through `object_store`/Rustls; ARM compilation still required. |
| `cargo tree --locked -i openssl --target aarch64-unknown-linux-gnu` | PASS | No active `openssl` package is present in the ARM64 target graph after the Rustls change; `aws-lc-rs` remains required for object storage. |
| `cargo fmt --all -- --check` | NOT PASS (pre-existing) | Reports formatting differences in existing `examples/web_workload.rs` and `src/telemetry/mod.rs`; no Rust source formatting was changed for this portability work. |
| `npm ci --ignore-scripts --dry-run --include=optional` (Admin) | PASS | Lockfile/install contract accepted on Windows x64; native ARM SWC/Sharp execution remains untested. |
| Same command (Doctor) | PASS | Lockfile/install contract accepted on Windows x64; native ARM SWC/Sharp execution remains untested. |
| ARM64 workflow YAML parse | PASS | `.github/workflows/arm64-compatibility.yml` and `.github/workflows/arm64-disposable-integration.yml` parsed successfully with PyYAML. |
| `bash -n deploy/single-vm/verify-ubuntu-arm64-host.sh` | NOT TESTED | Bash/WSL is not installed on this Windows workstation; the script must be syntax-checked on Ubuntu ARM64 before use. |
| `docker compose -f deploy/single-vm/compose.yml config --quiet` | PASS | Client-side Compose validation with disposable placeholder values; no containers started. |
| Compose rendered images/platforms/resource limits | PASS | Verified defaults resolve to reviewed manifest digests and `linux/arm64`; no runtime. |
| `docker buildx imagetools inspect` for six base images | PASS | Registry metadata only; Docker daemon unavailable. |
| Backend ARM64 image build | NOT TESTED | Docker Linux daemon unavailable on this workstation. |
| Admin/Doctor ARM64 image builds | NOT TESTED | Same limitation. |
| pgBackRest ARM64 image build | NOT TESTED | Same limitation. |
| Six-service health checks/migrations/RLS/roles | NOT TESTED | Requires native ARM64 Linux runtime and disposable PostgreSQL/Redis. |
| Redis AOF/restart persistence | NOT TESTED | Requires native runtime. |
| Auth/refresh/secure-cookie/Caddy HTTPS | NOT TESTED | Requires native Linux HTTPS path and browser/client smoke test. |
| Azure Blob, FCM, email, Agora, payment providers | NOT TESTED | Provider credentials/device/sandbox evidence is intentionally absent. |
| ARM64 backup/WAL/PITR/restore | NOT TESTED | Requires ARM image, repository, and disposable restore target. |

The mandatory local-correctness, system-consistency, and failure-recovery gates for money, identity, and clinical state are therefore **NOT TESTED** for ARM64. No endpoint success, source review, or x86 test is being used as a substitute.

## Resource-budget validation

The production Compose limits remain:

| Service | Memory limit | CPU placement/weight |
|---|---:|---:|
| PostgreSQL | 3072 MiB | `cpuset 0-3`, shares 3000 |
| Redis | 384 MiB | `cpuset 0-3`, shares 500 |
| Backend | 1536 MiB | `cpuset 0-3`, shares 6000 |
| Admin Web | 448 MiB | `cpuset 0-3`, shares 750 |
| Doctor Web | 384 MiB | `cpuset 0-3`, shares 750 |
| Caddy | 128 MiB | `cpuset 0-3`, shares 500 |
| **Total** | **5952 MiB (5.8125 GiB)** | Shared four-core pool |

Against 8 GiB, the configured container-limit remainder is 2240 MiB (2.1875 GiB) before Ubuntu, Docker, page cache, WAL/backup activity, and filesystem overhead. This is a configuration calculation, not target-VM measurement. The Compose file intentionally retains the existing limits and does not add host-level application services.

## Ubuntu Server 24.04 ARM64 installation and hardening

Use the official Ubuntu Server 24.04 LTS ARM64 Generation 2 image in South Africa North. Microsoft’s Cobalt guest-OS table includes Ubuntu 20.04+ for Cobalt ARM64 VMs: [Cobalt guest OS support](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/cobalt-overview). Confirm after first boot:

```bash
uname -m                         # aarch64
. /etc/os-release && echo "$ID $VERSION_ID $VERSION_CODENAME"
dpkg --print-architecture       # arm64
```

Install only security updates and Docker Engine from Docker’s official Ubuntu apt repository. Docker documents Ubuntu Noble and ARM64 support and installs the Compose/Buildx plugins from the same repository: [Docker Engine on Ubuntu](https://docs.docker.com/engine/install/ubuntu/).

```bash
sudo apt-get update
sudo apt-get full-upgrade -y
sudo apt-get install -y ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc
sudo tee /etc/apt/sources.list.d/docker.sources >/dev/null <<'EOF'
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: noble
Components: stable
Architectures: arm64
Signed-By: /etc/apt/keyrings/docker.asc
EOF
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
sudo systemctl enable --now docker
sudo docker run --rm hello-world
docker compose version
docker buildx version
```

Do not install Docker Desktop, a second reverse proxy, a host PostgreSQL/Redis service, GUI packages, or development toolchains. If a non-root deploy account is placed in the `docker` group, treat it as root-equivalent and restrict membership; otherwise run deployment commands through `sudo`.

### Azure and host controls

1. NSG: allow TCP 22 only from the administrator’s fixed source range; allow TCP 80/443 to the public service; deny 5432, 6379, Docker API ports, and all other inbound traffic. Use Azure Bastion or a restricted jump path where available.
2. Host firewall: keep Docker’s `DOCKER-USER` chain policy explicit; Docker-published ports can bypass naïve UFW rules. Do not publish PostgreSQL or Redis from Compose.
3. Apply unattended security updates or a reviewed maintenance schedule. Keep UTC/NTP enabled.
4. Attach managed disks for PostgreSQL, Redis, Caddy state, erasure journal, and the separate pgBackRest repository. Mount them before Compose and verify ownership/permissions; never use the OS disk as the only database/backup copy.
5. Store `/etc/hellodoctor/hellodoctor.env` as root-owned mode 0600. Prefer Docker secret/file mounts for the pgBackRest cipher key. Do not place provider credentials in images, GitHub logs, browser variables, or the committed example file.
6. Configure Docker’s `json-file` rotation or a remote collector. The Compose limit is 10 MiB × 3 per service; monitor `/var/lib/docker` and the database volume separately.
7. Configure Azure Monitor/Log Analytics alerts for VM CPU/memory/disk, Docker/container restarts, PostgreSQL health, Redis health, Caddy certificate expiry, WAL growth, backup staleness, and low free space.
8. Keep Docker/containerd enabled at boot and retain the Compose `restart: unless-stopped` policy; verify the stack comes back after a planned Docker service restart before internal acceptance.

## First controlled deployment procedure

1. Run `deploy/single-vm/verify-ubuntu-arm64-host.sh` as a read-only preflight.
2. Copy the reviewed ARM64 image environment to the private host file and replace every `registry.example.invalid/...REPLACE...` value with a real digest-pinned application/backup artifact produced by the ARM64 CI workflow.
3. Create and permission persistent directories/volumes for PostgreSQL, Redis, Caddy, erasure journal, and the separate backup repository. Do not point the first run at production data.
4. Run `docker compose --env-file /etc/hellodoctor/hellodoctor.env -f deploy/single-vm/compose.yml config --quiet` and retain the rendered configuration for change review.
5. Run the disposable six-service smoke test on the ARM VM: image pulls, health checks, migrations, synthetic auth/refresh, Caddy route checks, PostgreSQL/Redis restart, and record-level persistence.
6. Activate `compose.backup.yml` only after the repository, cipher key custody, Azure Blob permissions, `pgbackrest check`, and a disposable restore have passed.
7. Replace disposable volumes with approved production volumes only after the operator has reviewed the image digests, rendered Compose, secrets, DNS/TLS, NSG, and backup acceptance evidence.
8. Keep the first deployment window observable; do not claim provider delivery, payment execution, or clinical production readiness from this image/host smoke test.

## Rollback procedure

1. Keep the previous known-good image digests and Compose environment file outside the new release directory.
2. Stop application rollout by selecting the previous digest-pinned `BACKEND_RUNTIME_IMAGE`, `ADMIN_WEB_IMAGE`, `DOCTOR_WEB_IMAGE`, `POSTGRES_IMAGE`, `REDIS_IMAGE`, `CADDY_IMAGE`, and backup image. Do not use a mutable tag.
3. Run `docker compose ... config --quiet`, then `docker compose ... up -d` and wait for all health checks.
4. Verify the backend health/migration status, web login/refresh, Caddy routes, PostgreSQL connectivity, Redis AOF state, and audit/financial reconciliation signals.
5. If database schema changes are involved, follow the reviewed migration rollback/recovery procedure; never delete or reset the live database to force a version match.
6. If the VM is lost or the rollback requires data recovery, restore PostgreSQL into a new disposable/ replacement target with pgBackRest/PITR, verify pre/post-boundary markers, then reconnect services only after integrity and authorization checks pass.

## Final acceptance matrix

| Acceptance item | Status |
|---|---|
| ARM64-compatible base image manifests identified and pinned | PASS |
| Rustls/OpenSSL/AWS-LC dependency path prepared | PASS for source/configuration; native ARM compile NOT TESTED |
| Native ARM64 backend/web/backup image builds | BLOCKED — Docker ARM64 runner execution unavailable in this session |
| Six-service ARM64 startup and health checks | NOT TESTED |
| PostgreSQL migrations, role grants, and RLS on ARM64 | NOT TESTED |
| Redis AOF persistence and restart | NOT TESTED |
| Admin/Doctor auth, refresh, secure cookies, and Caddy HTTPS | NOT TESTED |
| Azure Blob contract/provider checks | NOT TESTED |
| ARM64 pgBackRest/WAL/PITR/restore | NOT TESTED |
| 4-vCPU/8-GiB resource budget preserved | PASS configuration calculation; target pressure NOT TESTED |
| Rollback artifacts and procedure documented | PASS procedure; prior ARM production artifact NOT TESTED |
| Controlled internal ARM64 deployment readiness | **BLOCKED pending native ARM64 CI/runtime evidence** |

---

## Addendum — 2026-11-07: emulated build/runtime verification, and one real bug found and fixed

**Environment difference from the 2026-11-05 assessment:** this addendum was produced on a Windows workstation with a *working* Docker Desktop daemon, `docker buildx` with the `linux/arm64` platform registered (QEMU user-mode emulation), and real GNU Bash 5.3 available — none of which the original assessment had. This closes several, but not all, of the gates above with real evidence. **Emulated execution is still not native ARM64 silicon** — nothing here supersedes the "BLOCKED pending native ARM64 CI/runtime evidence" line above; it narrows what that evidence gate still has to cover.

### What was verified, and how

| Item | Previous status | New status | Real evidence obtained |
|---|---|---|---|
| Admin Web image build | NOT TESTED | **PASS — builds under `linux/arm64` emulation** | `docker buildx build --platform linux/arm64 --load` succeeded against the existing, unmodified `apps/admin-web/Dockerfile`. |
| Admin Web runtime | NOT TESTED | **PASS — runs under emulation** | `docker image inspect --format '{{.Architecture}}'` reported `arm64`; `docker run --rm --entrypoint node <image> -p 'process.arch'` actually executed inside the emulated container and printed `arm64` (the same check `.github/workflows/arm64-compatibility.yml`'s `admin-web-arm64` job performs). |
| Doctor Web image build + runtime | NOT TESTED | **PASS — builds and runs under emulation** | Identical method and result to Admin Web, against the existing, unmodified `apps/doctor-web/Dockerfile`. |
| pgBackRest image build | NOT TESTED | **PASS — builds under `linux/arm64` emulation** | `docker buildx build --platform linux/arm64 --load` against the existing, unmodified `deploy/single-vm/backup/pgbackrest.Dockerfile` succeeded; apt resolved and installed the pinned `pgbackrest=2.60.0-1.pgdg12+1` for `arm64` from Debian bookworm's own repositories (the same version this file's own comment already recorded as locally exercised). |
| pgBackRest runtime | NOT TESTED | **PASS — runs under emulation** | `docker run --rm --entrypoint sh <image> -c 'dpkg --print-architecture; pgbackrest version'` actually executed inside the emulated container: real output `arm64` / `pgBackRest 2.60.0` (matching the pinned version). |
| Rust backend image build | NOT TESTED | **PASS — builds under `linux/arm64` emulation, after a fix (see below)** | `docker buildx build --platform linux/arm64 --load` against `services/backend/Dockerfile` completed the full multi-stage build — the entire `cargo build --release --bin hellodoctor-backend` dependency tree (Axum, Tokio, SQLx, Argon2, Ed25519, AES-GCM, Rustls, `aws-lc-rs` via `object_store`) compiled successfully for `aarch64-unknown-linux-gnu` under emulation. |
| Rust backend runtime | NOT TESTED | **PASS — runs real application logic under emulation** | `docker image inspect --format '{{.Architecture}}'` reported `arm64`. Running the compiled binary with no configuration produced a genuine Rust panic from real startup-validation code (`FATAL: ENVIRONMENT must be explicitly set...`, `src/config.rs:418`) — not an "exec format error" or segfault, i.e. a real aarch64 ELF binary executing real logic. Supplying minimal synthetic env vars advanced it to the next real check (`FATAL: JWT_ED25519_PRIVATE_KEY_B64 must be set...`, `src/config.rs:423`), confirming multiple layers of genuine startup logic execute correctly under emulation. No database/Redis/secrets were configured, so the process never attempted a real connection or any provider call — this is functional/startup evidence only, not a running-service test. |
| Backup toolchain shell scripts | NOT TESTED (no bash available) | **PASS — syntax-checked with real bash** | `bash -n` (GNU bash 5.3.9, via Git Bash) against `deploy/single-vm/verify-ubuntu-arm64-host.sh` and all seven scripts in `deploy/single-vm/backup/` reported no syntax errors. This is a syntax check only — no script was executed, so runtime behavior (actual pgBackRest invocation, Key Vault calls, cron registration) remains unverified. |

### A real bug found and fixed: `services/backend/Dockerfile`'s `TARGETARCH` check

The first `linux/arm64` backend build attempt failed immediately (in the architecture-guard `RUN` step the Dockerfile itself added for exactly this purpose):

```
unexpected Rust host aarch64-unknown-linux-gnu for TARGETARCH=amd64
```

Root cause: the Dockerfile declared

```dockerfile
FROM rust:1-bookworm@... AS builder
ARG TARGETARCH=amd64
```

`TARGETARCH` is one of Buildx's automatic platform build arguments — but per Docker's own documented behavior, a bare `ARG TARGETARCH` (no default) redeclared inside a stage is what inherits Buildx's real per-platform value; giving it an explicit hardcoded default (`=amd64`) instead defines a new, stage-scoped argument that is **not** fed by that automatic value unless `--build-arg TARGETARCH=...` is *also* passed explicitly on the command line. It was not — not in the command used here, and not in `.github/workflows/arm64-compatibility.yml`'s `rust-backend-arm64` job, which builds with the exact same invocation shape (`docker buildx build --platform linux/arm64 --load --tag ... --file services/backend/Dockerfile services/backend`, no extra `--build-arg`). **This means the real native ARM64 CI job, as currently written, would very likely have hit this exact same failure the first time it ran** — this was not an artifact of local emulation.

**Fix applied** (`services/backend/Dockerfile`): redeclare the argument bare, with no hardcoded default:

```dockerfile
FROM rust:1-bookworm@... AS builder
ARG TARGETARCH
```

**Fix verified two ways, both conclusive:**
1. The exact CI invocation shape (`--platform linux/arm64`, no extra `--build-arg`) now correctly resolves `TARGETARCH=arm64` and passes the architecture guard — confirmed by a full build completing successfully.
2. A native `--platform linux/amd64` build was run to confirm the existing amd64 build path (the Dockerfile's own stated "multi-platform build contract") is unaffected: `host=x86_64-unknown-linux-gnu` correctly matched the `amd64:x86_64-unknown-linux-gnu` case, and the build completed successfully end to end (image produced, architecture `amd64`). The fix is a strict generalization (a bare `ARG` inheriting Buildx's real per-platform value) with no special-casing removed, and this confirms no regression for the existing `amd64` path.

No other source, migration, RLS, or business-logic file was touched. Nothing was committed, staged, or pushed.

### What is still genuinely blocked — unchanged by this addendum

- **Real ARM64 silicon.** Every result above is QEMU user-mode emulation on an x86_64 host, not Azure Cobalt 100 hardware. Emulation proves functional/logical correctness (the code runs, the logic is right) but says nothing about real ARM64 performance, timing-sensitive behavior, or any hardware-specific edge case — the native ARM64 CI workflow (`arm64-compatibility.yml`) and the disposable six-service integration workflow (`arm64-disposable-integration.yml`) are still the authoritative gates for that, and neither has been dispatched.
- **Six-service integration** (real Postgres/Redis startup, migrations, RLS, Caddy HTTPS, auth/refresh/secure-cookie flows, restart persistence) — not attempted in this addendum; `deploy/single-vm/compose.local-verification.yml` exists for exactly this and is the natural next step, but still only under emulation here, not native hardware.
- **pgBackRest WAL archiving, Azure Blob repository access, retention, and PITR restore** — only the image build and a bare `pgbackrest version` call were verified; no backup was actually taken, no restore was attempted, and no Azure Blob credentials were used.
- **Azure Blob, FCM, email, Agora, and payment provider contract tests** — unchanged; provider credentials remain intentionally absent.
- **Image publication.** All images built in this addendum are local, untagged-for-registry, verification artifacts (`*:arm64-verify` / `*:test`) — none were pushed anywhere, and `deploy/single-vm/compose.yml` still requires real, digest-pinned, registry-published images via `${BACKEND_RUNTIME_IMAGE:?...}` etc.
- **Getting any of this into the real CI/CD and review path.** The `TARGETARCH` fix, and everything else described in this document, remain uncommitted working-tree changes. The publication sequence, file list, and exclusions already specified earlier in this document are unchanged and still apply.

