import hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent
def read(p):
    b=p.read_bytes()
    return b.decode('utf-16' if b[:2]==b'\xff\xfe' else 'utf-8-sig')
result={}
for client in ['doctor_flutter','patient_flutter']:
    for phase in ['before','after']:
        events=[json.loads(l) for l in read(OUT/f'{client}_{phase}.jsonl').splitlines() if l.startswith('{')]
        completed=[e for e in events if e['type']=='testDone' and not e['hidden']]
        result[f'{client}_{phase}']={'total':len(completed),'passed':sum(e['result']=='success' for e in completed),
          'failed':sum(e['result']!='success' for e in completed),'skipped':sum(e['skipped'] for e in completed),
          'complete':any(e['type']=='done' for e in events)}
for suite,path in [('rust_library','services/backend/consultation_lib_final.log'),
                   ('backend_first_full','services/backend/consultation_api_final.log'),
                   ('backend_final','services/backend/consultation_api_final_corrected.log')]:
    p=ROOT/path
    s=read(p) if p.exists() else ''
    match=re.search(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',s)
    result[suite]=dict(zip(['passed','failed','ignored','measured','filtered_out'],map(int,match.groups()[1:]))) if match else {'complete':False}
for app in ['doctor-web','admin-web']:
    s=read(ROOT/f'apps/{app}/consultation_tests.log')
    result[app]={key:int(re.search(r'\b'+key+r' (\d+)',s)[1]) for key in ['tests','pass','fail','cancelled','skipped','todo']}
artifacts=[]
for p in (ROOT/'releases').glob('*consultations-20261014.apk'):
    artifacts.append({'file':p.relative_to(ROOT).as_posix(),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
result['artifacts']=artifacts
(OUT/'test_results.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
