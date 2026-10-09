"""Replace remaining simplified dialogs with the canonical prototype DOM."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
clinical=ROOT/'apps/doctor-web/src/components/Clinical.tsx'
source=clinical.read_text(encoding='utf-8')
start=source.index('export function SlotForm(')
end=source.index('\nexport function History(',start)
source=source[:start]+'''export function SlotForm({onCreated,onClose}:{onCreated:()=>void;onClose:()=>void}) {
  const [error,setError]=useState('');const [busy,setBusy]=useState(false);const [done,setDone]=useState(false);const [batch,setBatch]=useState(true);const [selectedDate,setDate]=useState('');const [startTime,setStart]=useState('08:00');const [endTime,setEnd]=useState('09:00');const lock=useRef(false);const [created,setCreated]=useState<string[]>([]);
  const pending=useRef<{ranges:Date[][];keys:string[];saved:string[]}|null>(null);
  const start=new Date(`${selectedDate}T${startTime}:00+03:00`),end=new Date(`${selectedDate}T${endTime}:00+03:00`);
  const count=Number.isFinite(start.getTime())&&end>start?(batch?Math.ceil((end.getTime()-start.getTime())/600000):1):0;
  async function submit(){if(lock.current||done)return;setError('');if(!count){setError('Choose a date and an end time after the start time.');return;}if(count>24){setError('Choose an availability block of at most four hours.');return;}lock.current=true;setBusy(true);
    const ranges=batch?Array.from({length:count},(_,i)=>[new Date(start.getTime()+i*600000),new Date(Math.min(end.getTime(),start.getTime()+(i+1)*600000))]):[[start,end]];
    pending.current??={ranges,keys:ranges.map(()=>crypto.randomUUID()),saved:[]};const job=pending.current;
    try{for(let i=job.saved.length;i<job.ranges.length;i++){const[from,to]=job.ranges[i];const dto=await post<RecordDto>('doctor/slots',{start_time:from.toISOString(),end_time:to.toISOString()},job.keys[i]);job.saved.push(String(dto.id));setCreated([...job.saved]);}setDone(true);onCreated();}
    catch(e){setError((job.saved.length?`${job.saved.length} slots were created. Retry continues with the remaining slots. `:'')+errorText(e));if(job.saved.length)onCreated();}finally{lock.current=false;setBusy(false);}
  }
  const frozen=busy||done||!!pending.current;
  return <><Prototype name="slotGeneratorModal" action={handler=>handler.includes('close')?onClose():void submit()} override={n=>{
    if(id(n)==='slotGeneratorModal')return {props:{className:'',style:{display:'contents'}}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(hasClass(n,'sheet-desc'))return {children:<>{batch?'Auto-generate sequential 10-minute slots in your schedule':'Create one availability slot'}<label style={{display:'block',marginTop:8}}>Date · East Africa Time (UTC+3)<input aria-label="Slot date" type="date" required value={selectedDate} disabled={frozen} onChange={e=>setDate(e.target.value)} style={{display:'block',width:'100%',marginTop:3,padding:'8px 9px',borderRadius:10,border:'1px solid var(--border)',fontSize:12}}/></label></>};
    if(id(n)==='slotGenStartTime'||id(n)==='slotGenEndTime'){const isStart=id(n)==='slotGenStartTime';return {tag:'input',props:{type:'time',value:isStart?startTime:endTime,defaultValue:undefined,disabled:frozen,'aria-label':isStart?'Start time':'End time',onChange:(e:React.ChangeEvent<HTMLInputElement>)=>isStart?setStart(e.target.value):setEnd(e.target.value)},children:null};}
    if(id(n)==='slotGenCountPreview')return {children:count?`${count} slot${count===1?'':'s'}`:'Select date and time'};
    if(n.tag==='b'&&n.line===8510)return {children:batch?'10 Minutes / Patient':'Selected availability block'};
    if(n.tag==='span'&&n.line===8518)return {children:'Booked slots are locked'};
    if(hasClass(n,'flow-cta'))return {props:{disabled:busy||done},children:<span>{busy?'Publishing…':done?'Published by the server ✓':pending.current?'Retry remaining slots':'Generate & Publish Slots to Calendar 📅'}</span>};
    return undefined;
  }}/><label style={{display:'flex',gap:6,alignItems:'center',fontSize:11,marginTop:10}}><input type="checkbox" checked={batch} disabled={frozen} onChange={e=>setBatch(e.target.checked)}/>Generate sequential 10-minute slots</label><State error={error} empty={done?`${created.length} slot(s) created by the server`:undefined}/></>;
}
''' + source[end:]
start=source.index('export function Receipt(')
source=source[:start]+'''export function Receipt({row,onClose}:{row:RecordDto;onClose:()=>void}) {
  return <Prototype name="docConsultationFeeModal" action={onClose} override={n=>{
    if(id(n)==='docConsultationFeeModal')return {props:{className:'',style:{display:'contents'}}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(id(n)==='dcfmTrxId')return {children:`REF: ${String(row.transaction_number)}`};
    if(id(n)==='dcfmPatientTitle')return {children:`Fee Breakdown: ${String(row.patient_display_name)}`};
    if(id(n)==='dcfmSessionSub')return {children:date(String(row.created_at))};
    const values:Record<string,React.ReactNode>={dcfmGross:money(row.gross_amount),dcfmWithheld:money(row.platform_fee_amount),dcfmNet:money(row.net_amount)};
    if(id(n) in values)return {children:values[id(n)]};
    if(hasClass(n,'doc-earnings-step-label'))return {children:n.line<8940?'1. Gross eligible fee':n.line<8952?'2. Platform charge':'3. Doctor earning (net)'};
    if(hasClass(n,'doc-fee-pill'))return {omit:true};
    if(hasClass(n,'doc-earnings-calc-note'))return {children:<div>Financial state: {String(row.payment_status).replaceAll('_',' ')}. Eligible earning does not confirm a provider payout.</div>};
    return undefined;
  }}/>;
}
'''
clinical.write_text(source,encoding='utf-8')
wallet=ROOT/'apps/doctor-web/src/components/Wallet.tsx'
source=wallet.read_text(encoding='utf-8')
start=source.index('export function MonthlyStatement(');end=source.index('export function PayoutSettings()',start)
source=source[:start]+source[end:]
source=source.replace('Profile, RecordDto, Statement, money, payoutLabel, statementLabel','RecordDto, money')
wallet.write_text(source,encoding='utf-8')
app=ROOT/'apps/doctor-web/src/components/DoctorApplication.tsx'
source=app.read_text(encoding='utf-8').replace("wide={dialog==='statement'||dialog==='history'}>","wide={dialog==='statement'||dialog==='history'} bare={['statement','slot','receipt'].includes(dialog)}>")
source=source.replace('<SlotForm onCreated=', '<SlotForm onClose={close} onCreated=').replace('<Receipt row={receipt}/>','<Receipt row={receipt} onClose={close}/>')
app.write_text(source,encoding='utf-8')
