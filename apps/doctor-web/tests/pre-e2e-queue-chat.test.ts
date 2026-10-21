import test from 'node:test';
import assert from 'node:assert/strict';
import { messageSide, safeBackendPath, upcomingFirst } from '../src/lib/contracts.ts';
import type { Appointment } from '../src/lib/contracts.ts';

test('server queue eligibility and priority survive browser rendering without device-clock sorting',()=>{
  const values=[{id:'active',status:'IN_CONSULTATION',queue_actionable:true,start_time:'2030-01-01T00:00:00Z'},
    {id:'nearest',status:'CONFIRMED',queue_actionable:true,start_time:'2029-01-01T00:00:00Z'},
    {id:'history',status:'COMPLETED',queue_actionable:false,start_time:'2020-01-01T00:00:00Z'}] as Appointment[];
  assert.deepEqual(upcomingFirst(values).map(a=>a.id),['active','nearest','history']);
  assert.deepEqual(upcomingFirst(values).filter(a=>a.queue_actionable).map(a=>a.id),['active','nearest']);
  assert.deepEqual(values.map(a=>a.id),['active','nearest','history']);
});
test('sender user identity determines Doctor ownership on every load',()=>{
  for(let reload=0;reload<2;reload++) {
    assert.equal(messageSide({sender_id:'doctor-user'},'doctor-user'),'doctor');
    assert.equal(messageSide({sender_id:'patient-user'},'doctor-user'),'patient');
    assert.equal(messageSide({sender_id:'doctor-profile-id'},'doctor-user'),'patient');
  }
});
test('BFF attachment routes are narrowly scoped and do not permit arbitrary public objects',()=>{
  const c='11111111-1111-4111-8111-111111111111';const m='22222222-2222-4222-8222-222222222222';
  assert.equal(safeBackendPath(`chat/${c}/attachments`,'POST'),true);
  assert.equal(safeBackendPath(`chat/${c}/messages/${m}/attachment`,'GET'),true);
  assert.equal(safeBackendPath(`chat/${c}/attachments`,'GET'),false);
  assert.equal(safeBackendPath(`chat/${c}/messages/${m}/attachment`,'POST'),false);
  assert.equal(safeBackendPath(`chat/${c}/messages/../../object/attachment`,'GET'),false);
});
