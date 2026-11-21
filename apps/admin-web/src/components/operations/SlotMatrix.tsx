'use client';
import { tr } from '@/lib/i18n';
import {useMemo,useState} from 'react';
import {Ban,CalendarCheck,CalendarClock,Lock,RefreshCw} from 'lucide-react';
import type {AdminSlot} from '../../lib/api';
import {scheduleLabel} from '../../lib/schedule';
import {Card,EmptyState,PageHead,StatTile} from '../ui/afya';

/**
 * Operations & Safety → Slot Matrix & Locks (Operations board, panel 2).
 * Same rules as before: only Available ⇄ Blocked can be changed here; booked,
 * payment-locked and cancelled slots are read-only. Times are EAT.
 */
const CHIP:Record<AdminSlot['status'],string>={AVAILABLE:'available',BLOCKED:'blocked',BOOKED:'booked',LOCKED_IN_PAYMENT:'locked',CANCELLED:'cancelled'};
const LABEL:Record<AdminSlot['status'],string>={get AVAILABLE(){return tr('ui2.available')},get BLOCKED(){return tr('ui2.blocked')},get BOOKED(){return tr('ui2.booked')},get LOCKED_IN_PAYMENT(){return tr('ui2.lockedInPayment')},get CANCELLED(){return tr('ui2.cancelled')}};
const time=(iso:string)=>new Date(iso).toLocaleTimeString('en-KE',{timeZone:'Africa/Nairobi',hour:'2-digit',minute:'2-digit',hour12:false});

export function SlotMatrix({slots,date,setDate,loading,error,updatedAt,busyId,onRefresh,onToggle}:{slots:AdminSlot[];date:string;setDate:(d:string)=>void;loading:boolean;error:string|null;updatedAt:string|null;busyId:string|null;onRefresh:()=>void;onToggle:(id:string,action:'BLOCK'|'UNBLOCK')=>void}){
  const [doctor,setDoctor]=useState('');
  const doctors=useMemo(()=>[...new Map(slots.map(s=>[s.doctor_id,s.doctor_name])).entries()].sort((a,b)=>a[1].localeCompare(b[1])),[slots]);
  const visible=doctor?slots.filter(s=>s.doctor_id===doctor):slots;
  const count=(st:AdminSlot['status'])=>visible.filter(s=>s.status===st).length;
  const groups=useMemo(()=>{const g=new Map<string,AdminSlot[]>();for(const s of visible){const list=g.get(s.doctor_id)??[];list.push(s);g.set(s.doctor_id,list);}
    return [...g.values()].map(list=>list.sort((a,b)=>a.start_time.localeCompare(b.start_time))).sort((a,b)=>a[0].doctor_name.localeCompare(b[0].doctor_name));},[visible]);

  return <div className="afya-stack">
    <PageHead title={tr('ui.slotMatrixLocks')} subtitle={tr('ui4.doctorScheduleSlotsBlocksAndPayment')}/>
    <div className="afya-stat-grid">
      <StatTile icon={CalendarCheck} tone="green" value={count('AVAILABLE')} label={tr('ui.available')}/>
      <StatTile icon={Ban} tone="neutral" value={count('BLOCKED')} label={tr('ui.blocked')}/>
      <StatTile icon={CalendarClock} tone="blue" value={count('BOOKED')} label={tr('ui.booked')}/>
      <StatTile icon={Lock} tone="amber" value={count('LOCKED_IN_PAYMENT')} label={tr('ui.lockedInPayment')}/>
    </div>
    <Card title={`${tr('tpl.scheduleSlots')}${date?` — ${new Date(`${date}T12:00:00+03:00`).toLocaleDateString('en-US',{timeZone:'Africa/Nairobi',weekday:'short',month:'short',day:'numeric',year:'numeric'})} (EAT)`:tr('tpl.allDates')}`}
      subtitle={tr('ui4.clickAnAvailableSlotToBlock')}
      actions={<div className="afya-slot-legend">{(['AVAILABLE','BLOCKED','BOOKED','LOCKED_IN_PAYMENT'] as const).map(s=><span key={s} className={`slot-chip ${CHIP[s]}`}>{LABEL[s]}</span>)}</div>}>
      <div className="afya-filter-bar">
        <label className="afya-check" style={{gap:10}}>{tr('ui.scheduleDate')}<input className="afya-input" style={{width:170}} type="date" aria-label={tr('ui.scheduleDate')} value={date} onChange={e=>setDate(e.target.value)}/></label>
        <button type="button" className="afya-btn-secondary" onClick={()=>setDate('')}>{tr('ui.allDates')}</button>
        <select className="afya-select" aria-label={tr('ui.doctor')} value={doctor} onChange={e=>setDoctor(e.target.value)}><option value="">{tr('ui.allDoctors')}</option>{doctors.map(([id,name])=><option key={id} value={id}>{name}</option>)}</select>
        <button type="button" className="afya-btn-secondary" disabled={loading||Boolean(busyId)} onClick={onRefresh}><RefreshCw size={15} aria-hidden/>{tr('ui.refresh')}</button>
        <span className="afya-stat-note" role="status" aria-live="polite" style={{marginTop:0}}>{loading?tr('ui2.refreshingSchedule'):updatedAt?tr('tpl.lastSynced', { time: scheduleLabel(updatedAt) }):tr('ui2.scheduleNotLoaded')}</span>
      </div>
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error}</div>}
      {!groups.length?<EmptyState title={loading?tr('ui2.loadingSchedule'):error?tr('ui2.scheduleUnavailable'):date?tr('ui2.noScheduleSlotsForThisDate'):tr('ui2.noScheduleSlotsExistYet')}/>:
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>{tr('ui.doctor')}</th><th>{tr('ui.schedule10MinuteSlots')}</th><th style={{textAlign:'right'}}>{tr('ui.total')}</th><th style={{textAlign:'right'}}>{tr('ui.booked')}</th><th style={{textAlign:'right'}}>{tr('ui.locked')}</th></tr></thead>
        <tbody>{groups.map(list=><tr key={list[0].doctor_id}>
          <td style={{fontWeight:700,whiteSpace:'nowrap',verticalAlign:'top'}}>{list[0].doctor_name}</td>
          <td><div className="afya-slot-row">{list.map(slot=>{
            const canToggle=(slot.status==='AVAILABLE'||slot.status==='BLOCKED')&&!busyId&&!error;
            return <button key={slot.id} type="button" data-slot-id={slot.id} data-slot-status={slot.status} className={`slot-chip ${CHIP[slot.status]}`} disabled={!canToggle}
              title={`${LABEL[slot.status]}${!date?` · ${scheduleLabel(slot.start_time)}`:''} – ${time(slot.end_time)} EAT${slot.status==='AVAILABLE'?tr('tpl.clickToBlock'):slot.status==='BLOCKED'?tr('tpl.clickToMakeAvailable'):''}`}
              style={{opacity:slot.status==='CANCELLED'?0.5:busyId===slot.id?0.6:1}}
              onClick={()=>canToggle&&onToggle(slot.id,slot.status==='AVAILABLE'?'BLOCK':'UNBLOCK')}>
              {date?time(slot.start_time):scheduleLabel(slot.start_time)}{slot.status==='LOCKED_IN_PAYMENT'&&<Lock size={11} aria-label={tr('ui.locked')}/>}
            </button>;})}</div></td>
          <td style={{textAlign:'right'}}>{list.length}</td>
          <td style={{textAlign:'right'}}>{list.filter(s=>s.status==='BOOKED').length}</td>
          <td style={{textAlign:'right'}}>{list.filter(s=>s.status==='LOCKED_IN_PAYMENT').length}</td>
        </tr>)}</tbody>
      </table></div>}
    </Card>
  </div>;
}
