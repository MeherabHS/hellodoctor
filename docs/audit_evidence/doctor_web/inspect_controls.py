import importlib.util,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('i',ROOT/'docs/audit_evidence/inspect_doctor_prototype.py');i=importlib.util.module_from_spec(spec);spec.loader.exec_module(i)
p=i.Parser();p.feed((ROOT/'prototype/index.html').read_text(encoding='utf-8'))
for n in i.walk(p.root):
    a=n['attrs'];ident=a.get('id','');cls=a.get('class','')
    if (('modal' in cls or 'sheet' in cls or 'overlay' in cls) and ident and not ident.startswith(('admin','ag','af','patientWeb'))) or ('bubble' in cls and n.get('line',0) in range(7400,7540)):
        print(n['line'],ident,cls,i.label(n)[:100])
