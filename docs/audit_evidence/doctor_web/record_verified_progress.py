"""Record completed checks only; preserve earlier checkpoint/progress content."""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];EVIDENCE=Path(__file__).parent
verification=json.loads((EVIDENCE/'verification-current.json').read_text())
verification['backend_ready']='HELLODOCTOR_BACKEND_READY'
verification['schema_versions']={'source':'20261019000001','binary':'20261019000001','database':'20261019000001'}
verification['latest_browser']=json.loads((EVIDENCE/'workflow/completion-current-results.json').read_text())
(EVIDENCE/'verification-current.json').write_text(json.dumps(verification,indent=2)+'\n',encoding='utf-8')
mapping=ROOT/'docs/DOCTOR_WEB_BACKEND_MAPPING.md'
source=mapping.read_text(encoding='utf-8').replace('Full rerun and current live binary verification remain pending until\ntheir evidence is recorded.','The second full run passed 52 tests and exposed an older historical fixture\nthat completed before T-15; correcting its fixture timing makes the targeted\nhistory regression pass. All 53 distinct tests have passed across these runs;\na single full run after the final test-only fixture correction is not claimed.\nCanonical startup verifies source/binary/database at 20261019000001. The latest\nreal browser check confirms completion status and retry without double credit.')
mapping.write_text(source,encoding='utf-8')
acceptance=ROOT/'docs/DOCTOR_WEB_FINAL_ACCEPTANCE.md'
source=acceptance.read_text(encoding='utf-8')
source=source.replace('latest canonical dialog walkthrough pending','latest canonical dialog opens without browser exceptions; visual parity pending')
source=source.replace('targeted current API regression passes; full rerun/live current binary pending','targeted current API regression and current-binary browser completion/retry pass; full acceptance remains pending')
source=source.replace('API first run 52 passed/1 failed, corrected targeted test passed, full rerun pending','API second run 52 passed/1 failed on an older early-timing fixture; corrected targeted history test passes (53 distinct API tests pass across runs); one final full green run not claimed')
source += '\nCurrent-binary follow-up: `workflow/completion-current-results.json` verifies\nUI success agrees with authoritative completion, same-reason retry has no\nfinancial change, statement canonical dialog opens, and slot generator\ndismisses with Escape; zero browser exceptions. Source/binary/database migration\nversions all equal 20261019000001 and canonical startup reports ready.\n'
acceptance.write_text(source,encoding='utf-8')
entry='''

## 2026-10-07 — Doctor Web implementation checkpoint

Created one Doctor Web project at `apps/doctor-web`, using the canonical
`prototype/index.html` and current Rust contracts. Flutter application source
is unchanged; Flutter analyze and all 50 tests pass. No host software, browser,
global package, Windows service or infrastructure was installed; MinIO remains
absent. The user approved only the loopback preview on 127.0.0.1:3001.

Current Web production build, typecheck, local format/safety checks and 8 Node
tests pass. Installed Edge was used for real MFA/session/core API checks and 55
canonical visual references across five viewport widths. Visual parity is not
accepted: 11 screen roots and 20 explicit subflows still require complete
rendered comparison and final control acceptance. Current verdict is
DOCTOR_WEB_PARTIAL; this is not completion of the requested exact replication.

A live reproducible shared defect required a narrow Rust service change:
complete-no-rx previously returned success while the appointment remained
CONFIRMED. It now runs the existing authoritative completion function in the
no-Rx transaction, preserving DTOs and existing clinical-document authorization.
No schema migration or financial redesign was introduced. Format/clippy and
29 Rust library tests pass. The second full API run passed 52/53, exposing an
old early-timing historical fixture; that fixture's corrected targeted test
passes. All 53 distinct API tests pass across runs; a final single full green
run is not claimed.

Backend canonical startup restored readiness and verified source, binary and
database migration versions all 20261019000001. The latest production Web
browser test verifies authoritative no-Rx completion, unchanged earnings on
retry, statement dialog and slot-generator Escape dismissal, with no browser
exceptions. Full visual/session/prototype coverage remains unfinished.

See DOCTOR_WEB_PROTOTYPE_INVENTORY.md, DOCTOR_WEB_BACKEND_MAPPING.md,
DOCTOR_WEB_VISUAL_PARITY_REPORT.md and DOCTOR_WEB_FINAL_ACCEPTANCE.md for
current evidence and concrete remaining work.
'''
for name in ['CHECKPOINT.md','PROGRESS.md']:
    file=ROOT/'docs'/name
    with file.open('a',encoding='utf-8') as handle:handle.write(entry)
print('Recorded verified results; no visual or full acceptance PASS asserted.')
