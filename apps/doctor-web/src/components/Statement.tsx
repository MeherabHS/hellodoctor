'use client';
import { useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { Profile, RecordDto, Statement as StatementDto, money, payoutLabel, statementLabel } from '../lib/contracts';
import { Prototype, hasClass, id, text } from './Prototype';
import { State } from './Common';
import { date } from './Queue';

export function Statement({profile,onClose}:{profile:Profile;onClose:()=>void}) {
  const [month,setMonth]=useState(new Date().toISOString().slice(0,7));
  const result=useResource<StatementDto>(`doctor/wallet/statements?month=${month}&limit=50&offset=0`);
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');const s=result.data;
  async function more(){if(!s||s.next_offset===null||busy)return;setBusy(true);setError('');try{const next=await api<StatementDto>(`doctor/wallet/statements?month=${month}&limit=50&offset=${s.next_offset}`);result.setData({...next,entries:[...s.entries,...next.entries]});}catch(e){setError(errorText(e));}finally{setBusy(false);}}
  return <><Prototype name="doctorStatementModal" action={handler=>handler.includes('close')?onClose():window.print()} override={n=>{
    if(id(n)==='doctorStatementModal')return {props:{className:'',style:{display:'contents'}}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(hasClass(n,'sheet-pill-tag'))return {children:s?s.status.replaceAll('_',' '):'Loading statement'};
    if(n.tag==='span'&&text(n).includes('Cycle:'))return {children:<>Cycle: <input aria-label="Statement month" type="month" value={month} min="2000-01" max={new Date().toISOString().slice(0,7)} onChange={e=>{if(e.target.value)setMonth(e.target.value);}} style={{font:'inherit',color:'inherit',border:'1px solid var(--border)',borderRadius:6,padding:4}}/></>};
    if(id(n)==='dsmDocTitle')return {children:'Physician Earnings & Platform Charge Statement'};
    if(hasClass(n,'sheet-desc'))return {children:<>Official reconciliation statement for <strong>{profile.full_name}</strong>. {s?statementLabel(s):'Statement'} is supplied by Finance.</>};
    const values:Record<string,React.ReactNode>={dsmGrossVal:money(s?.gross_earnings),dsmWithheldVal:money(s?.platform_charge),dsmFinalVal:money(s?.payable)};
    if(id(n) in values)return {children:values[id(n)]};
    if(n.tag==='span'&&n.line===8818)return {children:'2. Platform charge'};
    if(n.tag==='span'&&n.line===8827)return {children:`3. ${s?statementLabel(s):'Estimated payout'}`};
    if(n.tag==='div'&&n.line===8819)return {children:'Authoritative platform charge for the statement period'};
    if(n.tag==='div'&&n.line===8828)return {children:'Statement payable; provider payment remains separate'};
    if(n.tag==='div'&&n.line===8804)return {children:'Statement summary'};
    if(n.tag==='div'&&n.line===8843)return {children:'Itemized consultations'};
    if(hasClass(n,'doc-earnings-calc-note'))return {children:<div><strong>{s?.basis_note??'Total calculation is based including the platform charge 20%.'}</strong><br/><strong>Accounting basis:</strong> Values come from the month-scoped financial statement. Adjustments: {money(s?.adjustments)}.<br/>Disbursement: {s?payoutLabel(s.payout_status,(s.payout_destination as RecordDto|null)?.provider_code):'—'}<br/>Destination: {String((s?.payout_destination as RecordDto|null)?.masked_destination??'Not configured')}<State loading={result.loading} error={result.error||error} retry={()=>void result.refresh()}/></div>};
    if(n.tag==='div'&&text(n).includes('Itemized Consultations'))return {children:'Itemized Consultations'};
    if(n.tag==='thead')return {children:<tr>{['Consultation','Gross eligible','Platform charge','Doctor earning'].map((title,i)=><th key={title} style={{padding:'8px 10px',textAlign:i?'right':'left'}}>{title}</th>)}</tr>};
    if(n.tag==='tbody')return {children:s?.entries.map((row,index)=><tr key={String(row.consultation_reference)+index} style={{borderBottom:'1px solid var(--border)'}}><td style={{padding:'8px 10px'}}><strong style={{fontSize:11.5}}>{String(row.consultation_reference)}</strong><div style={{fontSize:9.5,color:'var(--text-sub)'}}>{date(row.date as string)} · {String(row.financial_status).replaceAll('_',' ')}</div></td>{[row.gross_fee,row.platform_charge,row.doctor_earning].map((amount,i)=><td key={i} style={{padding:'8px 10px',textAlign:'right',fontWeight:700,color:['#1E40AF','#DC2626','#059669'][i]}}>{money(amount)}</td>)}</tr>)};
    if(n.tag==='button'&&hasClass(n,'primary'))return {props:{disabled:!s},children:'Print / Save Statement (PDF) 📄'};
    return undefined;
  }}/>{s?.entries.length===0&&<State empty="No eligible consultations in this month"/>}{s&&s.next_offset!==null&&<button className="sheet-btn secondary" disabled={busy} onClick={()=>void more()}>{busy?'Loading…':'Load more statement entries'}</button>}</>;
}
