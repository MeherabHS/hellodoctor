'use client';
import {useEffect,useMemo,useRef,useState} from 'react';
import {Activity,Clock,Radio,ShieldCheck} from 'lucide-react';
import {getActivity,humanize,SEARCH_PATTERN,type ActivityArea,type ActivityItem} from '../../lib/activityApi';
import {Card,Chip,EmptyState,PageHead,SearchField,StatTile,Tabs,type ChipTone} from '../ui/afya';

/**
 * Activity Log (product owner, 2026-11-14): every Admin sees who did what and
 * when, across the whole platform, refreshed live. Admins are named by email;
 * patients and doctors appear only by role. No record contents (before/after
 * values, reasons, medical detail) are ever shown. Every value is rendered as
 * text by React.
 */
const POLL_MS = 10_000;
const AREAS: {id: ActivityArea; label: string}[] = [
  {id: 'all', label: 'All'}, {id: 'content', label: 'Content'}, {id: 'people', label: 'People & Care'},
  {id: 'operations', label: 'Operations'}, {id: 'campaigns', label: 'Campaigns'}, {id: 'finance', label: 'Finance'},
  {id: 'security', label: 'Sign-ins'}, {id: 'team', label: 'Admin Team'},
];
const ROLE_LABEL: Record<string, string> = {ADMIN: 'Admin', DOCTOR: 'A doctor', PATIENT: 'A patient', SYSTEM: 'System'};
/** Plain-language names for actions whose codes read poorly. */
const ACTION_LABEL: Record<string, string> = {
  ADMIN_REAUTH_FAILED: 'Wrong password when inviting',
  ADMIN_SIGNED_IN: 'Signed in to Admin',
  AUTH_ROLE_MISMATCH: 'Blocked: forged role in session',
  ADMIN_INVITATION_SENT: 'Invited an Admin',
  ADMIN_INVITATION_RESENT: 'Resent an Admin invitation',
  ADMIN_INVITATION_REVOKED: 'Revoked an Admin invitation',
  ADMIN_INVITATION_ACCEPTED: 'Accepted an Admin invitation',
};
const RESULT_TONE: Record<string, ChipTone> = {SUCCESS: 'success', FAILURE: 'danger', DENIED: 'danger'};

const TIME = new Intl.DateTimeFormat('en-GB', {timeZone: 'Africa/Nairobi', day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false});
function eatTime(iso: string): string { const d = new Date(iso); return Number.isNaN(d.getTime()) ? '—' : `${TIME.format(d)} EAT`; }

export function ActivityLogPage(){
  const [area,setArea]=useState<ActivityArea>('all');
  const [search,setSearch]=useState('');const [query,setQuery]=useState('');
  const [onlyMine,setOnlyMine]=useState(false);
  // Pages are tagged with the filter they were loaded for, so a filter change
  // shows "loading" until its own first page arrives (no stale rows).
  type Loaded={key:string;items:ActivityItem[];cursor:string|null};
  const [head,setHead]=useState<Loaded|null>(null);const [older,setOlder]=useState<Loaded|null>(null);
  const [loadingMore,setLoadingMore]=useState(false);
  const [error,setError]=useState('');const [updatedAt,setUpdatedAt]=useState<Date|null>(null);
  const filterKey=`${area}|${query}|${onlyMine}`;
  const keyRef=useRef(filterKey);
  useEffect(()=>{keyRef.current=filterKey;},[filterKey]);

  // Debounce typing; ignore characters the server would reject.
  const searchValid=SEARCH_PATTERN.test(search.trim());
  useEffect(()=>{const t=setTimeout(()=>{if(searchValid)setQuery(search.trim());},350);return()=>clearTimeout(t);},[search,searchValid]);

  // Newest page, refreshed live while the tab is visible.
  useEffect(()=>{
    let active=true;
    const key=filterKey;
    async function load(first:boolean){
      if(!first&&document.visibilityState!=='visible')return;
      try{
        const page=await getActivity({area,search:query,mine:onlyMine,before:null});
        if(!active)return;
        setHead({key,items:page.items,cursor:page.next_before});setError('');setUpdatedAt(new Date());
      }catch{if(active)setError('The activity log could not be loaded. It will retry automatically.');}
    }
    void load(true);
    const timer=setInterval(()=>void load(false),POLL_MS);
    return()=>{active=false;clearInterval(timer);};
  // eslint-disable-next-line react-hooks/exhaustive-deps -- filterKey covers area/query/onlyMine
  },[filterKey]);

  const current=head?.key===filterKey?head:null;
  const extra=older?.key===filterKey?older:null;
  const loading=current===null;
  const cursor=extra?extra.cursor:current?.cursor??null;

  async function loadMore(){
    if(!cursor||loadingMore)return;setLoadingMore(true);
    const key=filterKey;
    try{
      const page=await getActivity({area,search:query,mine:onlyMine,before:cursor});
      if(keyRef.current!==key)return;
      setOlder(prev=>({key,items:[...(prev?.key===key?prev.items:[]),...page.items],cursor:page.next_before}));
    }catch{setError('Older activity could not be loaded. Try again.');}
    finally{setLoadingMore(false);}
  }

  const rows=useMemo(()=>{
    const seen=new Set<string>();const all:ActivityItem[]=[];
    for(const i of [...(current?.items??[]),...(extra?.items??[])]){if(!seen.has(i.id)){seen.add(i.id);all.push(i);}}
    return all;
  },[current,extra]);
  const anchor=updatedAt?.getTime()??0;
  const lastHour=rows.filter(i=>anchor-new Date(i.at).getTime()<3_600_000).length;
  const adminsActive=new Set(rows.filter(i=>i.actor_email).map(i=>i.actor_email)).size;

  return <div className="afya-stack">
    <PageHead title="Activity Log" subtitle="Who did what, and when, across the platform. Visible to every Admin and updated live."/>
    <div className="afya-stat-grid">
      <StatTile icon={Activity} tone="green" value={loading?'—':rows.length} label="Events shown"/>
      <StatTile icon={Clock} tone="amber" value={loading?'—':lastHour} label="In the last hour"/>
      <StatTile icon={ShieldCheck} tone="blue" value={loading?'—':adminsActive} label="Admins in view"/>
      <StatTile icon={Radio} tone="green" value="Live" label="Refreshes every 10 s" note={updatedAt?`Updated ${updatedAt.toLocaleTimeString('en-GB',{timeZone:'Africa/Nairobi',hour12:false})} EAT`:undefined}/>
    </div>

    <Card>
      <Tabs label="Activity area" items={AREAS} value={area} onChange={setArea}/>
      <div className="afya-filter-bar" style={{marginTop:14,marginBottom:0}}>
        <div style={{flex:'1 1 320px'}}><SearchField value={search} onChange={setSearch} label="Search actions" placeholder="Search actions, e.g. publish, verify, deleted" maxLength={80}/></div>
        <label style={{display:'inline-flex',alignItems:'center',gap:8,fontSize:13.5,fontWeight:600}}>
          <input type="checkbox" checked={onlyMine} onChange={e=>setOnlyMine(e.target.checked)}/> Only my actions
        </label>
      </div>
      {!searchValid&&<p className="afya-stat-note" style={{margin:'8px 0 0'}}>Search uses letters, digits, spaces and _ only.</p>}
    </Card>

    {error&&<div className="afya-login-error" role="alert">{error}</div>}

    <Card title="Events" subtitle="Newest first. Times are East Africa Time. Patients and doctors are shown by role only; record contents are never shown." padded={false}>
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>Time</th><th>Who</th><th>Action</th><th>Record</th><th>Result</th></tr></thead>
        <tbody>{rows.map(i=><tr key={i.id}>
          <td style={{whiteSpace:'nowrap',fontVariantNumeric:'tabular-nums'}}>{eatTime(i.at)}</td>
          <td><div style={{display:'flex',alignItems:'center',gap:8,flexWrap:'wrap'}}>
            <span style={{fontWeight:650}}>{i.actor_email??ROLE_LABEL[i.actor_role]??humanize(i.actor_role)}</span>
            {i.is_you&&<Chip tone="info">You</Chip>}
            {i.actor_email&&<Chip tone="outlined">Admin</Chip>}
          </div></td>
          <td>{ACTION_LABEL[i.action]??humanize(i.action)}</td>
          <td><div>{humanize(i.resource_type)}</div>{i.resource_id&&<div className="afya-stat-note afya-mono" title={i.resource_id}>{i.resource_id.slice(0,8)}</div>}</td>
          <td><Chip tone={RESULT_TONE[i.result]??'neutral'}>{humanize(i.result)}</Chip></td>
        </tr>)}</tbody>
      </table>
      {!loading&&!rows.length&&<EmptyState title="No activity matches">Try another area or search.</EmptyState>}
      {loading&&<EmptyState title="Loading activity…"/>}
      </div>
      {cursor&&<div style={{padding:16,textAlign:'center'}}><button type="button" className="afya-btn-secondary" disabled={loadingMore} onClick={()=>void loadMore()}>{loadingMore?'Loading…':'Load older activity'}</button></div>}
    </Card>
  </div>;
}
