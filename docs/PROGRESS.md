# HelloDoctor — Engineering Progress & Milestone Tracker

> **Document Version:** 1.0.0  
> **Status:** Architecture specification remediation complete — documentation is synchronized and ready for M2/M3 implementation.
> **Target Execution Agent:** Claude Code Autonomous Engineer  
> **Last Updated:** 2026-10-02

---

## 1. Executive Status Dashboard

- **Current Milestone:** M2 — Canonical State Model & Database v2
- **Overall Completion:** `18%`
- **Last Completed Task:** Authoring complete 30-phase engineering specification and ADR package.
- **Current Task:** Implement reversible PostgreSQL migrations and the M2/M3 authentication/API foundation.
- **Blocked Tasks:** None at the documentation stage; implementation must still pass the defined security and integration gates.
- **Known Issues:** Production services and migrations have not yet been created in this workspace.
- **Tests Passing:** Existing prototype regression claim remains historical; production test suites are not yet present.
- **Active Architecture Decisions:** ADR-001 through ADR-010 approved.

---

## 2. Milestone Execution Checklist

| Milestone ID | Description | Status | Verification Check |
|---|---|---|---|
| **SPEC** | Complete 30-Phase Architecture & Engineering Specs | **COMPLETE** | Cross-document stale-term and endpoint consistency checks pass |
| **M0-M1** | Architecture Remediation & Security Model | **COMPLETE** | Security, payment, schema, MFA, RLS, and privacy decisions synchronized |
| **M2** | Canonical State Model & Database v2 | PENDING | `sqlx migrate run` |
| **M3** | API v2 & Auth (Ed25519) | PENDING | OTP & JWT test suites pass |
| **M4** | Agora ADR & Implementation Setup | PENDING | `cargo check` & `flutter analyze` |
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

1. Implement reversible M2 PostgreSQL migrations from `docs/DATABASE_SCHEMA.md`.
2. Implement M3 auth/API foundations from `docs/API_SPEC.md` and `docs/AUTH_SECURITY.md`.
3. Run migration, RLS, MFA, and auth tests before beginning M4/M5 UI work.
4. Update this `docs/PROGRESS.md` document after completing each milestone.
