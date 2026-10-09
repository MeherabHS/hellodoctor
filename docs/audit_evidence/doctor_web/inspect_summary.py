import importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('inspect',ROOT/'docs/audit_evidence/inspect_doctor_prototype.py')
m=importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
p=m.Parser();p.feed((ROOT/'prototype/index.html').read_text(encoding='utf-8'))
nodes=list(m.walk(p.root));ids={n['attrs']['id']:n for n in nodes if n['attrs'].get('id')}
for name in ['doc-view-2','doc-view-3','patientHistoryModal','labReportModal','docConsultationFeeModal']:
    print(name)
    for n in m.walk(ids[name]):
        if n['attrs'].get('id') or n['tag'] in ('button','input','select','textarea') or 'date' in n['attrs'].get('class',''):
            print(n['line'],n['tag'],n['attrs'].get('id',''),n['attrs'].get('class',''),m.label(n)[:80])
