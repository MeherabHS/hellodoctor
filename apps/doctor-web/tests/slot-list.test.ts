import test from 'node:test';
import assert from 'node:assert/strict';
import { loadDoctorSlots } from '../src/lib/slot-list.ts';
import { safeBackendPath } from '../src/lib/contracts.ts';
import type { Slot } from '../src/lib/contracts.ts';

test('schedule counts include AVAILABLE rows beyond the first historical page',async()=>{
  const calls:string[]=[];
  const slots=await loadDoctorSlots(async path=>{
    calls.push(path);return path.endsWith('offset=0')?Array.from({length:100},(_,i)=>({id:String(i),status:'CANCELLED'} as Slot)):[{id:'today',status:'AVAILABLE'} as Slot,{id:'blocked',status:'BLOCKED'} as Slot];
  },()=>true);
  assert.equal(slots.length,102);assert.equal(slots.filter(s=>s.status==='AVAILABLE').length,1);
  assert.deepEqual(calls,['doctor/slots?limit=100&offset=0','doctor/slots?limit=100&offset=100']);
});
test('schedule pagination suppresses old-account results and stops further reads',async()=>{
  let current=true;let calls=0;
  await assert.rejects(loadDoctorSlots(async()=>{calls++;current=false;return Array(100).fill({status:'AVAILABLE'});},()=>current),{name:'AbortError'});
  assert.equal(calls,1);
});
test('BFF admits only the new own-slot PATCH route, retaining method and path restrictions',()=>{
  const id='11111111-1111-4111-8111-111111111111';
  assert.equal(safeBackendPath(`doctor/slots/${id}`,'PATCH'),true);
  assert.equal(safeBackendPath(`doctor/slots/${id}`,'DELETE'),true);
  assert.equal(safeBackendPath('doctor/slots','PATCH'),false);
  assert.equal(safeBackendPath(`doctor/slots/${id}/block`,'POST'),false);
  assert.equal(safeBackendPath(`admin/slots/${id}`,'PATCH'),false);
});
