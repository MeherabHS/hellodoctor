# HelloDoctor — Engineering Progress & Milestone Tracker

> **Document Version:** 1.1.0  
> **Status:** M2 (Database v2) and M3 (Auth v2) implemented and verified against a real PostgreSQL instance. M4+ (Flutter mobile, Agora client UI, localization, offline guards) remain pending.
> **Target Execution Agent:** Claude Code Autonomous Engineer  
> **Last Updated:** 2026-10-02

---

## 1. Executive Status Dashboard

- **Current Milestone:** M4 — Agora ADR & Implementation Setup (Flutter mobile workspace)
- **Overall Completion:** `~30%`
- **Last Completed Task:** Replaced the backend's in-memory `AppState` with a real PostgreSQL-backed repository layer (13 per-aggregate repo modules), replaced the fully-bypassable auth (hardcoded OTP/MFA codes, unchecked doctor password) with real Argon2id password hashing, hashed/rate-limited OTP, encrypted-at-rest admin TOTP, and Ed25519-signed JWTs; fixed the disbursement batch math to sum ground-truth transaction rows instead of re-deriving gross via division; implemented real idempotency and a Prometheus `/metrics` + tracing telemetry module; restricted CORS to a configured origin allow-list; wired the admin portal's earnings views to the mandatory 20%-disclosure text and consistent fee wording, fixed its grievance-status enum to match the backend, added the missing `/admin/grievances/:id/dismiss` endpoint end-to-end, and removed dead `Sidebar.tsx`/`Topbar.tsx` components.
- **Current Task:** None in flight. Next up is M4+ (Flutter mobile app) per the roadmap, which this change did not touch.
- **Blocked Tasks:** None.
- **Known Issues:**
  - No request-authentication middleware yet extracts a real caller identity from the JWT; handlers still use fixed demo actor IDs per route group (pre-existing simplification, unchanged by this pass) — a prerequisite for real authorization (BOLA/IDOR) hardening (M1/M-SECURITY).
  - Admin-created doctor accounts get a random temporary password returned once in the API response; there's no "change password" or TOTP-enrollment flow yet.
- **Tests Passing:** `cargo test` — 11/11 integration tests pass against a live Postgres 16 instance (`services/backend/docker-compose.yml`), covering OTP/password/TOTP auth, atomic slot booking + 20% fee split, the 5-photo upload cap, grievance refund/warn/dismiss, 24h chat expiry, and monthly disbursement aggregation. `cargo clippy --all-targets -- -D warnings` is clean. Admin-web: `npx tsc --noEmit` and `npm run build` are clean.
- **Active Architecture Decisions:** ADR-001 through ADR-010 approved.

---

## 2. Milestone Execution Checklist

| Milestone ID | Description | Status | Verification Check |
|---|---|---|---|
| **SPEC** | Complete 30-Phase Architecture & Engineering Specs | **COMPLETE** | Cross-document stale-term and endpoint consistency checks pass |
| **M0-M1** | Architecture Remediation & Security Model | **COMPLETE** | Security, payment, schema, MFA, RLS, and privacy decisions synchronized |
| **M2** | Canonical State Model & Database v2 | **COMPLETE** | `sqlx migrate run` applies cleanly (6 reversible migrations, each with a `.down.sql`); 13 per-aggregate repository modules replace the in-memory `AppState` |
| **M3** | API v2 & Auth (Ed25519) | **COMPLETE** | Real Argon2id passwords, hashed+rate-limited OTP, encrypted TOTP, Ed25519 JWT with `permissions[]` claim; `cargo test` auth suites pass |
| **M4** | Agora ADR & Implementation Setup | PENDING | Backend RTC-token endpoint exists (`agora_token_service.rs`); Flutter mobile workspace (`apps/mobile/`) not yet created — `cargo check` passes, `flutter analyze` N/A |
| **M5** | Specialist Discovery & Star-Rating Elimination | PENDING | Zero star rating widget test passes |
| **M6** | Emergency Contact Helpline & Patient Home | PENDING | Banner position & dialer test passes |
| **M7** | Atomic Slot Booking & Payment Hold | PENDING | Concurrency double-booking and payment-reconciliation tests pass |
| **M8** | Multi-Prescription Upload Engine (Max 5 Images) | PENDING | Cap & dropzone widget tests pass |
| **M9** | Virtual Waiting Room & Device Readiness | PENDING | Pre-flight mock tests pass |
| **M10** | Agora RTC Telehealth & Telemetry Auto-Capture | PENDING | Agora token tests pass |
| **M11** | 24-Hour Clinical Chat Desk | PENDING | 24h lock & WebSocket broadcast tests pass |
| **M12** | Handwritten Prescription Photos & Health Vault | PENDING | Upload-pipeline and cryptographic integrity tests pass |
| **M13** | Doctor Wallet & 20% Debarred Settlement | PENDING | 3-tier arithmetic unit test passes |
| **M14** | Clinical Grievance Redressal & Board Arbitration | PENDING | Dispute & payment refund tests pass |
| **M15** | Central Admin Consoles & Doctor Dossiers | PENDING | Dossier & error log tests pass |
| **M16** | Bilingual Localization (English/Swahili) | PENDING | Language switcher & offline tests pass |
| **M17** | Offline Guards | PENDING | Network guard tests pass |
| **M-SECURITY**| Security Gates & Verification | PENDING | OWASP, ABAC, RLS, File Upload, Push Audits |
| **M18** | Final Quality Assurance & Production Verification | PENDING | 100% full regression pass |

---

## 3. Immediate Next Actions for Claude Code

1. M4: scaffold the Flutter mobile workspace (`apps/mobile/`) per `docs/FLUTTER_ARCHITECTURE.md` and wire it to the now-real backend.
2. Add request-authentication middleware that extracts the caller's identity from the Ed25519 JWT, then replace the handlers' hardcoded demo actor IDs with it (prerequisite for M1/M-SECURITY's BOLA/IDOR gate).
3. Add a doctor/admin password-change and TOTP-enrollment flow (today these are only seeded for the fixed demo accounts).
4. Continue M5+ per the roadmap once M4 lands.
5. Update this `docs/PROGRESS.md` document after completing each milestone.
