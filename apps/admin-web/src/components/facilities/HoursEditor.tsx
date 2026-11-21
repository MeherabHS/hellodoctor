'use client';
import { tr } from '@/lib/i18n';
import type {DayHours,OpeningHours,Period,Weekday} from '../../lib/facilityContent';
import {copyMondayToWeekdays,weekdayLabels,weekdays} from '../../lib/facilityContent';

/** Seven-day schedule: per day Closed / Open 24 hours / opening periods (up to
 * three, each optionally closing after midnight). `null` = hours not provided. */
export function HoursEditor({value,onChange}:{value:OpeningHours|null;onChange:(v:OpeningHours|null)=>void}){
  if(!value){
    return <div className="fac-hours"><p className="cms-hint">{tr('ui.openingHoursAreNotProvidedPatients')}</p>
      <button type="button" onClick={()=>onChange({MON:{periods:[{open:'08:00',close:'17:00'}]}})}>{tr('ui.addOpeningHours')}</button></div>;
  }
  const setDay=(day:Weekday,next:DayHours|undefined)=>{const copy={...value};if(next)copy[day]=next;else delete copy[day];onChange(copy);};
  const setPeriod=(day:Weekday,i:number,patch:Partial<Period>)=>{const periods=[...(value[day]?.periods??[])];periods[i]={...periods[i],...patch};setDay(day,{periods});};
  return <div className="fac-hours">
    <div className="fac-hours-actions">
      <button type="button" onClick={()=>onChange(copyMondayToWeekdays(value))}>{tr('ui.copyMondayHoursToWeekdays')}</button>
      <button type="button" onClick={()=>onChange(null)}>{tr('ui.removeOpeningHours')}</button>
      <span className="cms-hint">{tr('ui.timesAreLocalKenyaTimeAfrica')}</span>
    </div>
    <table className="fac-hours-table"><tbody>{weekdays.map(day=>{
      const d=value[day];const mode=!d||(!d.all_day&&!(d.periods?.length))?'closed':d.all_day?'all_day':'periods';
      return <tr key={day}><th scope="row">{weekdayLabels[day]}</th><td>
        <select aria-label={`${weekdayLabels[day]} status`} value={mode} onChange={e=>{const m=e.target.value;setDay(day,m==='closed'?undefined:m==='all_day'?{all_day:true}:{periods:[{open:'08:00',close:'17:00'}]});}}>
          <option value="closed">{tr('ui.closed')}</option><option value="periods">{tr('ui.open')}</option><option value="all_day">{tr('ui.open24Hours')}</option>
        </select></td>
        <td>{mode==='periods'&&<div className="fac-periods">{(d?.periods??[]).map((p,i)=><div className="fac-period" key={i}>
          <input type="time" aria-label={tr('tpl.periodOpens', { day: weekdayLabels[day], n: i+1 })} value={p.open} onChange={e=>setPeriod(day,i,{open:e.target.value})}/>
          <span>{tr('tpl.to')}</span>
          <input type="time" aria-label={tr('tpl.periodCloses', { day: weekdayLabels[day], n: i+1 })} value={p.close==='24:00'?'00:00':p.close} onChange={e=>setPeriod(day,i,{close:e.target.value})}/>
          <label className="cms-check"><input type="checkbox" checked={Boolean(p.overnight)} onChange={e=>setPeriod(day,i,{overnight:e.target.checked})}/>{tr('ui.closesNextDay')}</label>
          <button type="button" aria-label={tr('tpl.removePeriod', { day: weekdayLabels[day], n: i+1 })} onClick={()=>{const periods=(d?.periods??[]).filter((_,j)=>j!==i);setDay(day,periods.length?{periods}:undefined);}}>{tr('ui.remove')}</button>
        </div>)}
        {(d?.periods?.length??0)<3&&<button type="button" onClick={()=>setDay(day,{periods:[...(d?.periods??[]),{open:'14:00',close:'17:00'}]})}>{tr('ui.addPeriod')}</button>}</div>}</td></tr>;
    })}</tbody></table>
  </div>;
}
