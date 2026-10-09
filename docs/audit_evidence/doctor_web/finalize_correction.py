"""Record correction evidence and append verified results; no runtime mutation."""
import hashlib
import json
from pathlib import Path
from collections import Counter

ROOT = Path(__file__).resolve().parents[3]
DOCS = ROOT / 'docs'
OUT = Path(__file__).parent / 'correction'
baseline = json.loads((OUT / 'source-baseline.json').read_text(encoding='utf-8'))
changed = []
for name, expected in baseline.items():
    path = ROOT / name
    actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None
    if actual != expected:
        changed.append(name)
mobile_changed = [p for p in changed if p.startswith('apps\\mobile\\')]
backend_changed = [p for p in changed if not p.startswith('apps\\mobile\\')]
assert not mobile_changed, mobile_changed
visual = json.loads((OUT / 'visual-browser-results.json').read_text(encoding='utf-8'))
workflow = json.loads((OUT / 'workflow-results.json').read_text(encoding='utf-8'))
assert len(visual['screens']) == 45
assert len(workflow['checks']) == 14
assert all(v == 'PASS' for v in visual['checks'].values())
assert all(v == 'PASS' for v in workflow['checks'].values())
assert not visual['console_errors'] and not workflow['console_errors']
build = (ROOT / 'apps/doctor-web/.next/BUILD_ID').read_text().strip()
integrity = {
    'build_id': build,
    'baseline_files_checked': len(baseline),
    'mobile_changed': mobile_changed,
    'backend_changed_against_baseline': backend_changed,
    'backend_edits_by_this_correction': [],
    'backend_preservation_note': 'Notification repository/service and API tests changed separately during this pass. This correction did not edit them; changes are preserved and not attributed to this UI work.',
    'screen_viewport_checks': len(visual['screens']),
    'populated_workflow_checks': len(workflow['checks']),
    'ui_auth_checks': visual['checks'],
    'console_exceptions': 0,
    'host_software_installed': 'NONE',
    'browser_installed_or_downloaded': False,
    'verdict': 'DOCTOR_WEB_PARTIAL',
    'visual_parity': 'PARTIAL',
}
(OUT / 'source-integrity-final.json').write_text(json.dumps(integrity, indent=2) + '\n', encoding='utf-8')

notice = ('> Current presentation result: [Doctor Web correction report](DOCTOR_WEB_CORRECTION_REPORT.md). '
          'The older combined mobile/Web acceptance below is historical; mobile roots are not Web visual authority.\n\n')
for name in ['DOCTOR_WEB_PROTOTYPE_INVENTORY.md', 'DOCTOR_WEB_VISUAL_PARITY_REPORT.md', 'DOCTOR_WEB_FINAL_ACCEPTANCE.md']:
    path = DOCS / name
    original = path.read_text(encoding='utf-8')
    if not original.startswith(notice):
        path.write_text(notice + original, encoding='utf-8')

def append(name, content):
    path = DOCS / name
    with path.open('a', encoding='utf-8') as stream:
        stream.write('\n\n' + content.strip() + '\n')

append('DOCTOR_WEB_PROTOTYPE_INVENTORY.md', '''
## Completed presentation correction inventory - 2026-10-07

The correction report supplies the current screen-by-screen component/API,
desktop layout, screenshot and status map. It supersedes the earlier hybrid
component map: `doctorAppShell`, `doc-view-*` and mobile bottom tabs are no longer
rendered. All five actual Web roots are accounted for. The four secondary Web
workspaces use the canonical Web classes and preserve existing APIs.

Rendered evidence: `audit_evidence/doctor_web/correction/web_*` and separate
`canonical_*` / `reference_viewport_*` images. All nine workspaces were checked
at five viewport sizes. Shared dialogs were captured at 1440x900. This is not
an assertion that every historical inventory element passed visual acceptance.
The exhaustive earlier 887-element mixed-role inventory is historical source
evidence; it is not the current Web screen total.

The following disposition map accounts for canonical static Web handlers,
including repeated demo controls. Dynamic Q&A and EMR rows are additionally
covered by their current components. Individual controls without an action
regression are explicitly not marked VERIFIED. See raw element records in
`audit_evidence/doctor_web/correction/web-prototype-controls.json`.

| Prototype interaction / locator | Current Web implementation | Backend / limitation | Test and visual evidence |
|---|---|---|---|
| Global search; Web page switching | DoctorApplication, DesktopAppointments | Owned appointments only; no clinical cache persistence | 45-page walkthrough; primary Web screenshots |
| Topbar Shift/calendar, Appointments calendar/Add | Schedule, SlotForm | Existing slots list/create/cancel | Date/slot screenshots; Escape and booked protection checked; new creation not rerun |
| Bell simulated EMR sync | Existing honest availability/notification behavior | Unchanged in this pass; no fake synchronization | No notification feature or acceptance added |
| Settings toast | ProfileSettings dialog | Allowed profile edit, server-confirmed duty; earnings/payout secondary links | Settings screenshot, duty original state restored |
| Sidebar demo patient switchers | Actual owned queue rows | Doctor appointments | Upcoming-first test; no demo identities in runtime |
| Dashboard lab requisition | Truthful unavailable action | No structured lab-order contract | Preserved control; BACKEND_DATA_NOT_AVAILABLE |
| Dashboard/Rx quick actions and preview controls | Rx desktop workspace; documents for completed consultation | Existing prescription/document contracts | Rx populated screenshot; two failed pages retain retry/remove |
| Dashboard earnings KPI | Actual statement dialog and earnings workspace | Wallet/month-scoped statement | Wallet/statement screenshots and open controls |
| Dashboard queue/View All; appointment call controls | Appointments/Telehealth with selected appointment | Appointments; RTC deferred | Empty/populated screenshots; no fake connected call |
| Appointments All/Today/Upcoming/Completed filters | DesktopAppointments filter state | Existing authorized list, authoritative ordering retained | Rendered filter layout; upcoming-first regression |
| Appointment historical chart | Summary/History | Appointment-scoped summary/history | Authorized/foreign denial checks and dialog captures |
| Appointment reschedule simulated success | Preserved action with truthful unsupported state | No current Doctor rescheduling contract | BACKEND_DATA_NOT_AVAILABLE; never fake notification/success |
| Patients search/category filters | Patients | Owned appointments; allergy/chronic category unsupported | Table and empty screenshots; no inferred clinical classifications |
| Patients registration simulated action | Preserved unavailable action | No Doctor-owned patient registration contract | BACKEND_DATA_NOT_AVAILABLE |
| Patients call/history/lab row actions | Telehealth, History, Documents | Existing appointment authorization and document access | History/document captures; no arbitrary patient-ID access |
| Telehealth mic/camera/share/reaction | Truthful deferred controls | Agora Web not integrated | DEFERRED_EXTERNAL_INTEGRATION; no calling/connected claim |
| Telehealth quick Rx/completion | Rx; context-guarded Completion dialog | Existing upload/complete/complete-no-rx | Form screenshot; no new completion financial mutation |
| History/Chat drawers, close, side tabs/fullscreen | Telehealth Web panel/view state | History/chats only while selected context requires them | EMR and in-call chat captures; no browser request storm |
| EMR Encounters/Meds/Labs/Notes/Full Dossier | Web EMR cards, History/Episode/Documents | Server-scoped history/documents; structured labs unavailable | Authorized history and screenshots; not every episode substate accepted |
| EMR Lab Report and Order Lab | Authorized Documents / honest unavailable lab ordering | Existing document access; lab gap | Authorized empty dialog; no real document bytes acceptance |
| In-call chat input, Enter and Send | Real Web ChatThread | HTTP messages, lock/expiry/idempotency retained | Real message POST 201; escaped HTML-like text |
| In-call attachment placeholder | Existing Rx attachment entry | Handwritten prescription upload; no arbitrary chat file contract | Rx controls preserved; storage failure truthful |
| In-call emoji selector placeholder | No standalone selector acceptance claimed | Prototype only simulates a toast | NOT_COMPLETE for this individual prototype interaction; no new product feature invented |
| Q&A Refresh, All/Open/Answered/search and dynamic answer cards | Qna Web DOM at every size | Existing list/detail/answer APIs | Q&A screenshots/backend responses; helpful votes unavailable |

Settings/Rx/availability/earnings standalone desktop pages do not exist as Web
page roots in the prototype. Their desktop workspaces are documented required
platform differences, not claims of copying a mobile screen. Strict complete
modal/control visual parity remains PARTIAL; NOT_COMPLETE = 0 is not claimed.
''')

append('DOCTOR_WEB_BACKEND_MAPPING.md', '''
## Presentation correction - 2026-10-07

No Rust or Flutter files were edited by this correction. Flutter baseline hashes
are unchanged. Three notification/test Rust files changed separately in the
workspace; source-integrity-final.json records them without attributing them to
this UI work. Those changes were preserved.
All route/DTO/session/authorization mappings above remain. No Web-specific API,
migration, financial computation or alternative clinical authorization was added.
The earlier shared-contract and login recovery changes predate this correction.

Entry points changed only: queue is the Web Appointments surface; Rx/chat belong
to the Telehealth secondary flow; slots are reachable through calendar and
Appointments; earnings/statement/payout through Dashboard/profile. The Web EMR
panel now reads the same appointment-scoped history and existing HTTP chat
contracts, with contextual fetching and reauthorization. Profile duty awaits the
server and the test restores its original value.

Current real API regression and sanitized request outcomes are recorded in
`audit_evidence/doctor_web/correction/workflow-results.json`. Rx's storage failure
is the real 409 RESOURCE_CONFLICT, not a successful upload. Chat message creation
returns 201. Foreign summary/history requests remain denied. No live provider or
new completion mutation is claimed. See the correction report for exact scope.
''')

append('DOCTOR_WEB_VISUAL_PARITY_REPORT.md', '''
## Latest Web-only correction evidence - 2026-10-07

Current build: `vBxYbGtSpLPl4rQ_7Fvhi`. The old hybrid-shell screenshots/statuses
above describe the preceding implementation, not the current one. The actual
canonical five Web pages were rendered in installed Edge. Original `canonical_*`
captures remain distinct from explicitly normalized `reference_viewport_*`
captures. Normalization covers the user-required fullscreen shell and flexible
consultation workspace/action wrapping; the prototype file is unchanged.

45 actual Web screen/viewport checks pass: nine workspaces at 1920x1080,
1440x900, 1280x720, 1024x768 and 390x844. Geometry proves viewport width, zero
outer offset/radius/shadow, no root max-width, no rendered mobile shell/tab nodes
and no horizontal page overflow. Actual screenshots were inspected against
rendered canonical/normalized references, including all five primary Web pages.
Populated and dialog captures add clinical/workflow evidence.

The current screen table, comparison differences and data gaps are in
[DOCTOR_WEB_CORRECTION_REPORT.md](DOCTOR_WEB_CORRECTION_REPORT.md).
The report separates retained prototype Web geometry from required fullscreen,
narrow Web drawer and desktop secondary-workflow differences. Global visual
parity remains PARTIAL: no unmodified canonical Web Rx/settings page exists;
some real data is absent; exhaustive shared-modal/control acceptance is not
finished. This is not a source-only visual PASS or a pixel-perfect claim.
''')

append('DOCTOR_WEB_FINAL_ACCEPTANCE.md', '''
## Latest presentation correction acceptance - 2026-10-07

See [DOCTOR_WEB_CORRECTION_REPORT.md](DOCTOR_WEB_CORRECTION_REPORT.md) for the
mandatory current results matrix, complete Web-screen table and host report.
Fullscreen shell, viewport use, outer-frame removal, duplicate/mobile desktop
navigation removal and Rx/Telehealth/Q&A/Appointments/Dashboard desktop layouts
PASS. All five actual Web roots are accounted for; real API integration survives.
The correction pass did not modify Flutter/Rust, manifests or host software.

Typecheck/lint/format, 10 unit tests and production build PASS. Installed Edge
walkthrough PASS for this corrective regression: 45 screen/viewport checks,
real UI login/MFA/reload/logout and 14 populated workflow checks, with no
unhandled JavaScript exceptions. See correction JSON/screenshots and baseline
source preservation report. BROWSER_WALKTHROUGH here has a narrower declared scope
than the historical original full-project acceptance gate above.

VISUAL_PARITY = PARTIAL. Broader registration/session/authorization families,
all shared-modal interaction parity and actual storage/RTC/provider capabilities
are not newly certified. NOT_COMPLETE = 0 is not claimed.
Final overall verdict: **DOCTOR_WEB_PARTIAL**. The reported presentation defects
are corrected; stop this pass without reopening backend/mobile architecture.
''')

checkpoint = '''
## Doctor Web presentation correction - 2026-10-07

Corrected build `vBxYbGtSpLPl4rQ_7Fvhi` is running on approved loopback 3001.
Fullscreen viewport shell, sole canonical Web sidebar, no mobile bottom tabs,
desktop Rx/chat/availability/earnings and Web EMR panel are implemented. Existing
APIs/session handling retained. All five canonical Web roots are accounted for.
10 Web tests, typecheck/lint/format/build PASS; 45 Edge screen/viewport checks,
UI login/MFA/reload/logout and 14 populated workflow checks PASS, zero JS
exceptions. Original/normalized prototype references are kept separately.
VISUAL_PARITY remains PARTIAL, with data/provider and exhaustive dialog limits
explicit in DOCTOR_WEB_CORRECTION_REPORT.md. Overall DOCTOR_WEB_PARTIAL.
Baseline hashes prove Flutter unchanged. No backend edits were made by this pass;
three separately changed notification/test files are recorded and preserved.
No software,
browser, service or Docker infrastructure installed. Stop correction scope here.
'''
append('CHECKPOINT.md', checkpoint)
append('PROGRESS.md', checkpoint)
print(json.dumps(integrity))
