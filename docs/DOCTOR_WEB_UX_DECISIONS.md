# Doctor Web UX decisions

## 2026-10-07 ? User-authorized desktop hierarchy correction

Status: Dashboard/account/settings build and 13 real-browser checks PASS. Rendered Dashboard, account popover and all five Settings sections were inspected at desktop sizes; Profile/Dashboard responsive evidence also covers 390px. Build: lGgKwZC70Jn19ywa_XOwV. Patient Chats follow-up subsequently passed 10 checks. Final build NSx9VY1Mkxp3QUUfkUpDg passes all 30 combined browser checks; the prior build identifier in this sentence records the earlier checkpoint.

All decisions below are `WEB_UX_IMPROVEMENT_OVER_PROTOTYPE`. They are intentional and must not be reverted to the prior modal/hybrid hierarchy merely for prototype parity.

| Prototype element | Problem | Web decision | Business impact | Backend contract | Rationale |
| --- | --- | --- | --- | --- | --- |
| Dashboard KPI cards | Unsupported metrics and duplicate empty consultation counts | Today's consultations, next consultation, eligible net earnings, OPEN Q&A; one upcoming/active list | Presentation only; no financial calculation | Own appointment listing; wallet summary `final_net`, `total_gross`, `withheld_fee`; status=OPEN Q&A | Doctor can identify work next without developer placeholders |
| Instant Rx Dashboard button | No authorized consultation anchor | Remove global CTA; prescription starts from an appointment | Ownership/time-window unchanged | Consultation-scoped prescription POST | No context-free prescription workflow exists |
| Recent Prescription Dispatches | No authorized cross-consultation dispatch listing in current API | Omit unsupported Dashboard section | No invented records or per-appointment request fan-out | BACKEND_DATA_NOT_AVAILABLE | Missing aggregates belong in documentation |
| Sidebar Patient Queue switcher | Same active appointment set as Dashboard/appointments | Remove redundant counter/switcher | No queue capability removed | Own appointments | One count and one operational list |
| Doctor tools | Duplicates global destinations and buries chat | Remove global tools dropdown; dedicated Patient Chats primary item | Existing messaging remains unchanged | Own chats and message API | Primary workflows stay visible |
| Top-right Doctor badge | Opens large combined settings modal | Compact account popover | Current session/profile only | Own profile; BFF logout | Account menu owns account actions |
| Sidebar Doctor Profile card | Competing settings entry | Static identity display | None | Own profile | Clear identity role |
| Settings | Giant combined modal | Full desktop page with five local sections | Same Doctor-editable field only | GET/PATCH doctor/profile; GET/PUT doctor/duty; GET payout-methods | Configuration is grouped by purpose |
| View profile | Would duplicate a new professional page | Opens read-only Professional section of Settings | No extra data access | Own profile | One professional identity authority |
| Wallet buttons in Profile strip | Financial/navigation duplication | Wallet stays a domain; Payout has only contextual View Wallet link | No frontend financial state changes | Wallet APIs | Separate earnings from configuration |
| In-call Chat | Must not own entire message inbox | Retain contextual thread in Telehealth; dedicated inbox under Patient Chats | One existing message state machine | Same chat messages endpoint | Distinguish consultation from ongoing messaging |
| Camera Capture | User asked to remove Doctor-side capture | File selection only; same 1?5 prescription page flow | No fake persistence | Existing consultation prescription endpoint | Explicit user preference |

## Current profile editability contract

| Field | Displayed | Editable by Doctor | Backend endpoint | Admin controlled | Read only |
| --- | --- | --- | --- | --- | --- |
| Display name (`full_name`) | Yes | Yes, localized Save | GET/PATCH doctor/profile (PATCH permits only full_name; denies unknown fields) | No | No |
| Initials/avatar | Initials only | No photo-upload action | GET doctor/profile, initials presentation | No image contract | Yes |
| Specialty | Yes | No | GET doctor/profile | Protected professional data | Yes |
| Hospital | Yes | No | GET doctor/profile | Protected professional data | Yes |
| Qualifications | Yes | No | GET doctor/profile | Protected professional data | Yes |
| KMPDC number / authority | Yes | No | GET doctor/profile | Yes | Yes |
| Verification status | Yes | No | GET doctor/profile | Yes | Yes |
| Email/phone | No (not supplied by own profile DTO) | No | No current Doctor profile edit contract | Not exposed here | Not rendered |
| Duty | Availability section | Yes; wait for server | GET/PUT doctor/duty | Server ABAC/verification restrictions | No |
| Payout destination/readiness | Payout section, masked only | Provider connection deferred; no raw destination collection | GET doctor/wallet/payout-methods | Provider/server controlled | Yes until external integration |
| Password/MFA/session management | Current session logout only | Logout | Existing BFF auth/logout | Existing auth session rules | No fabricated controls |

No Doctor credential self-verification, mass assignment, raw payout details, password/MFA management API, article API or storage substitute is introduced.

## Dedicated Patient Chats route follow-up ? complete

Primary sidebar Patient Chats ? `/doctor/chats`; browser history/back/forward/reload and session recovery verified. The rest of the existing single-workspace routing is preserved, with allowlisted page-name history state and no clinical data in that state. In-call Chat retains only the selected consultation thread, sharing ChatThread; it does not own the inbox. Unsupported clinical classifications removed; no unread/preview claims from incomplete DTO data. Message input now follows the backend 2,000-character cap. No new backend, mobile presentation or chat state machine added. Final evidence: [UX correction report](DOCTOR_WEB_UX_CORRECTION_REPORT.md).


## 2026-10-07 ? Real slot blocking and semantic cards

WEB_UX_IMPROVEMENT_OVER_PROTOTYPE: schedule slots use state-dependent identity/icon/actions. Available slots have no patient, so a question-mark avatar was misleading. Blocking is reversible availability configuration, cancellation is removal; neither may modify a patient booking. Web shows supported statuses and authoritative actions; mobile receives a minimal compatibility filter so BLOCKED does not become a fabricated booked-patient card. New business capability is explicitly user-approved; migration/DTO/transition tests protect it.

Contract, tests, mobile exception, telemetry findings and full report: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).
