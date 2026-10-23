"""Compare this task's saved source snapshot, including pre-existing dirty files.
Only four exact UI-label expectation updates are permitted; invariants stay exact.
"""
import collections, difflib, hashlib, json, re, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent
SNAP=ROOT/'services/backend/target/patient_consultation_baseline'

def assertions(s):
    result=[]
    for m in re.finditer(r'\b(?:assert(?:_eq|_ne|_matches)?!|expect|assert(?:\.[A-Za-z_]+)?)\s*\(',s):
        start=m.start(); i=m.end(); depth=1; quote=None
        while i<len(s) and depth:
            c=s[i]
            if quote:
                if c=='\\': i+=2;continue
                if c==quote:quote=None
            elif c in '\"\'':quote=c
            elif c=='(':depth+=1
            elif c==')':depth-=1
            i+=1
        if not depth:result.append(re.sub(r'\s+','',s[start:i]))
    return collections.Counter(result)

inventory=json.loads((OUT/'test_inventory_before.json').read_text(encoding='utf-8-sig'))
assert subprocess.check_output(['git','rev-parse','pre-patient-security-validated'],cwd=ROOT,text=True).strip()==inventory['baseline_commit']
report={'baseline_tag_unchanged':True,'existing_tests_deleted':0,'existing_assertions_deleted':0,
        'assertions_weakened':0,'added_skip_ignore_markers':0,'changed_existing_test_files':[],
        'documented_ui_expectation_updates':[],'added_assertions':0,'unchanged_assertions':0}
diffs=[]
allowed={"find.text('CallPatient')":"find.text('Startvideoconsultation')",
         "find.text('Chat')":"find.text('Follow-upmessages')",
         "find.text('WriteRx')":"find.text('Writeprescription')"}
for relative,meta in inventory['files'].items():
    old_path=SNAP/relative; new_path=ROOT/relative
    assert old_path.exists(),relative
    assert new_path.exists(),('Deleted original test source',relative)
    old=old_path.read_text(encoding='utf-8-sig');new=new_path.read_text(encoding='utf-8-sig')
    assert hashlib.sha256(old_path.read_bytes()).hexdigest()==meta['sha256'],('Snapshot modified',relative)
    a,b=assertions(old),assertions(new)
    report['unchanged_assertions']+=sum((a&b).values())
    removed=a-b; added=b-a
    for statement,count in removed.items():
        replacement=None
        if relative=='apps/mobile/test/doctor_widgets_test.dart':
            for before,after in allowed.items():
                if before in statement:replacement=statement.replace(before,after);break
        assert replacement and added[replacement]>=count,('Unapproved assertion removal/change',relative,statement)
        added[replacement]-=count
        report['documented_ui_expectation_updates'].append({'file':relative,'old':statement,'new':replacement,'count':count,
          'reason':'Requested modality-specific consultation actions; identical findsOneWidget and width checks retained. Added negative opposite-modality assertions.'})
    report['added_assertions']+=sum(added.values())
    delta=list(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='before/'+relative,tofile='after/'+relative))
    if delta:
        report['changed_existing_test_files'].append(relative);diffs.extend(delta)
        for line in delta:
            if line.startswith('+') and not line.startswith('+++'):
                assert not re.search(r'#\[ignore\]|\bskip\s*:|\b(?:test|it|describe)\.skip\s*\(',line),('Added skip',relative,line)
    # All named Rust regression cases must remain, regardless of formatting.
    for name in re.findall(r'\bfn\s+(test_[A-Za-z0-9_]+)\s*\(',old):
        assert re.search(r'\bfn\s+'+name+r'\s*\(',new),('Deleted case',relative,name)
new_files=[]
for folder,pattern in [('services/backend/tests','*.rs'),('apps/mobile/test','*.dart'),('apps/mobile_patient/test','*.dart'),('apps/doctor-web/tests','*.test.ts'),('apps/admin-web/tests','*.test.ts')]:
    for p in (ROOT/folder).rglob(pattern):
        relative=p.relative_to(ROOT).as_posix()
        if relative not in inventory['files']:
            new_files.append(relative); report['added_assertions']+=sum(assertions(p.read_text(encoding='utf-8-sig')).values())
report['new_test_files']=sorted(new_files)
(OUT/'assertion_preservation.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
(OUT/'test_changes_from_task_snapshot.patch').write_text(''.join(diffs),encoding='utf-8')
print(json.dumps(report,indent=2))
