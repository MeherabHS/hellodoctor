"""Inventory canonical controls recursively without accepting visual parity."""
import importlib.util,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('inventory',ROOT/'docs/audit_evidence/inspect_doctor_prototype.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
raw=(ROOT/'prototype/index.html').read_text(encoding='utf-8');p=m.Parser();p.feed(raw)
nodes=list(m.walk(p.root));ids={n['attrs']['id']:n for n in nodes if n['attrs'].get('id')}
screens=[f'doc-view-{i}' for i in range(6)]+[f'dwPage_{name}' for name in ['dashboard','appointments','patients','telehealth','qna']]
dialogs=['doctorAuthSection','doctorStatementModal','docConsultationFeeModal','patientHistoryModal','labReportModal','slotGeneratorModal']
mapping={
 'doc-view-0':('Queue.tsx','doctor/profile; doctor/duty; doctor/appointments; doctor/qna/questions'),
 'doc-view-1':('Rx.tsx','doctor/appointments; consultations/:appointment/prescription'),
 'doc-view-2':('Schedule.tsx','doctor/slots'),
 'doc-view-3':('Wallet.tsx','doctor/wallet/summary; doctor/wallet/ledger'),
 'doc-view-4':('Chat.tsx','doctor/chats; chat/:conversation/messages'),
 'doc-view-5':('Qna.tsx','doctor/qna/questions; doctor/qna/questions/:id/answers'),
 'dwPage_qna':('Qna.tsx','doctor/qna/questions; doctor/qna/questions/:id/answers'),
 'doctorAuthSection':('Auth.tsx','auth/doctor/login; auth/doctor/verify-otp; auth/doctor/register; auth/refresh; auth/logout'),
 'doctorStatementModal':('Statement.tsx','doctor/wallet/statements?month=YYYY-MM'),
 'docConsultationFeeModal':('Clinical.tsx Receipt','doctor/wallet/ledger'),
 'patientHistoryModal':('Clinical.tsx History','doctor/appointments/:current/patient-history; /:episode'),
 'labReportModal':('Clinical.tsx Documents','doctor/consultations/:appointment/summary; contextual patient-history/:episode; appointment document grants'),
 'slotGeneratorModal':('Clinical.tsx SlotForm','POST doctor/slots'),
}
rows=[];handlers=set();seen=set()
def collect(n,screen):
    if not isinstance(n,dict):return
    attrs=n['attrs'];ident=attrs.get('id','')
    if ident in screens:screen=ident
    h='; '.join(v or '' for k,v in attrs.items() if k.startswith('on'))
    handlers.update(re.findall(r'\b([A-Za-z_$][\w$]*)\(',h))
    if (ident or h or attrs.get('class') or n['tag'] in ('button','input','select','textarea')) and n['line'] not in seen:
        seen.add(n['line']);component,backend=mapping.get(screen,('Desktop.tsx / DoctorApplication.tsx','Current Doctor profile and owned appointments; deferred RTC controls'))
        rows.append({'element':ident or '.'+attrs.get('class',n['tag']).replace(' ','.'),'line':n['line'],'screen':screen,'purpose':m.label(n)[:100],'interaction':h or 'Display / field / layout','component':component,'backend':backend,'status':'NOT_COMPLETE'})
    for child in n['children']:collect(child,screen)
for root in ['doctorAppShell','doctorWebShell',*dialogs]:collect(ids[root],root)
subflows=['Doctor login','Doctor registration','Doctor MFA','Pending verification','Chat list / search / filters','Chat thread / canned replies / header actions','Rx camera / file selection','Rx multiple pages / preview / remove / retry','Slot generator / individual slot','Slot cancellation confirmation','Monthly statement / month / pagination / print','Consultation fee receipt','Patient history / episode / pagination','Lab document / table view / page selection / download','Desktop telehealth / drawers / fullscreen / call controls','Consultation completion / with Rx / no Rx / referral / escalation','Profile settings / logout','Payout readiness','Offline / reconnect','Q&A list / guidance / concurrent answer']
out=Path(__file__).parent
(out/'coverage-current.json').write_text(json.dumps({'screens':screens,'subflows':subflows,'controls':rows,'handlers':sorted(handlers),'matched':0},ensure_ascii=False,indent=2),encoding='utf-8')
md=['# Doctor Web canonical prototype inventory','','Authority: `prototype/index.html`, completely parsed again after implementation. Root `index.html` is not used. This replaces the initial flattened inventory, which could assign the last screen name to sibling navigation controls. Screen ownership now follows recursive DOM ancestry.','','This is an interim coverage record. `NOT_COMPLETE` includes controls whose rendered parity or complete interaction sequence remains unverified. Source portability and successful API calls do not establish `MATCHED`.','','## Screen and subflow counts','',f'Canonical screen roots: {len(screens)}. Explicit subflows: {len(subflows)}. Element rows: {len(rows)}. Rendered visual matches accepted: 0.','','| Prototype ID/class and source line | Screen | Purpose | Interaction | Data required | Backend source | Mobile reference | Web implementation / status |','|---|---|---|---|---|---|---|---|']
for row in rows:
    fields=[f"`{row['element']}` ({row['line']})",row['screen'],row['purpose'],row['interaction'],'Current Doctor/session or authorized DTO',row['backend'],'doctor_pages.dart / doctor_repository.dart',row['component']+' · '+row['status']]
    md.append('| '+' | '.join(str(x).replace('|','/').replace('\n',' ') for x in fields)+' |')
md+=['','## Explicit subflows','',*[f'- {flow}: NOT_COMPLETE' for flow in subflows],'','## Shared patient-only flow distinction','','`rxPadPreviewModal` includes patient medicine ordering. It is retained in extracted source evidence but is not a Doctor ordering capability. Doctor prescription previews use uploaded handwritten images; no generated prescription or pharmacy order is fabricated.','','## Component and visual system map','','DoctorApplication uses the actual desktop shell; Queue, Chat, Rx, Schedule and Wallet use their canonical source wrappers. Qna preserves its distinct layout. Prototype renders inert DOM templates through React; source scripts are excluded. Prototype CSS values remain in `src/app/prototype.css`; local font assets preserve Inter and Plus Jakarta Sans weights. `web.css` contains explicit browser additions that still require visual review.','','The current desktop/mobile shell combination is not visually accepted. No duplicate Doctor project was created.','','## Canonical handlers read','',', '.join(f'`{name}`' for name in sorted(handlers))]
(ROOT/'docs/DOCTOR_WEB_PROTOTYPE_INVENTORY.md').write_text('\n'.join(md)+'\n',encoding='utf-8')
print(json.dumps({'screens':len(screens),'subflows':len(subflows),'element_rows':len(rows),'visual_matches_accepted':0}))
