import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {CommandCenter} from '../src/components/operations/CommandCenter';
import {timelineRows,type ConsultationTimelineData} from '../src/components/operations/ConsultationTimeline';
import type {CommandTelemetry} from '../src/lib/api';

const stream={session_id:'b85b29e9-feb4-4460-b575-24c0372fee78',appointment_id:'a',channel_name:'rtc_b093f51bd52e40a592',doctor_name:'Dr. A',patient_name:'P',
  started_at:'2026-11-12T09:27:55Z',packet_loss_percent:null,rtt_ms:null,connection_state:'Unmeasured',session_status:'ACTIVE',
  timeline_url:'/admin/consultations/b85b29e9-feb4-4460-b575-24c0372fee78/timeline'} as CommandTelemetry['active_streams'][number];
const telemetry={active_specialists:1,completed_today:0,active_sessions_count:1,active_streams:[stream],needs_review_count:0} as unknown as CommandTelemetry;
const timeline:ConsultationTimelineData={session_id:stream.session_id,status:'ACTIVE',recovery_state:'NONE',started_at:stream.started_at,ended_at:null,patient_present:false,doctor_present:false,interruption_reason:null,
  events:[{event_type:'PATIENT_JOINED',occurred_at:'2026-11-12T09:30:00Z'}],provider_presence_events:[{event_type:'JOINED',participant_role:'DOCTOR',provider_event_at:'2026-11-12T09:29:00Z'}],
  reconnect_windows:[],related_grievances:[],related_system_errors:[],recovery_resolution:null};

test('the dashboard shows no elapsed timer and no emergency-helpline tile',async()=>{
  let v!:ReactTestRenderer;
  await act(async()=>{v=create(<CommandCenter telemetry={telemetry} overview={null} needsReviewOnly={false} setNeedsReviewOnly={()=>{}} busyId={null} onRecordDecision={()=>{}} apiBase="/api/proxy"/>);});
  const text=JSON.stringify(v.toJSON());
  for(const gone of ['Elapsed','Emergency helpline','Configured'])assert.ok(!text.includes(gone),gone);
  assert.equal(v.root.findAllByType('a').length,0,'Timeline is no longer a link to raw JSON');
  act(()=>v.unmount());
});

test('Timeline opens a readable panel in the page, not raw JSON in a new tab',async()=>{
  const before={fetch:globalThis.fetch,window:globalThis.window};const urls:string[]=[];
  globalThis.window={addEventListener(){},removeEventListener(){}} as unknown as Window&typeof globalThis;
  globalThis.fetch=(async(url:RequestInfo|URL)=>{urls.push(String(url));return new Response(JSON.stringify({success:true,data:timeline}),{headers:{'Content-Type':'application/json'}});}) as typeof fetch;
  let v!:ReactTestRenderer;
  try{
    await act(async()=>{v=create(<CommandCenter telemetry={telemetry} overview={null} needsReviewOnly={false} setNeedsReviewOnly={()=>{}} busyId={null} onRecordDecision={()=>{}} apiBase="/api/proxy"/>);});
    await act(async()=>{v.root.findAllByType('button').find(b=>b.children.join('')==='Timeline')!.props.onClick();});
    const text=JSON.stringify(v.toJSON());
    assert.ok(urls.some(u=>u.endsWith(stream.timeline_url!)),urls.join());
    assert.equal(v.root.findAllByProps({role:'dialog'}).length,1);
    for(const shown of ['Consultation timeline','Not ended','Nobody','Patient joined','Doctor · Joined'])assert.ok(text.includes(shown),shown);
    assert.ok(!text.includes('provider_presence_semantics'),'no raw JSON');
    await act(async()=>{v.root.findAllByType('button').find(b=>b.props['aria-label']==='Close timeline')!.props.onClick();});
    assert.equal(v.root.findAllByProps({role:'dialog'}).length,0);
  }finally{act(()=>v.unmount());globalThis.fetch=before.fetch;globalThis.window=before.window;}
});

test('timeline rows merge app and Agora events in time order',()=>{
  assert.deepEqual(timelineRows(timeline).map(r=>r.label),['Doctor · Joined','Patient joined']);
});
