"""Port canonical DOM/CSS as safe React templates. Never ship prototype scripts/data."""
import importlib.util
import json
import re
from copy import deepcopy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('inventory', ROOT / 'docs/audit_evidence/inspect_doctor_prototype.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
raw = (ROOT / 'prototype/index.html').read_text(encoding='utf-8')
parser = module.Parser()
parser.feed(raw)
nodes = list(module.walk(parser.root))
ids = {n['attrs']['id']:n for n in nodes if n['attrs'].get('id')}
OUT = ROOT / 'apps/doctor-web/src'
(OUT / 'generated').mkdir(parents=True, exist_ok=True)
(OUT / 'app').mkdir(parents=True, exist_ok=True)

# Exact source CSS, including all tokens and state rules. Remove comments only.
css = re.search(r'<style>(.*?)</style>', raw, re.S).group(1)
css = re.sub(r'/\*.*?\*/', '', css, flags=re.S)
css = '\n'.join(line.rstrip().replace('\t', '  ') for line in css.splitlines())+'\n'
css = css.replace('bmdc', 'registration')  # historical prototype selectors only
(OUT / 'app/prototype.css').write_text(css, encoding='utf-8')

names = ['doctorAppShell','doctorWebShell', *[f'doc-view-{i}' for i in range(6)], *[f'dwPage_{s}' for s in ['telehealth','dashboard','appointments','patients','qna']], 'doctorAuthSection','doctorStatementModal','docConsultationFeeModal','patientHistoryModal','labReportModal','rxPadPreviewModal','rxCameraViewfinderModal','slotGeneratorModal']
templates = {name:deepcopy(ids[name]) for name in names}
def first(root, cls):
    return next(n for n in module.walk(root) if cls in n['attrs'].get('class','').split())
for name, root, cls in [('queueCard','doc-view-0','queue-card'),('conversationCard','doc-view-4','conv-card'),('slotCard','doc-view-2','doc-slot-card'),('ledgerCard','doc-view-3','patient-queue-card'),('appointmentCard','dwPage_appointments','dw-pt-card')]:
    templates[name] = deepcopy(first(ids[root], cls))

# Runtime-owned regions have no demo payload, even in the production source/bundle.
regions = {'doctorQueueList','docConvContainer','docChatMsgStream','dwChatMsgStream','docSlotsContainer','doctorSlotsList','docSlotList','docConsultationsLedgerContainer','rxPatientSelectorBar','rxCapturedSheetDoc','rxVfDocPreview','dwApptCardsGrid','dwHistScrollContent','docMobileQnaContainer','dwQnaListContainer','previewRxModalContent','pdfDocumentViewArea','pdfDataTableViewArea'}
for root in templates.values():
    for n in list(module.walk(root)):
        a = n['attrs']
        if a.get('id') in regions or n['tag'] == 'tbody':
            n['children'] = []
        if 'rx-preset-strip' in a.get('class','') and a.get('id') != 'rxPatientSelectorBar':
            n['children'] = []
templates['doctorAppShell']['children']=[n for n in templates['doctorAppShell']['children'] if isinstance(n,dict) and 'doc-bnav' in n['attrs'].get('class','')]
for n in module.walk(templates['doctorWebShell']):
    if n['attrs'].get('id')=='dwMainContentArea':n['children']=[]

# Generated text slots retain exact wrapper geometry; only API DTOs fill them.
pattern = re.compile(r'Rafiq|Nusrat|Kamal|Sabrina|Tanvir|Tanjim|Bilkis|Farzana|Shamima|Tariqul|Zayd|Dhaka|Banani|Dhanmondi|Gulshan|Uttara|Mohakhali|Bangladesh|BMDC|bKash|Nagad|BDT|৳|BK-MER|NG-TXN|PT-\d|HD-\d|10678|Penicillin|Amoxicillin|Azithromycin|Paracetamol|Fexofenadine|Aceclofenac|Omeprazole|Fluticasone|Napa|Fexo|120/80|74 bpm|100\.2|98\.6|99%|168 mg|8\.2%|13,400|Neutrophils|14 / 22|8\.4 mins|28 Issued|184 Total|99\.4%|85%|15%|iPhone|5G|1080p|Connected|cloud signed|Escrow Cleared|instant withdrawal|instant settlements|20%|42 consults|4 Left|2 Completed|Sep 2026|Oct 26|24 Sep|10:30|10:40|10:50|11:30|11:45|12:00|02:30|04:00|04:10|04:20|08:00|18:00|34 M|28 F|52 M|34M|28F|52M|60F|22M|Male, 34|Female, 28|Male, 52|34 yrs|28 yrs|52 yrs|39 yrs|47 yrs|23h|18h|3 Patients|28 Prescriptions|3 ALERTS|100%|Demo|demo')
def clean(n):
    if isinstance(n, str):
        return '—' if pattern.search(n) else n
    a = n['attrs']
    props = {}
    # Events become inert action identifiers. No eval or inline event attributes.
    for k, v in a.items():
        if isinstance(v, str):
            # Historical IDs and placeholders must not survive sanitization.
            v = re.sub('bmdc', 'Registration' if k in ('id', 'for') else 'registration', v, flags=re.I)
            if k == 'placeholder':
                v = v.replace('+880', '+254')
        if k.startswith('on'):
            # Remove fake fixture names from action identifiers; runtime uses active context.
            event = re.sub(r"'(rafiq|nusrat|kamal|tanvir)'", "'context'", v or '')
            if pattern.search(event):
                event = re.match(r'\s*([A-Za-z_$][\w$]*)', event)[1] + '()'
            props['data-'+k] = event
        elif k == 'style':
            style = {}
            for declaration in (v or '').split(';'):
                if ':' not in declaration:
                    continue
                key,value = declaration.split(':',1)
                key = key.strip()
                key = key if key.startswith('--') else re.sub(r'-([a-z])',lambda m:m[1].upper(),key)
                if key and value.strip():
                    style[key] = value.strip()
            props['style'] = style
        elif k == 'class':
            props['className'] = v.replace('bmdc', 'registration')
        elif k == 'for':
            props['htmlFor'] = v
        elif k == 'value':
            props['defaultValue'] = ''
        elif k in ('autoplay','playsinline','readonly','tabindex','maxlength','colspan','rowspan'):
            key = {'autoplay':'autoPlay','playsinline':'playsInline','readonly':'readOnly','tabindex':'tabIndex','maxlength':'maxLength','colspan':'colSpan','rowspan':'rowSpan'}[k]
            props[key] = True if v is None else v
        else:
            key = {'viewbox':'viewBox','stroke-width':'strokeWidth','stroke-linecap':'strokeLinecap','stroke-linejoin':'strokeLinejoin','fill-rule':'fillRule','clip-rule':'clipRule'}.get(k,k)
            props[key] = '—' if isinstance(v,str) and pattern.search(v) and k in ('title','placeholder','aria-label') else (True if v is None else v)
    children = [clean(c) for c in n['children']]
    if n['tag'] == 'textarea':
        children = []
    return {'tag':n['tag'],'props':props,'children':children,'line':n['line']}
(OUT / 'generated/templates.json').write_text(json.dumps({k:clean(v) for k,v in templates.items()},ensure_ascii=False,separators=(',',':')), encoding='utf-8')
print('Ported',len(templates),'templates and exact canonical CSS; prototype scripts excluded.')
