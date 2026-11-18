# Assurance baseline

Everything in this pack was measured against exactly this system. To reproduce a number, rebuild
this baseline and rerun the scenario (see [PERFORMANCE_REPORT.md §7](PERFORMANCE_REPORT.md#7-reproducing-these-results)).

## System under test

| Item | Value |
|---|---|
| Git commit | `eef9ce74a4fe44211edee0fc51308db36a88bce7` (branch `feat/afyamkononi-website`; tree `f05d416c148dd66e420224780c605c9078ee6570`) |
| Former hash | `6f02b84ec3a003ff6946c63c41bed8d348099e32`: the same commit before the 2026-11-18 history rewrite that removed commit-message attribution lines. Code is byte-identical (same tree); the local image tags below keep the old short hash as their label |
| Working tree | `services/backend/src`, `services/backend/migrations`, `apps/admin-web`, `apps/doctor-web` identical to the commit |
| Backend version | `hellodoctor-backend` 2.4.0 |
| Migrations | 130 up + 130 down (every migration reversible); latest `20261222000012_booking_emails_every_role` |
| Migration-set hash | `8595e0af8b3187bbeaa66fa90a27d4dd656a13bb778e33747624103b4dd139b5` |
| Row-level security | Enabled **and forced** on 82 tables; 5 `NOLOGIN NOBYPASSRLS` capability roles |
| Financial module | `FINANCIAL_MODULE_MODE=deferred`, as production runs it (no payment gateway is integrated) |

**Measured code vs. later work.** Every scenario ran against the commit above. Work made after it
(chat read receipts with migration `20261222000013_chat_read_markers`, notification sounds,
the Doctor app package rename) is **not** in the measured images. The one deliberate exception is the
*after-fix* runs (`*_fixed` evidence folders), which use `hellodoctor-backend:perf-fixed`: the
baseline commit plus only the two proxy-race and log-noise fixes described in
[PERFORMANCE_REPORT.md](PERFORMANCE_REPORT.md), so a before/after comparison changes one thing.

The migration-set hash is reproducible from git:

```bash
cd services/backend/migrations
for f in $(ls | LC_ALL=C sort); do printf '%s\n' "$f"; git hash-object "$f"; done | sha256sum
```

## Topology and resource envelope (target: 4 vCPU / 8 GB)

The approved single-VM production topology, `deploy/single-vm/compose.yml`, with the
`compose.perf-baseline.yml` overlay. All six services share four CPUs (`cpuset 0-3`); limits were
read back from the running containers with `docker inspect`, not taken from the file.

| Service | Image (local build or pinned tag) | Memory cap | CPU share weight |
|---|---|---:|---:|
| PostgreSQL 16.16 | `postgres:16-bookworm` | 3,072 MiB | 3000 |
| Redis 7.4.11 | `redis:7-alpine` | 384 MiB | 500 |
| Backend (Rust/Axum) | `hellodoctor-backend:perf-6f02b84` `sha256:6b55356c…05e5bc` | 1,536 MiB | 6000 |
| Admin Web (Next.js) | `hellodoctor-admin-web:perf-6f02b84` `sha256:9bca4b29…890f15` | 448 MiB | 750 |
| Doctor Web (Next.js) | `hellodoctor-doctor-web:perf-6f02b84` `sha256:1c2c0264…a469b4` | 384 MiB | 750 |
| Caddy 2.11.7 | `caddy:2.11.7-alpine` | 128 MiB | 500 |
| **Total** | | **5,952 MiB** of 8 GB | |

Backend settings that shape capacity, at production values: 5 database pools × 10 connections,
admission-control lanes from `src/admission.rs`, per-account limiters (OTP, login, booking
10/hour, grievance), Argon2id with 4 hashing workers.

## Load generator

`hellodoctor-loadgen:perf-6f02b84` (`services/backend/bench/loadgen.Dockerfile`), built from the
same commit plus the harness changes in this pack. It runs as a container pinned to CPUs 4-11, never
on the four server CPUs, and reaches the API through Caddy (`Host: api.localhost`), the same path a
phone would take.

## Dataset

`docs/assurance/evidence/baseline/CANONICAL_BASELINE_MANIFEST.json`, **version 2**. 10,194 users
(10,000 patients, 142 doctors including 100 login-test doctors, 40 peer navigators, admins),
13,440 historical appointments with chat conversations, 73,920 chat messages, 62,405 doctor
schedule slots and 12,000 peer slots. All synthetic. Built on a freshly migrated database from
`scripts/seed_capacity_dataset.sql`, `seed_capacity_dataset_scaleup_10k.sql`,
`seed_capacity_peers.sql` and `seed_auth_stress_doctors.sql`; the login scenarios additionally apply
`seed_auth_stress_patients.sql` as per-scenario setup. The snapshot (gitignored, SHA-256
`655af6f9…967ad17c`) is restored and verified (hash, row counts, roles, forced RLS) before
**every** scenario, and Redis is flushed, so no run inherits another's state.

Version 1 (without peers and login-test users, SHA-256 `e722ef4d…3768bc`) was used only by the
first exploratory ramp (`ramp_v1`), which is kept for the record and not quoted.

## Host

| Item | Value |
|---|---|
| CPU | AMD Ryzen 5 5600 (6 cores / 12 threads); the four server vCPUs are hardware threads, as cloud vCPUs are |
| RAM | 23.9 GB physical; Docker Desktop VM sees 12 CPUs / 19.5 GiB |
| Docker | Engine 29.7.2, WSL2 kernel 6.18.33.2 |
| OS | Windows 11 Pro 10.0.26200 |

## Differences from production (read before quoting any number)

1. **x86-64, not ARM64.** Production targets Ubuntu 24.04 ARM64; `compose.yml` pins `linux/arm64`.
   Running that here would mean CPU emulation, so the overlay sets `linux/amd64`.
2. **No TLS.** Caddy serves plain HTTP; production adds TLS termination cost at the proxy.
3. **One machine, no network.** Load generator and server share a host, with no internet latency.
   Real users in Kenya on mobile networks will see higher end-to-end latency than the server-side
   numbers here. The generator's CPU separation is at the VM-vCPU level; Windows may still schedule
   both onto the same physical cores.
4. **One source IP.** All synthetic users arrive from one address, so the per-IP global limit is
   raised to 100,000/min. Every other control (JWT, sessions, ABAC, RLS, per-account limiters,
   admission control) runs at production values.
5. **No external providers.** Payments are deferred (no gateway, as in production); the
   `*_payments_enabled` folders are an earlier exploratory configuration and are not quoted. Agora,
   FCM and email are not called.
6. **Development mode flags** (`ENVIRONMENT=development`, dev OTP) so the Admin/Doctor web
   workload can sign in without an SMS provider, as in the Milestone 8 runs.
