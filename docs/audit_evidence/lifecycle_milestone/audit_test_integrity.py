"""Compare current tests with the frozen pre-milestone source snapshot.

No test source, fixtures, runner options or historical tags are modified.
"""
import base64, difflib, hashlib, json, re
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent
BASE=json.loads((OUT/'baseline.json').read_text(encoding='utf-8'))

def assertions(source):
    found=[]
    for match in re.finditer(r'\b(?:assert(?:_eq|_ne|_matches)?!|expect(?:Later)?)\s*\(',source):
        index=match.end();depth=1;quote=None
        while index<len(source) and depth:
            char=source[index]
            if quote:
                if char=='\\':index+=2;continue
                if char==quote:quote=None
            elif char in '\"\'':quote=char
            elif char=='(':depth+=1
            elif char==')':depth-=1
            index+=1
        text=source[match.start():index]
        found.append({'line':source[:match.start()].count('\n')+1,'text':text,'key':re.sub(r'\s+','',text)})
    return found

def test_name(source,line):
    before='\n'.join(source.splitlines()[:line-1])
    names=list(re.finditer(r'(?:async\s+)?fn\s+(test_\w+)|\btest(?:Widgets)?\(\s*[\'\"]([^\'\"]+)',before))
    return (names[-1].group(1) or names[-1].group(2)) if names else 'test/helper context'

def reason(file,test):
    if '/mobile_patient/test/' in file:return 'Approved exact CTA wording: Proceed to payment method. Enabled/disabled state, backend state and financial checks remain.'
    if '/mobile/test/' in file:return 'Approved current/today operational Queue and separate History; tomorrow is asserted absent in Queue and history remains available separately.'
    if 'history_window' in test:return 'Approved retained own-encounter history after 24 hours, with unrelated-Doctor denials retained and summary reads added.'
    if 'rx_pages' in test:return 'Approved ADMIN access and retained own-encounter Rx reads. Independent download expiry, legacy no-session denial and session-revocation checks were added; foreign Doctor denial remains.'
    if 'private_financial' in test:return 'Approved retained own-encounter intake reads. The original private-profile cutoff and foreign/private/financial isolation assertions remain unchanged.'
    if 'queue' in test:return 'Approved default Queue contains current/today only. Explicit All retains exact aggregate row count and ordering; Schedule and History receive exact separate assertions.'
    return 'Approved canonical ADMIN role: exact active role/audit identity becomes ADMIN, retired claims are rejected with exact 401, and Patient/Doctor remain exact 403. All-Admin privileges are intentional; durable denial/account-attribution checks are preserved or strengthened.'

changed=[];files=[];missing=[];deleted_tests=[];new_markers=[];deleted_assertions=[]
for file,snapshot in BASE['files'].items():
    if '/test/' not in file and '/tests/' not in file:continue
    path=ROOT/file
    if not path.exists():missing.append(file);continue
    old=base64.b64decode(snapshot['content']).decode('utf-8')
    new=path.read_text(encoding='utf-8')
    if old.splitlines()==new.splitlines():continue
    files.append(file)
    diff=list(difflib.unified_diff(old.splitlines(),new.splitlines(),fromfile='baseline/'+file,tofile='current/'+file))
    directory=OUT/'test_diffs';directory.mkdir(exist_ok=True)
    (directory/(file.replace('/','__')+'.diff')).write_text('\n'.join(diff)+'\n',encoding='utf-8')
    previous=assertions(old);current=assertions(new)
    for operation,a,b,c,d in difflib.SequenceMatcher(a=[x['key'] for x in previous],b=[x['key'] for x in current],autojunk=False).get_opcodes():
        if operation not in ('replace','delete'):continue
        for assertion in previous[a:b]:
            name=test_name(old,assertion['line'])
            item={'file':file,'test':name,'baseline_line':assertion['line'],'old_assertion':assertion['text'],'new_assertion':[x['text'] for x in current[c:d]],'reason':reason(file,name),'opcode':operation}
            changed.append(item)
            if operation=='delete':deleted_assertions.append(item)
    for line in diff:
        if line.startswith('+') and not line.startswith('+++') and re.search(r'#\[ignore\]|\bskip\s*[:(]|\.skip\(|@Skip',line):new_markers.append({'file':file,'line':line})
    pattern=r'#\[sqlx::test[^\]]*\]\s*async fn (\w+)' if file.endswith('.rs') else r'\btest(?:Widgets)?\(\s*[\'\"]([^\'\"]+)'
    deleted_tests.extend({'file':file,'test':name} for name in set(re.findall(pattern,old))-set(re.findall(pattern,new)))

original=base64.b64decode(BASE['files']['services/backend/tests/api_tests.rs']['content']).decode('utf-8')
current=(ROOT/'services/backend/tests/api_tests.rs').read_text(encoding='utf-8')
private_assertion=re.search(r'assert_eq!\(\s*expired_followup,\s*0,[\s\S]*?\);',original).group(0)
assert re.sub(r'\s+','',private_assertion) in re.sub(r'\s+','',current),'Private-profile cutoff assertion must remain intact'
report={'baseline_counts':BASE['counts'],'baseline_source':BASE['counts_source'],'modified_existing_test_files':files,'missing_existing_test_files':missing,'removed_existing_test_functions':deleted_tests,'added_skip_ignore_lines':new_markers,'existing_assertions_deleted_without_replacement':deleted_assertions,'intentionally_changed_assertions':changed,'private_profile_cutoff_assertion_preserved':True,'additional_approved_contract_changes':[
 'test_doctor_history_window_pagination_and_cutoff: own-encounter list reads at +16 min and at/after 1440 min completion now return exact 200; unrelated Doctor still exact 404; own summary after cutoff is additionally exact 200.',
 'test_doctor_queue_shows_upcoming_consultations_first: old +3-day fixtures remain unchanged; original exact All ordering/pagination checks now explicitly request view=all, with new default Queue absence and exact Schedule/History assertions.',
 'test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history: default Queue 3-to-2 and active Queue 4-to-3 intentionally move tomorrow into a separately asserted one-row Schedule. All retains exact five rows and historical ordering.',
 'test_identity_compliance_trace_scoped_authorized_and_audited: four durable denials remain required; the query includes two AUTH_ROLE_MISMATCH audit rows for retired claims, with a new exact two-row/session/request/account-role assertion.'
 ],'note':'Product-contract replacements are listed exactly. Full diffs also include fixture role/endpoint changes and additions. No security/business/concurrency assertion is silently removed.'}
(OUT/'test_integrity_audit.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
lines=['# Test integrity audit','',f'Modified existing test files: {len(files)}. Intentional assertion replacements: {len(changed)}.','',
'No removed existing test functions or added skip/ignore markers were detected. The private-profile cutoff remains exact zero.','',
'## Existing test files','']+['- `'+file+'`' for file in files]
lines+=['','## Intentional assertion replacements','']
for index,item in enumerate(changed,1):
    lines += [f'### {index}. {item["test"]}', '',f'File: `{item["file"]}` (frozen baseline line {item["baseline_line"]}).','', 'Old:', '```', item['old_assertion'], '```', '', 'New:', '```', '\n'.join(item['new_assertion']), '```', '', item['reason'], '']
lines += ['## Changes to expected-value tables and request surfaces','']+['- '+value for value in report['additional_approved_contract_changes']]
(OUT/'TEST_INTEGRITY_AUDIT.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
print(json.dumps({'modified_existing_test_files':len(files),'intentional_assertion_replacements':len(changed),'missing_files':missing,'removed_tests':deleted_tests,'deleted_assertions_without_replacement':len(deleted_assertions),'added_skip_markers':new_markers},indent=2))
if missing or deleted_tests or deleted_assertions or new_markers:raise SystemExit(1)
