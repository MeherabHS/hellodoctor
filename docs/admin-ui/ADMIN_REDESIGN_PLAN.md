# Admin Web redesign — plan and product-owner decisions

Product-owner instruction, 2026-11-13/14. The current Admin Web (prototype styling) is replaced by the five AfyaMkononi "Admin Workflow" boards in `docs/design/admin/`:

| Board | File |
|---|---|
| Blog Manager | `admin-blog-manager-workflow.png` |
| Facility Management | `admin-facility-management-workflow.png` |
| People & Care Records | `admin-people-and-care-workflow.png` |
| Operations, Safety & Governance | `admin-operations-safety-governance-workflow.png` |
| Campaigns & Finance | `admin-campaigns-and-finance-workflow.png` |

The boards are 1672 × 941 collages; each panel is a small rendering, so they set the design language (green sidebar, white cards, stat tiles, tabs, filter bars, chips, tables, phone previews) rather than exact pixel values. The boards use slightly different sidebars; the app uses one merged sidebar.

Only presentation changes. BFF routes, session handling, API clients, validation, workflows and tests stay; the two new backend features below are the only functional additions.

## Decisions

### Removed from the design (not built, not shown)

| Area | Removed |
|---|---|
| Blog | Article views and reads counts; per-topic order and "visible in app" toggles; short summary; auto-save; Resources section |
| Facilities | Facility type; city/town; per-facility services table (order, opening days, visibility, call). Patient preview has no Nearby chip, no View on Map, no bookmark or share |
| Patients | Key conditions, care status, risk level, care plan, NHIF number, phone in the list; Add Patient; Edit Profile; Prescriptions and 24h chat tabs |
| Doctors | Submitted documents; Request Changes; Message doctor; doctor photos |
| Operations | Platform uptime; per-service health and latency; "vs yesterday" trends; Outages and Maintenance tabs; Log Incident; Log Complaint; Assigned-to |
| Campaigns | SMS and Email channels (push only); audience segments beyond Patients / Doctors / Both |
| Navigation | Users, Reports and Resources items |

**Patient conditions are never shown anywhere in the Admin front end** (no column, chip, filter, tile or detail field), whatever a reference image shows. The reference's condition chips were illustrative only.

### Kept and kept honest

- Finance pages stay read-only behind the Deferred banner, show real data (empty today), and keep the 20% platform charge and its mandatory note. No simulated ledger rows.
- Article bookmark/heart icon is not shown in the patient preview.

### New backend features (product owner chose to keep them)

1. **Q&A audit** — Admin lists doctor answers to patient questions, marks an answer **Approved** or **Flagged** with a note (each review kept as an audit trail), and a **Flagged answer is hidden from patients** until it is approved again. The patient's identity is not shown to the Admin; only the question text, doctor, dates and review history.
2. **Grievance appeals** — recorded by an **Admin** on the patient's behalf (no Patient app change) against a decided grievance (warned, refunded or dismissed), with a reason. An Admin then **upholds** the decision or **overturns** it (the grievance returns to review). One open appeal per grievance; every step audited.

Both get reversible migrations, ABAC actions, handler tests and Admin Web screens.

## Merged navigation

| Group | Pages |
|---|---|
| Dashboard | Command Center |
| People & Care | Patients Directory, Doctor Verification, Clinicians |
| Operations & Safety | Slot Matrix & Locks, App Errors & Incidents, Grievances (Complaints, Adjudication, Resolved, Appeals), Rx & Q&A Audit |
| Content | Articles, Topics, Quizzes, Facilities |
| Engagement | Campaigns |
| Finance (Deferred) | All Payments & Escrow, Settlements & Payouts, Payment Settings |

## Build order

1. Design tokens, shared components (sidebar, top bar, stat tile, chip, tabbed card, filter bar, data table with pagination, empty state, phone frame) and the login page.
2. Content: Articles, Topics, Quizzes, Facilities.
3. People & Care: Patients, patient record, Doctor Verification, application review, clinician detail.
4. Operations & Safety, including the Q&A audit and appeals backend.
5. Campaigns and the three Finance pages.

Each step: `tsc`, lint, Admin Web tests; backend `clippy` and tests for backend steps; screenshots for review against the boards.

## Status

| Step | State |
|---|---|
| 1. Tokens, shell, shared components, login | Done: `app/globals.css` (tokens, Plus Jakarta Sans self-hosted in `public/fonts`), `components/shell/AdminSidebar.tsx` (grouped green sidebar, breadcrumb top bar), `components/ui/afya.tsx` (PageHead, StatTile, Chip, Card, Tabs, SearchField, Pagination, EmptyState, Notice, QueuedImage), login page, white logo `public/brand/`. Landing page is now the Dashboard |
| 2. Content | Done: Articles dashboard (tiles, status tabs, topic filter, cover thumbnails), article/quiz editor (language tabs, Markdown toolbar, details column, tag chips, toggles, Write / Patient preview), Topics & Categories page (new `topics` page id), Quizzes, Facilities dashboard and editor |
| 3. People & Care | Done: `components/people/` — Patients Directory (tiles, gender/age/activity filters, no conditions, blood group or phone), patient record page replacing the old modal (summary, visit timeline, consultations, attachments, export and erase), Doctor Verification (queue tabs, application review with Approve/Reject on the existing handlers, clinician detail with consultation history from the dossier). Removed the dead "Chronic Care"/"Pediatric" filters (they never matched; category was hard-coded). Fixed "Last visit" always empty: the list now uses `last_consult_at` |
| 4. Operations & Safety (+ Q&A audit, appeals backend) | Done: `components/operations/` — Command Center (fixed "of N registered", which read the doctors list that only loads on Settlements), Slot Matrix (per-doctor rows, tiles, doctor filter), App Errors & Incidents (status tabs, severity chips, module filter), Consultation Grievances (Complaints, Adjudication queue, Resolved, Appeals; record/uphold/overturn), Rx & Q&A Audit (prescription alerts; answer review with history, approve, flag and hide). Backend: migrations `20261215000001_qna_answer_audit` and `20261215000002_grievance_appeals`, `api/v1/qna_audit_handlers.rs`, appeal handlers and `GrievanceService::{record_appeal, decide_appeal, list_appeals}`, patient Q&A feed hides flagged answers; tests `tests/cases/{qna_audit,grievance_appeals}.rs`. Appeals are limited to warned or dismissed decisions: a refund decision has already started its finance flow |
| 5. Campaigns and Finance | Done: Campaigns list and editor (EN/SW tabs, character counters, audience with eligible count, push channel only, schedule, delivery monitoring, live preview, audit history). Finance pages in deferred mode: board-style read-only panels from real values, 20% platform charge and the mandatory note on every earnings view; the enabled-mode payout and ledger logic is unchanged |
| 6. Admin Team, Activity Log, forgot password (product owner, 2026-11-14) | Built: Admin Team group in the sidebar with **Admins & Invitations** (invite by email, 5-minute single-use link, resend, revoke; Admins table shows who invited whom and when) and **Activity Log** (every Admin sees who did what and when, live every 10 s; areas, action search, only mine, load older; Admins named by email, patients and doctors by role, no record contents). Public pages `/invite` (token stripped from the address bar) and `/forgot-password` (email → 6-digit code → new password), "Forgot password?" on the login page. Backend `admin_invitation_service.rs`, `admin_team_handlers.rs`, `activity_handlers.rs`, migration `20261216000001_admin_invitations`; see `docs/API_SPEC.md` §8 |

Dialogs (incident detail, grievance adjudication, payout history, payment method, simulate) keep their content and actions with the brand header; toasts use the dark green.

## Admin request queue

The backend admits only two `/api/v1/admin/*` requests at a time (`services/backend/src/admission.rs`, the "Expensive" lane) and answers 503 beyond that; the old Admin Web already hit it from parallel polling. Admin Web now sends every `/admin` call, media upload and thumbnail through `src/lib/adminQueue.mjs` (two slots; reads turned away with 503 are retried twice with back-off; writes never retried). Thumbnails load through it as data URLs (`QueuedImage`). Test: `tests/admin-queue.test.ts`. Tab and tile counts are fetched one at a time.
