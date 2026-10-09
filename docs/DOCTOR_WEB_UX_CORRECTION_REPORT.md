# Doctor Web targeted UX corrections — 2026-10-07

The preview at **http://127.0.0.1:3001** runs build **NSx9VY1Mkxp3QUUfkUpDg**. Dashboard/account/settings were completed and verified before the dedicated Patient Chats routing pass. These are the user-approved `WEB_UX_IMPROVEMENT_OVER_PROTOTYPE` changes; prototype typography, colors, cards, borders, badges and desktop shell remain the visual language.

## Verified changes

- Dashboard now has Today's Consultations, Next Consultation, Eligible Earnings and Open Q&A, followed by one upcoming/active consultation list. No modality panel, duplicate sidebar queue, unsupported aggregate cards, global Instant Rx or unsupported recent-prescription section.
- Top-right Doctor identity opens a compact account menu. Settings is a full desktop workspace with Profile, Professional Details, Availability, Payout and Security. The sidebar identity card is static. View profile opens the read-only Professional section rather than another duplicate surface.
- Display name is the only editable profile field. Credential, hospital, qualification and verification values are read-only. Saving gives local loading/validation/success feedback. Duty changes wait for the existing server endpoint. Payout displays the current masked destination/readiness or truthful empty state; real provider connection remains deferred.
- Existing Patient Chats is a primary sidebar destination at `/doctor/chats`, with a fluid two-pane inbox/thread workspace. Telehealth is not highlighted on Chats. Browser back/forward, reload, cold login and MFA restore the Chat section. Telehealth retains its contextual In-call Chat using the same `ChatThread` and backend contract.
- All/Active/Read-only filters reflect server lock and expiry data. Unsupported Urgent/Follow-up/Refill classifications are removed. No fabricated unread badge or preview. Global appointment search and conversation search remain separate.
- Doctor camera capture is removed. File selection/previews and per-page retry/remove remain. The live backend rejects persistence with **409 RESOURCE_CONFLICT: “Document storage is not configured. Upload is unavailable.”** This is `STORAGE_CONFIGURATION_REQUIRED`; server persistence has not been verified as successful. A prescription still requires an owned, authorized consultation.

## Current backend mapping

| Surface/action | Current contract | What is rendered |
| --- | --- | --- |
| Today's count / operational list / next consultation | GET `doctor/appointments?limit=100&offset=…` | Count/filter from the complete authorized listing; server's nearest-upcoming ordering retained; next time comes from authoritative schedule. No separate business state calculation. |
| Earnings | GET `doctor/wallet/summary` | `final_net` as lifetime eligible net, `total_gross`, `withheld_fee`; KES formatting only; no frontend financial arithmetic or claim of payout receipt. |
| Open Q&A | GET `doctor/qna/questions?status=OPEN&limit=100&offset=0` | Returned open-question count; `100+` at the page cap, not an invented exact total. |
| Recent prescription dispatch aggregate | No Doctor-wide dispatch-list/aggregate contract | Omitted; `BACKEND_DATA_NOT_AVAILABLE` documented instead of exposed on Dashboard. No per-appointment request fan-out. |
| Profile | GET/PATCH `doctor/profile` | PATCH accepts only `full_name`; protected fields are information rather than disabled inputs. See field matrix in UX decisions. |
| Availability | GET/PUT `doctor/duty` | Server-confirmed own duty value; no optimistic success. |
| Payout | GET `doctor/wallet/payout-methods` | Masked destination, binding, verification and payout-capable values. No raw financial detail collection or simulated real payout. |
| Security/account | Existing BFF logout → POST `auth/logout` | Current session logout; no unsupported password/MFA/session-management controls invented. |
| Chat inbox | GET `doctor/chats?limit=100&offset=…` | Patient display identity, authorized appointment context, last-message timestamp, lock/expiry. |
| Thread | GET/POST `chat/{conversation_id}/messages` | Escaped plain text, current sender/time; idempotent sending; backend expiry/ownership remains authoritative. Composer maximum corrected to current 2,000-character backend limit. |
| Chat unread/preview/classifications | Not supplied by current inbox DTO; no supported mark-read/count or category API | `UNREAD_COUNT = NOT_SUPPORTED`; previews/categories omitted. Raw `is_read` on message rows is not a trustworthy aggregated Doctor inbox count. |
| Prescription | POST `consultations/{appointment_id}/prescription` | Same base64/mime/notes/idempotency contract; failed page status retains local preview; no fake persistence. |

Routes are browser presentation only. History stores an allowlisted page name; it does not persist messages, patient records, tokens or document URLs. `/doctor/chats` is served directly by Next and restores the existing HttpOnly BFF session before fetching data.

## Evidence and limitations

Typecheck, lint, project format check, **11 unit tests** and production build PASS. **30 browser/API checks** pass on the same final build: prescription 7, Dashboard/settings 13, Patient Chats 10. No browser exceptions or unexpected 5xx occurred. Screenshots contain controlled development accounts/data.

| Evidence | Results |
| --- | --- |
| [Prescription checks](audit_evidence/doctor_web/rx-upload/results.json) | Picker opens, two local previews, real storage error, retry/remove, no persisted record on failure, camera absent, no global context-free prescription action. |
| [Dashboard/settings checks](audit_evidence/doctor_web/dashboard-settings/results.json) | Compact summaries, backend queue/KES, account keyboard/focus, full settings, real save/restore, validation, mass-assignment rejection, duty/restore, payout deferral, security/logout, four responsive widths. |
| [Patient Chats checks](audit_evidence/doctor_web/patient-chats/results.json) | Direct sidebar/URL, real messages/send, filters, expiry, two-Doctor list/read/send ownership, in-call separation, back/forward/reload, distinct search, cold login/MFA. |
| [Source/build summary](audit_evidence/doctor_web/dashboard-settings/final-summary.json) | Same build for all three walkthroughs; source hashes confirm mobile/backend unchanged. |

Chat expiry testing temporarily expired one existing synthetic Doctor conversation; its original expiry/lock values were restored in `finally`. A new synthetic test message was deleted by its exact returned ID after verification. Display-name and duty changes were restored. No ordinary Doctor record, financial state or infrastructure was changed by these tests.

Rendered Dashboard/account/all five Settings sections were inspected. Dashboard/settings evidence covers 1920×1080, 1440×900, 1024×768 and 390×844. Chat evidence also covers 1280×720, selected/unselected/empty/read-only threads and contextual In-call Chat. The final visual review corrected spacing and text separators before acceptance. This is scoped visual acceptance of authorized UX changes, not an assertion that every original prototype screen has strict pixel parity.

## Dashboard acceptance

```text
DUPLICATE_CONSULTATION_COUNTERS = REMOVED
SIDEBAR_PATIENT_QUEUE_REDUNDANCY = REMOVED
CLINICAL_MODALITY_REDUNDANCY = REMOVED
PRESCRIPTION_REDUNDANCY = REMOVED
INSTANT_RX = REMOVED
UNAVAILABLE_DEVELOPER_CARDS = 0
DOCTOR_TOOLS_DUPLICATE_NAV = REMOVED
EMOJI_PLACEHOLDERS = 0 (Dashboard and shared empty/error graphics)
TODAYS_CONSULTATIONS = BACKEND_WIRED
UPCOMING_QUEUE = BACKEND_WIRED_UPCOMING_FIRST
EARNINGS = BACKEND_WIRED_KES
OPEN_QA_OR_ATTENTION = BACKEND_WIRED
PRODUCTION_BUILD = PASS
DASHBOARD_VISUAL_REVIEW = PASS
```

## Account/settings acceptance

```text
OLD_PROFILE_MODAL_REMOVED = YES
TOP_RIGHT_ACCOUNT_MENU = PASS
SIDEBAR_PROFILE_ROLE = STATIC_IDENTITY
SETTINGS_FULL_PAGE = PASS
PROFILE_SECTION = PASS
PROFESSIONAL_SECTION = PASS
AVAILABILITY_SECTION = PASS
PAYOUT_SECTION = PASS (readiness/empty state; real provider setup remains deferred)
SECURITY_SECTION = PASS (current session logout; unsupported controls omitted)
WALLET_DUPLICATION = REMOVED
DUTY_LOCATION = CLEAR
LOGOUT_LOCATION = CLEAR
DOCTOR_EDITABLE_FIELDS_MATCH_BACKEND = YES
ADMIN_CONTROLLED_FIELDS_READ_ONLY = YES
WEB_UX_IMPROVEMENTS_DOCUMENTED = YES
PRODUCTION_BUILD = PASS
VISUAL_REVIEW = PASS
```

Wallet gained its own visible primary navigation during the preceding Dashboard/settings pass to retain the existing financial domain after removing Doctor tools/Profile actions. The subsequent Chat-only pass did not redesign Wallet or change that decision.

## Patient Chats acceptance

```text
PATIENT_CHAT_PRIMARY_NAV = PASS
PATIENT_CHAT_ROUTE = /doctor/chats
PATIENT_CHAT_HIDDEN_IN_DOCTOR_TOOLS = NO
DOCTOR_TOOLS_CHAT_REMOVED = YES
TELEHEALTH_ACTIVE_WHILE_ON_CHAT_PAGE = NO
TELEHEALTH_CHAT_SEPARATION = PASS
IN_CALL_CHAT_RETAINED = YES
CONVERSATION_LIST = BACKEND_WIRED
CHAT_THREAD = BACKEND_WIRED
READ_ONLY_EXPIRY = PASS
UNREAD_COUNT = NOT_SUPPORTED
CROSS_DOCTOR_IDOR = PASS (chat list/read/send)
CHAT_LIST_DESKTOP_LAYOUT = YES
CHAT_THREAD_DESKTOP_LAYOUT = YES
MOBILE_WIDTH_CHAT_CONTAINER = NO
DUPLICATE_PRIMARY_CHAT_NAVIGATION = NO
REAL_BACKEND_MESSAGES = YES
MOBILE_APP_MODIFIED = NO
BACKEND_MODIFIED = NO
PRODUCTION_BUILD = PASS
VISUAL_REVIEW = PASS
```

## Asset and host integrity report

```text
DOCTOR_MOBILE_FILES_CHANGED = NONE
BACKEND_FILES_CHANGED = NONE
LOCAL_ASSETS_REUSED = 4
OPEN_SOURCE_ASSETS_DOWNLOADED = 0
DOWNLOADED_ASSET_SOURCES = NONE
LICENSES_VERIFIED = YES (no new third-party licensing dependency)
UNLICENSED_ASSETS_USED = 0
REMOTE_HOTLINKED_ASSETS = 0
SVG_SECURITY_CHECK = PASS
HOST_SOFTWARE_INSTALLED = NONE
HOST_SOFTWARE_UNINSTALLED = NONE
WINDOWS_SERVICES_CREATED = NONE
PATH_MODIFIED = NO
REGISTRY_MODIFIED = NO
FIREWALL_MODIFIED = NO
NEW_DOCKER_INFRASTRUCTURE = NONE
DOCKER_INFRASTRUCTURE_ADDED = NONE
MINIO_REINTRODUCED = NO
BROWSER_INSTALLED = NO
PLAYWRIGHT_BROWSER_DOWNLOADED = NO
ANDROID_TOOLING_INSTALLED = NO
GLOBAL_PACKAGES_INSTALLED = NONE
NO_INSTALLATION_PERFORMED = YES
```

The targeted corrections are complete. Whole-application verdict remains **DOCTOR_WEB_PARTIAL**, including the genuine private-storage and external-provider deferrals documented previously. No unrelated architecture or product work was reopened.
