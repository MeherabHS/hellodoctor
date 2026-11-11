# HelloDoctor.Asia ARM64 suitability assessment

**Assessment date:** 2026-11-05  
**Scope:** read-only repository and documentation review plus current Microsoft/Azure documentation review. No source, image, benchmark, database, VM, or deployment was changed.

## Decision

**Recommendation B — ARM64 is technically feasible but offers insufficient demonstrated advantage to justify migration now.**

The application is predominantly portable. Rust, PostgreSQL 16, Redis, Caddy, Node.js, Next.js, and the provider protocols have ARM64 implementations. The concrete work is to produce and verify ARM64 images and to run an ARM64 load/soak comparison. The repository does not contain that evidence today. Milestone 8 already meets the useful local acceptance signals on the approved 4-vCPU/8-GiB topology (187.5 RPS in the corrected 60-minute run, 0.28% true system-error rate, and 99.9% booking completion), so a move would currently add supply-chain, CI, rollback, and performance-validation work without a measured product benefit.

ARM64 should remain a controlled test candidate. It should not become the production default until an ARM64 image, restart/backup drill, and workload-equivalent benchmark meet the existing x86 acceptance gates.

## Evidence reviewed

Repository evidence included:

- Rust manifest and lockfile: `services/backend/Cargo.toml`, `services/backend/Cargo.lock`.
- Backend images: `services/backend/Dockerfile`, `deploy/single-vm/backend.Dockerfile`.
- Web images and lockfiles: `apps/admin-web/Dockerfile`, `apps/admin-web/package.json`, `apps/admin-web/package-lock.json`, `apps/doctor-web/Dockerfile`, `apps/doctor-web/package.json`, `apps/doctor-web/package-lock.json`.
- Production Compose and proxy: `deploy/single-vm/compose.yml`, `deploy/single-vm/compose.backup.yml`, `deploy/single-vm/backup/pgbackrest.Dockerfile`, `deploy/single-vm/Caddyfile`.
- CI: `.github/workflows/application-checks.yml`, `.github/workflows/backend-migrations.yml`, `.github/workflows/security-scan.yml`.
- Milestone 8 evidence: `docs/MILESTONE_8_ENDURANCE_VALIDATION_AND_HARNESS_FIXES.md`, `docs/MILESTONE_8_SUSTAINED_RELIABILITY.md`, `docs/MILESTONE_8_MEMORY_PROFILE.md`, `docs/MILESTONE_8_FINAL_ACCEPTANCE.md`.
- Recovery/provider status: `docs/MILESTONE_8_FINAL_ACCEPTANCE.md`, `docs/BACKUP_RESTORE_RUNBOOK.md`, `docs/AZURE_BLOB_SETUP.md`, `docs/FCM_SETUP.md`, `docs/VM_CONFIGURATION_GUIDE.md`.

Current upstream evidence:

- Microsoft documents Ampere Altra ARM64 Dpsv5/Dplsv5 families, including `Standard_D4pls_v5` (4 vCPU, 8 GiB), Premium Storage, accelerated networking, live migration, and Generation 2 support: [Dplsv5 sizes](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/general-purpose/dplsv5-series), [Dpsv5 sizes](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/general-purpose/dpsv5-series).
- Microsoft documents x86-64 `Standard_D4ls_v5` (4 vCPU, 8 GiB) and its storage/network characteristics: [Dlsv5 sizes](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/general-purpose/dlsv5-series).
- Azure also documents newer Cobalt 100 ARM64 Dplsv6/Dpsv6 families; SKU and region selection remains a separate availability check: [Cobalt VM overview](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/cobalt-overview).
- Microsoft’s availability guidance is to query the target region with `az vm list-skus --location <region>`; the intended production region was not supplied, so regional capacity is **unverified**: [VM size overview](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes-general), [Products by region](https://azure.microsoft.com/en-us/explore/global-infrastructure/products-by-region/).
- Microsoft’s unauthenticated Retail Prices API is the authoritative way to obtain the current regional meter; prices below are therefore marked as indicative unless the query is repeated for the selected region and subscription: [Retail Prices API](https://learn.microsoft.com/en-us/rest/api/cost-management/retail-prices/azure-retail-prices).

## Compatibility matrix

| Component or assumption | Classification | Repository evidence and ARM64 assessment |
|---|---|---|
| Rust stable and `aarch64-unknown-linux-gnu` | Compatible with build/configuration changes | No `rust-toolchain.toml` or target pin exists. CI uses Rust 1.99.0 in `.github/workflows/backend-migrations.yml`; the backend Dockerfile uses a digest-pinned `rust:1-bookworm` builder. Rust’s normal Linux ARM64 target is appropriate, but the target must be explicitly built and tested. |
| Axum, Tokio, Tower, SQLx/PostgreSQL | Compatible without source changes | Pure Rust/network/database crates in `Cargo.toml`; no architecture-specific code was found. SQLx uses `runtime-tokio-rustls` for database connections. |
| Rust cryptography (`argon2`, `ed25519-dalek`, `aes-gcm`, `sha2`, `hmac`, `jsonwebtoken`) | Compatible without semantic changes; verify performance | These crates are portable. The lockfile also contains `ring` and `aws-lc-rs` through TLS/object-store paths; their ARM64 native code must be built and smoke-tested. No security control should be changed for ARM. |
| HTTP TLS/native libraries | Compatible with build/configuration changes; verification required | Direct `reqwest = "0.12"` does not disable default features. `Cargo.lock` therefore contains `native-tls`, `openssl`, and `openssl-sys`; the runtime image installs `libssl3`. Debian ARM64 packages exist, but the builder’s OpenSSL headers/linking and runtime ABI need an ARM build check. `object_store` also brings `aws-lc-rs` and reqwest 0.13. |
| PostgreSQL 16 image and `pg_stat_statements` | Compatible without source changes; digest verification required | Official `postgres:16` publishes `arm64v8`; PostgreSQL 16 itself supports ARM. `pg_stat_statements` is built into the official image. `deploy/single-vm/compose.yml` accepts a reviewed digest, so the ARM64 manifest/child digest must be verified. |
| Redis 7 | Compatible without source changes; digest verification required | Official Redis images publish `arm64v8`; the Compose command (`appendonly yes`) is architecture-neutral. Pin and inspect the ARM64 image rather than reusing an x86 child digest. |
| Caddy | Compatible without source changes; digest verification required | The official Caddy image publishes `arm64v8`. `deploy/single-vm/Caddyfile` contains no CPU-specific behavior. |
| Node.js 22 Bookworm and Next.js runtime | Compatible with build/configuration changes | Official Node images publish ARM64 and Next.js supports Linux ARM64. The Dockerfiles use a digest supplied by the operator but do not record its platform; verify the digest is a multi-platform index or ARM64 child. |
| Next.js SWC | Compatible with build/configuration changes | Both npm lockfiles contain `@next/swc-linux-arm64-gnu` and `@next/swc-linux-arm64-musl`. `npm ci` must retain optional platform packages; a lockfile generated only on x86 can otherwise select the wrong optional set. Next.js explicitly documents architecture-related SWC load failures. |
| Sharp/libvips and other native Node packages | Compatible with build/configuration changes; verify | Both lockfiles contain `@img/sharp-linux-arm64`, ARM64 libvips, and ARM64 musl variants. The standalone images rely on Next image optimization, so an ARM build must run `next build` and a production image request. |
| Docker Compose and service limits | Compatible with configuration changes | `compose.yml` has no `platform:` override and resource limits are architecture-neutral. Image variables are intentionally external and must be supplied with ARM64-capable digests. `cpuset: "0-3"` remains valid on a 4-vCPU host. |
| pgBackRest backup image | Requires verification | `deploy/single-vm/backup/pgbackrest.Dockerfile` starts from `postgres:16` and installs Debian `pgbackrest`, `zstd`, Python, cron, and curl. The scripts are POSIX shell and use no x86 binary, but the image/package availability, Azure repository path, encryption, WAL archive, and restore must be checked on ARM64. |
| Azure Blob, email OTP, FCM, Agora, payments | Compatible without architecture-specific protocol changes; provider verification remains required | These are HTTPS/provider integrations in Rust and deployment configuration. ARM changes neither RLS/ABAC nor wire formats. Existing Milestone 8 documents still mark live credentials, provider delivery, and some payment operations as blocked or partial. |
| GitHub Actions | Compatible with build/configuration changes | Current workflows run on `ubuntu-latest` (x64) and do not build/publish multi-arch images. GitHub documents ARM64 hosted labels such as `ubuntu-24.04-arm`, and Docker documents Buildx/QEMU or native multi-node builds. |

### Actual x86-64-specific assumptions

1. `services/backend/Dockerfile:8-16` explicitly says its Rust and Debian digests were verified on `linux/amd64` and gives an `amd64` pull command. Those digests must not be assumed valid for ARM64.
2. The web Dockerfiles use a digest supplied as `node@sha256:...` without recording the architecture check. A digest could be a manifest index or a single child; this is currently unknown.
3. No ARM64 target, `docker buildx`, `--platform linux/arm64`, multi-architecture manifest, or native ARM CI job exists in the repository.
4. Local benchmark evidence used Windows/Docker Desktop or x86-constrained containers. It is not an ARM measurement.
5. CI service images are unpinned tags (`postgres:16`, `redis:7-alpine`) on x64 GitHub runners. They test function, not ARM image resolution.

## Azure ARM64 VM suitability

### SKU options

| Family | Candidate | vCPU | RAM | Storage/network notes | Status |
|---|---:|---:|---:|---|---|
| ARM Ampere Altra | `Standard_D4pls_v5` | 4 | 8 GiB | No local temp disk; up to 8 remote disks; 6,400 uncached Premium SSD IOPS; 12,500 Mbps network; Premium Storage and accelerated networking supported | Best resource-shape match; target-region capacity unverified |
| ARM Ampere Altra | `Standard_D4ps_v5` | 4 | 16 GiB | No local temp disk; up to 8 remote disks; 6,400 IOPS; 12,500 Mbps network | Safer memory margin, but doubles RAM versus the plan; region/price unverified |
| ARM Ampere Altra | `Standard_D4pds_v5` | 4 | 16 GiB | Adds 150 GiB local temporary disk; same general network class | Useful only if local scratch is desired; database durability still belongs on managed disks |
| ARM Cobalt 100 | Dplsv6/Dpsv6 4-vCPU sizes | 4 | 8 or 16 GiB class | Newer ARM generation; exact SKU, price, and region must be checked | Candidate for a later comparison, not accepted on documentation alone |
| x86-64 baseline | `Standard_D4ls_v5` | 4 | 8 GiB | No local temp disk; 6,400 IOPS; 12,500 Mbps network | Closest named x86 shape; target-region capacity also requires `az vm list-skus` |

The assumption that ARM64 necessarily forces a 16-GiB VM is false: Microsoft documents `D4pls_v5` at exactly 4 vCPU/8 GiB. The low-memory ratio leaves less headroom for PostgreSQL, Node, Docker, and backup activity than `D4ps_v5`; that is a capacity choice, not an architecture advantage.

### Availability, operating system, and operations

- The intended Azure region is not stated. Therefore neither `D4pls_v5` nor `D4ls_v5` capacity, quota, zone support, or current offer is certified for production. Before purchase, run `az vm list-skus --location <region> --resource-type virtualMachines` and confirm `locationInfo`, restrictions, quota, and image availability.
- Azure documents ARM64 support for Azure Linux and mainstream Linux distributions. Use a supported ARM64 Ubuntu/Azure Linux image and Generation 2 VM; do not carry an x86 image URN into deployment.
- Dplsv5 and Dlsv5 both support Premium Storage and accelerated networking. Dplsv5 does not support ephemeral OS disks or nested virtualization; neither is required by this Compose deployment.
- Both families have live migration and memory-preserving update support in Microsoft’s tables. Availability-zone and capacity behavior remain region/SKU-specific.
- Docker’s Linux ARM64 container path is supported. A host-level operational risk remains: every image, backup helper, monitoring binary, and emergency diagnostic must be ARM64 or a verified multi-arch manifest. Accidental x86 pulls can fail at start or invoke emulation with poor performance.

### Cost comparison

Azure prices are dynamic and region-specific. The repository does not name a region or subscription, and the Microsoft Retail Prices API could not be treated as a fixed quote. The following East US values are **indicative snapshots from API-derived pricing mirrors, not an approval or a Microsoft quote**; repeat the official API query before budgeting:

| Shape | Indicative Linux PAYG | Indicative monthly compute (730 h) | Authority/status |
|---|---:|---:|---|
| `Standard_D4ls_v5` x86, 4/8 | about $0.170/h | about $124.10 | Snapshot only; verify with Microsoft API |
| `Standard_D4pls_v5` ARM, 4/8 | about $0.136/h | about $99.28 | Snapshot only; verify region/SKU with Microsoft API |
| `Standard_D4ps_v5` ARM, 4/16 | about $0.154/h | about $112.42 | Snapshot only; verify region/SKU with Microsoft API |

The indicative figures were cross-checked against API-derived public mirrors for [`D4ls_v5`](https://www.economize.cloud/resources/azure/pricing/virtual-machine/d4lsv5/), [`D4pls_v5`](https://instances.usage.ai/azure/vm/Standard_D4pls_v5), and [`D4ps_v5`](https://cld2day.com/tools/azure-vm/standard_d4ps_v5). Those mirrors are not Microsoft billing sources; the Microsoft Retail Prices API query for the selected region is the acceptance source.

The indicative ARM saving is roughly 20% for the same 4/8 shape, but it is not a measured total-cost saving: managed disks, public IP, egress, monitoring, backup storage, reserved pricing, taxes, and support are excluded. If the target region has no D4pls capacity, the 16-GiB D4ps option may change the comparison. Do not make a purchase decision from these snapshots.

## Milestone 8 performance suitability

The latest corrected evidence is positive but not architecture-specific:

- `docs/MILESTONE_8_ENDURANCE_VALIDATION_AND_HARNESS_FIXES.md` records a clean 60-minute disposable run at **187.5 steady-state RPS**, **0.28% full-run true system-error rate**, and **2,749/2,751 (99.9%) booking success**.
- The same run reports no PostgreSQL lock waits at the approved pool=10 configuration and no resource exhaustion. The first 15–20 minutes had higher error rates, followed by a low steady background rate; that warm-up behavior must be preserved in an ARM comparison.
- The corrected 40-minute 2,000-session evidence in `docs/MILESTONE_8_SUSTAINED_RELIABILITY.md` reports approximately **160 RPS**, 0.3503% true system errors, zero real 500s, zero OOM/restarts, backend CPU about 12–15%, PostgreSQL CPU about 16–22%, Redis about 2.4%, backend memory about 31–32 MB, and PostgreSQL memory stabilizing around 1.12–1.16 GiB.
- Earlier diagnostic runs show that PostgreSQL CPU/query shape and admission pressure, rather than a proven CPU instruction-set limitation, can dominate. Chat advisory-lock contention occurred in a wider pool experiment but not in the accepted pool=10 endurance run.

Nothing in these measurements predicts ARM64 being faster or slower. Ampere Altra provides one physical core per vCPU at 3.0 GHz; x86 Azure generations and host scheduling differ. Rust async I/O, PostgreSQL, and Redis may show similar behavior, while TLS, Argon2, image processing, SWC/Sharp, compression, and PostgreSQL query CPU can vary by microarchitecture and implementation dispatch.

**A separate ARM64 benchmark is required before acceptance.** It must reproduce the corrected pacing, dataset, pool/admission settings, 60-minute duration, error classification, booking outcome, CPU/RAM, PostgreSQL pool pressure, permit occupancy, advisory-lock waits, restart recovery, and image-processing paths. An ARM smoke test or x86 emulation run is not sufficient.

## Docker and deployment portability plan (proposed, not implemented)

1. **Backend build:** add an explicit stable toolchain/target policy and build `aarch64-unknown-linux-gnu`. Re-verify the Rust and Debian Bookworm base images on ARM64; do not reuse the `linux/amd64`-verified digests in `services/backend/Dockerfile`.
2. **TLS/native dependencies:** decide whether to retain `reqwest`’s native-tls/OpenSSL path or explicitly use Rustls for the direct client. If retaining it, prove ARM64 OpenSSL headers at build time and `libssl3` ABI at runtime. Verify `aws-lc-rs`, `ring`, `zlib-rs`, `zstd`, and image codecs in the locked graph.
3. **Web images:** build Admin and Doctor images for `linux/arm64`; verify optional `@next/swc-linux-arm64-gnu` and `@img/sharp-linux-arm64` are installed by `npm ci`, then run standalone production builds and image optimization requests. Keep the lockfiles; do not remove optional packages to make x86 CI pass.
4. **Manifest publication:** publish `linux/amd64,linux/arm64` OCI manifests with Buildx and sign/attest the result. `deploy/single-vm/compose.yml` can continue to consume digest-pinned variables, but each digest must be a reviewed manifest or the correct ARM64 child.
5. **PostgreSQL/Redis/Caddy:** inspect the exact digest with `docker buildx imagetools inspect`; confirm ARM64 child digests, `pg_stat_statements`, Redis AOF behavior, Caddy health checks, and filesystem permissions. Pin the reviewed results in the private deployment environment.
6. **pgBackRest:** build `deploy/single-vm/backup/pgbackrest.Dockerfile` for ARM64, verify Debian package architecture, zstd compression, Azure Blob authentication, encrypted repository access, WAL archiving, retention, and disposable PITR restore. The existing scripts are architecture-neutral, but production backup/restore is still not fully activated according to Milestone 8.
7. **Compose/runtime:** do not add an x86 emulation fallback. Keep RLS, separated pools, read-only containers, dropped capabilities, `no-new-privileges`, private data network, volume names, and timeout/resource settings unchanged unless the ARM benchmark supplies evidence for a separately reviewed adjustment.
8. **CI:** add a native ARM64 job (`ubuntu-24.04-arm` or an isolated self-hosted ARM64 runner) for Rust locked build/tests and both web builds. Add a Buildx job with `linux/amd64,linux/arm64`; QEMU is acceptable for a first build but native ARM should perform the release smoke test. Current `ubuntu-latest` jobs remain x64-only evidence.
9. **Rollback:** retain the current x86 images and image digests as a tested rollback set. A manifest tag must never silently move an existing deployment between architectures.

## Provider, backup, and security impact

| Area | ARM64 impact | Required conclusion |
|---|---|---|
| PostgreSQL RLS/capability roles | None in semantics; CPU and package behavior can differ | Preserve role-separated pools, RLS, ABAC, and migration order; run the same authorization suite on ARM. |
| Authentication and authorization | None in token/lock semantics | Ed25519/JWT, refresh-family locks, rate limits, and session revocation remain the same. Verify crypto/provider latency only. |
| Payment idempotency and locking | None in transaction semantics | PostgreSQL advisory locks and unique/idempotency constraints are architecture-neutral. Re-run payment race, retry, timeout, and reconciliation tests on ARM; do not infer money correctness from an HTTP 2xx. |
| Cryptography | Portable algorithms; native TLS/crypto dispatch needs build/runtime verification | Keep algorithms and key sizes unchanged. Record any ARM-specific implementation or performance change. |
| Backup/PITR | Format is portable, binaries/images are not automatically portable | A pgBackRest repository can be restored cross-architecture when PostgreSQL major/version and configuration are compatible, but ARM backup image, WAL archive, encryption, Azure Blob path, and restore drill must be proven. |
| Container isolation | No architectural weakening required | Preserve read-only roots, dropped capabilities, private network, external volumes, and `no-new-privileges`. |
| Dependency vulnerability management | Same source advisories, different binary artifacts | Scan lockfiles and the ARM image SBOM separately; ensure native optional Node packages and OS packages are included in the ARM scan. |
| Incident recovery/rollback | More image variants to retain | Keep x86 rollback artifacts, record platform in release metadata, and test restart/restore on ARM before production. |

Provider status remains the existing status, independent of CPU architecture: Azure Blob cloud RBAC/private endpoint, email OTP delivery, FCM device delivery, Agora device/media behavior, and real payment gateway execution are not certified by this assessment. ARM64 does not close those gaps.

## Cost-benefit and migration effort

| Category | Current x86-64 plan | ARM64 alternative |
|---|---|---|
| Azure VM SKU | Unnamed 4-vCPU/8-GiB plan; closest named shape `Standard_D4ls_v5` | `Standard_D4pls_v5` (Ampere Altra, 4 vCPU/8 GiB); `D4ps_v5` is the 16-GiB fallback |
| vCPU | 4 | 4 |
| RAM | 8 GiB | 8 GiB exact match; 16 GiB fallback |
| Monthly VM cost | Indicative East US D4ls: ~$124.10 PAYG; verify officially | Indicative East US D4pls: ~$99.28 PAYG; verify officially; D4ps ~$112.42 |
| Container compatibility | Current images and CI were verified on x86 paths | Official Postgres/Redis/Caddy/Node support ARM64, but application and backup images need ARM builds and digest review |
| Estimated migration effort | Existing operational path | Medium: multi-arch Docker/CI work, native dependency checks, ARM smoke/soak, backup/restore, release/rollback evidence |
| Performance evidence | Corrected 60-minute evidence: 187.5 RPS, 0.28% true errors, 99.9% booking | No ARM measurements; cannot claim faster or slower |
| Operational risks | Familiar x86 images and existing rollback artifacts | Unverified regional capacity, ARM child-digest drift, optional Node binaries, OpenSSL/AWS-LC build paths, and a second benchmark/rollback matrix |
| Overall suitability | Ready for the existing evidence path, subject to Milestone 8’s non-architecture production gates | Feasible test candidate; not justified as the default production platform yet |

## Acceptance gates and remaining gaps

Affected-module map: Rust build/TLS/crypto, Node/Next native packages, PostgreSQL/Redis/Caddy images, pgBackRest/WAL/PITR, Compose image selection, CI runners/buildx, Azure VM SKU/region, and release rollback. No application record, ledger, identity, clinical row, audit event, or provider state was changed.

Local correctness, system consistency, and failure-recovery gates for money/identity/clinical state were **not executed** because this assignment prohibits benchmark execution and deployment. The report therefore records source and documented evidence only; it does not certify an ARM provider, VM, or production deployment. Before accepting ARM64, run the following on disposable fixtures:

- ARM64 Rust locked build, unit/integration authorization suite, and web production builds.
- ARM64 six-service startup, health checks, image optimization, restart, and volume persistence.
- ARM64 corrected-pacing 60-minute endurance plus the 2,000-session sustained scenario, with the same error and business-outcome accounting.
- Pool pressure, admission-permit occupancy, PostgreSQL wait/lock metrics, CPU/RAM, and advisory-lock comparisons.
- Payment idempotency/race/retry/timeout tests and RLS/capability-role checks.
- pgBackRest encrypted backup, WAL/PITR, Azure Blob round-trip, and replacement-host restore drill.
- Provider/device checks separately for Azure Blob, email OTP, FCM, Agora, and payment gateways.

These are validation gates, not proposed source changes. No code or deployment file was modified for this assessment.

