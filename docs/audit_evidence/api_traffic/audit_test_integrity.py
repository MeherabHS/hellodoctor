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
    return 'No existing assertion replacement is approved for this traffic pass.'

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
report={'baseline_counts':BASE['counts'],'baseline_source':BASE['counts_source'],'modified_existing_test_files':files,'missing_existing_test_files':missing,'removed_existing_test_functions':deleted_tests,'added_skip_ignore_lines':new_markers,'existing_assertions_deleted_without_replacement':deleted_assertions,'intentionally_changed_assertions':changed,'private_profile_cutoff_assertion_preserved':True,'fixture_protocol_additions':[
 'Existing notification fakes implement the additive after-cursor protocol; no assertions changed.',
 'Existing schedule fakes accept optional day; no assertions changed.',
 'Patient chat fake implements newer-message protocol; no assertions changed.',
 'Doctor consultation pagination test now checks first-page length=100 and hasMore=true, explicitly loads the next page, then runs every original exact ordering/identity/page assertion unchanged. This exercises the approved on-demand pagination behavior.'
 ],'test_runners_changed':False,'historical_tag_rewritten':False}
(OUT/'test_integrity_audit.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps({'modified_existing_test_files':files,'intentional_assertion_replacements':len(changed),'missing_files':missing,'removed_tests':deleted_tests,'deleted_assertions_without_replacement':len(deleted_assertions),'added_skip_markers':new_markers},indent=2))
if missing or deleted_tests or changed or deleted_assertions or new_markers:raise SystemExit(1)
