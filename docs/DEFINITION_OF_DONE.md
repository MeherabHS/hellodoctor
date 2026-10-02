# Phase 22 — Definition of Done (DoD) Quality Standard

> **Document Version:** 1.0.1  
> **Mandate:** A task, feature, or milestone is strictly INCOMPLETE if any item on this checklist is unsatisfied.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Feature Completion Checklist

Every feature implemented by Claude Code must satisfy all criteria before being marked complete in `docs/PROGRESS.md`:

- [ ] **1. Architecture Integrity:**
  - Code resides in appropriate layers (Presentation, Domain, Data for Flutter; Handler, Service, Repository for Rust).
  - No domain logic embedded directly in UI widgets.
  - No raw SQL queries outside the repository layer.
- [ ] **2. UI & Visual Fidelity:**
  - Matches prototype geometry, colors, spacing, and micro-interactions per `docs/VISUAL_FIDELITY.md`.
  - Zero star rating characters (`⭐`, `★`) on patient screens.
  - Emergency Contact strip labeled "Emergency Contact" (no "24/7") and positioned above Upcoming Appointments.
  - Doctor earnings cards display the mandatory 20% calculation basis note.
- [ ] **3. Dual-Layer Validation:**
  - Client-side validation in Flutter for immediate UX feedback.
  - Server-side validation in Rust (`validator` crate) enforcing security and business integrity.
- [ ] **4. Error Handling & Privacy:**
  - Standard error envelope returned (`success: false`, `code`, `message`).
  - Zero internal stack traces or database errors exposed to clients.
  - Sensitive Protected Health Information (PHI) scrubbed from system logs.
- [ ] **5. Test Coverage:**
  - Unit tests written for all Blocs, domain services, and mappers.
  - Integration tests written for database transactions and API endpoints.
  - All tests execute and pass (`cargo test`, `flutter test`).
- [ ] **6. Code Quality & Static Analysis:**
  - Flutter: `flutter analyze` reports zero errors and zero warnings.
  - Rust: `cargo clippy --all-targets -- -D warnings` passes cleanly.
- [ ] **7. Formatting Standards:**
  - Flutter: `dart format --set-exit-if-changed .` passes.
  - Rust: `cargo fmt -- --check` passes.
- [ ] **8. Security Audit:**
  - Passwords hashed using Argon2id.
  - No hardcoded API keys, tokens, or private secrets in repository.
  - Role-based (RBAC) and Attribute-based (ABAC) authorization enforced on all endpoints.
- [ ] **9. Idempotency & Concurrency:**
  - Booking and payment endpoints support `Idempotency-Key` headers.
  - Slot bookings execute within serializable or row-locked transactions (`FOR UPDATE`).
- [ ] **10. Responsive & Offline Behavior:**
  - Renders properly on mobile (`390px` width) and tablet viewports.
  - Destructive actions blocked with `#offlineActionGuardModal` when network is offline.
- [ ] **11. Acceptance Criteria Verified:**
  - All matching criteria in `docs/ACCEPTANCE_CRITERIA.md` verified and passing.
- [ ] **12. Documentation & Progress Tracking:**
  - Milestone progress updated in `docs/PROGRESS.md`.
  - Git commit created with descriptive message following conventional commits.
- [ ] **13. BOLA/IDOR Protection:**
  - Every endpoint that accesses user-specific resources verifies ownership + relationship + appointment context beyond role check.
- [ ] **14. File Upload Security:**
  - All file uploads pass magic-byte validation, malware scanning, and image re-encoding before storage.
- [ ] **15. Agora Token Security:**
  - Agora App Certificate exists only on server. RTC tokens are short-lived and bound to specific appointments.
- [ ] **16. Audit Trail:**
  - Sensitive actions (record access, payment, prescription, admin actions) are logged to audit_events table.
- [ ] **17. PHI Protection:**
  - Push notifications contain no PHI. Error responses contain no internal details. Logs use allow-list field scrubbing.
