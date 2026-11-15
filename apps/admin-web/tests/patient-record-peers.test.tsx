import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {PatientRecord} from '../src/components/people/PatientRecord';
import type {AdminPatientPeerSession} from '../src/lib/api';
import type {PatientRecordItem} from '../src/data/adminStore';

const patient={key:'0b1c2d3e-0000-4000-8000-000000000001',backendId:'0b1c2d3e-0000-4000-8000-000000000001',id:'#PT-0B1C2',name:'Amina Otieno',chatActive:false,avatar:'',bgColor:'',meta:'',cohort:'',labs:'',category:'',
  dateOfBirth:'2001-01-01',gender:'FEMALE',consultationsCount:1,videoVisits:1,chatSessions:0,registeredAt:'2026-11-01T08:00:00Z',lastConsultAt:'2026-11-10T08:00:00Z',
  visits:[{appointmentId:'a1',appointmentNumber:'APT-1',createdAt:'2026-11-10T08:00:00Z',modality:'VIDEO',status:'COMPLETED',outcome:'COMPLETED_NO_RX',doctor:'Dr Jane Mwangi'}],vaultRecords:[]} as unknown as PatientRecordItem;
const sessions:AdminPatientPeerSession[]=[{request_id:'r1',peer_id:'p1',peer_name:'Neema Kamau',support_type:'CHAT',status:'COMPLETED',start_time:'2026-11-12T08:00:00Z',requested_at:'2026-11-11T08:00:00Z',completed_at:'2026-11-12T08:10:00Z',messages:7}];
const text=(v:ReactTestRenderer)=>JSON.stringify(v.toJSON());
type Node=string|{children:Node[]};
const words=(n:Node):string=>typeof n==='string'?n:n.children.map(words).join('');
const button=(v:ReactTestRenderer,label:string)=>v.root.findAllByType('button').find(b=>words(b as unknown as Node).includes(label))!;

test('the patient record lists peer sessions with doctor consultations',async()=>{
  const previous=globalThis.fetch;const calls:{url:string;body:string|null}[]=[];
  globalThis.fetch=async(url,init)=>{calls.push({url:String(url),body:init?.body?String(init.body):null});return new Response(JSON.stringify({success:true,data:{recorded:true}}));};
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<PatientRecord patient={patient} onBack={()=>{}} onExport={()=>{}} onErase={()=>{}} busy={false} loadPeerSessions={async()=>sessions}/>);});
    let t=text(view!);
    assert.ok(t.includes('Neema Kamau'),'peer on the timeline');
    assert.ok(t.includes('Peer Navigator'));
    assert.ok(t.includes('Dr Jane Mwangi'),'doctor visit still there');
    assert.ok(t.indexOf('Peer support chat')<t.indexOf('Video consultation'),'newest first');
    // The server records the view when the dossier loads; the page sends no
    // second view event of its own.
    assert.ok(!calls.some(c=>c.url.includes('/admin/audit/views')));
    await act(async()=>{button(view!,'Consultations').props.onClick();});
    t=text(view!);
    assert.ok(t.includes('Peer support sessions')&&t.includes('Neema Kamau'));
  }finally{globalThis.fetch=previous;view?.unmount();}
});
