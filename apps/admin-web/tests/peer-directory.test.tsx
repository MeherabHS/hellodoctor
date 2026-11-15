import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {PeerDirectory,PeerRecord,supportedPatients,patientCode} from '../src/components/people/PeerDirectory';
import {clearSoftReads,type AdminPeerDirectoryRow,type AdminPeerHistory,type AdminPeerSession} from '../src/lib/api';

const peer=(id:string,status:AdminPeerDirectoryRow['approval_status'],name:string,extra:Partial<AdminPeerDirectoryRow>={}):AdminPeerDirectoryRow=>({id,full_name:name,email:`${id}@example.test`,phone_number:'+254700000000',county:'Nairobi',organization:'Youth Centre',approval_status:status,approval_note:null,reviewed_at:null,created_at:'2026-11-01T08:00:00Z',...extra});
const session=(id:string,patient:string,name:string,status:AdminPeerSession['status'],start:string,extra:Partial<AdminPeerSession>={}):AdminPeerSession=>({request_id:id,patient_id:patient,patient_name:name,support_type:'CHAT',status,start_time:start,end_time:start,requested_at:start,decided_at:null,completed_at:null,cancelled_at:null,messages:0,call_started_at:null,call_ended_at:null,...extra});
const ok=(data:unknown)=>new Response(JSON.stringify({success:true,data}));
const text=(v:ReactTestRenderer)=>JSON.stringify(v.toJSON());
type Node=string|{children:Node[]};
const words=(n:Node):string=>typeof n==='string'?n:n.children.map(words).join('');
const button=(v:ReactTestRenderer,label:string)=>v.root.findAllByType('button').find(b=>words(b as unknown as Node).includes(label))!;

test('supported patients are counted once, from accepted or completed sessions only',()=>{
  const list=supportedPatients([
    session('r1','aaaaa111','Amina Otieno','COMPLETED','2026-11-10T07:00:00Z'),
    session('r2','aaaaa111','Amina Otieno','ACCEPTED','2026-11-14T07:00:00Z'),
    session('r3','bbbbb222','Brian Kip','DECLINED','2026-11-12T07:00:00Z'),
    session('r4','ccccc333','Cate Wairimu','COMPLETED','2026-11-11T07:00:00Z'),
  ]);
  assert.deepEqual(list.map(p=>[p.name,p.sessions,p.completed]),[['Amina Otieno',2,1],['Cate Wairimu',1,1]]);
  assert.equal(patientCode('aaaaa111-0000'),'#PT-AAAAA');
});

test('the directory lists every peer with activity and opens a record',async()=>{
  clearSoftReads();
  const previous=globalThis.fetch;const urls:string[]=[];
  const rows=[peer('p1','APPROVED','Grace Wanjiku',{patients_supported:2,sessions_completed:3,chat_sessions:2,call_sessions:1,last_session_at:'2026-11-14T07:00:00Z'}),peer('p2','SUSPENDED','Brian Otieno')];
  const history:AdminPeerHistory={sessions:[session('r1','aaaaa111','Amina Otieno','COMPLETED','2026-11-14T07:00:00Z',{messages:12})],status_history:[{event:'CREATED',at:'2026-11-01T08:00:00Z',by:'PEER'},{event:'APPROVED',at:'2026-11-02T08:00:00Z',by:'ADMIN'}]};
  globalThis.fetch=async url=>{urls.push(String(url));return String(url).includes('/history')?ok(history):ok(rows);};
  const opened:string[]=[];
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<PeerDirectory onOpenPatient={id=>opened.push(id)}/>);});
    const t=text(view!);
    assert.ok(t.includes('Grace Wanjiku')&&t.includes('Brian Otieno'),'every status is listed');
    assert.ok(t.includes('Suspended'));
    await act(async()=>{view!.root.findAllByProps({className:'admin-clickable-row'})[0].props.onClick();});
    assert.ok(urls.some(u=>u.includes('/admin/peers/p1/history')));
    assert.ok(text(view!).includes('Back to Peer Directory'));
    assert.ok(text(view!).includes('Amina Otieno'));
    await act(async()=>{button(view!,'Amina Otieno').props.onClick();});
    assert.deepEqual(opened,['aaaaa111']);
  }finally{globalThis.fetch=previous;view?.unmount();}
});

test('the record shows patients and account history, never topics or messages',async()=>{
  const history:AdminPeerHistory={
    sessions:[session('r1','aaaaa111','Amina Otieno','COMPLETED','2026-11-14T07:00:00Z',{support_type:'CALL',call_started_at:'2026-11-14T07:00:00Z',call_ended_at:'2026-11-14T07:08:00Z'}),
      session('r2','bbbbb222','Brian Kip','CANCELLED','2026-11-13T07:00:00Z')],
    status_history:[{event:'CREATED',at:'2026-11-01T08:00:00Z',by:'PEER'},{event:'APPROVED',at:'2026-11-02T08:00:00Z',by:'ADMIN'},{event:'SUSPENDED',at:'2026-11-05T08:00:00Z',by:'ADMIN'}]};
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<PeerRecord peer={peer('p1','SUSPENDED','Grace Wanjiku')} onBack={()=>{}} load={async()=>history}/>);});
    let t=text(view!);
    assert.ok(t.includes('8 min'),'call length');
    assert.ok(t.includes('Cancelled by patient'));
    assert.ok(!/topic"?:/i.test(t));
    await act(async()=>{button(view!,'Patients supported').props.onClick();});
    t=text(view!);
    assert.ok(t.includes('Amina Otieno')&&!t.includes('Brian Kip'),'only accepted or completed sessions count as supported');
    await act(async()=>{button(view!,'Account history').props.onClick();});
    t=text(view!);
    assert.ok(t.includes('Signed up in the Care app')&&t.includes('Suspended')&&t.includes('By an Admin'));
  }finally{view?.unmount();}
});
