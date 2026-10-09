import test from 'node:test';
import assert from 'node:assert/strict';
import { uploadPrescriptionPages } from '../src/lib/rx-upload.ts';
import type { RxPage } from '../src/lib/contracts.ts';
const page=(key:string)=>({key,state:'LOCAL',requestKey:key} as RxPage);
test('partial page failure preserves confirmed pages and retry reuses request key and frozen notes',async()=>{
  const attempts:string[]=[];const states:string[][]=[];
  const first=await uploadPrescriptionPages([page('one'),page('two')],'original',undefined,async p=>{attempts.push(p.requestKey);if(p.key==='two')throw new Error('unavailable');return {id:'server-one'};},rows=>states.push(rows.map(p=>p.state)),()=>true,()=> 'STORAGE_UNAVAILABLE');
  assert.deepEqual(first.map(p=>p.state),['UPLOADED','FAILED']);assert.equal(first[0].documentId,'server-one');
  const retried=await uploadPrescriptionPages(first,'edited after failure','two',async p=>{attempts.push(p.requestKey);assert.equal(p.uploadNotes,'original');return {id:'server-two'};},()=>{},()=>true,String);
  assert.deepEqual(attempts,['one','two','two']);assert.deepEqual(retried.map(p=>p.state),['UPLOADED','UPLOADED']);assert.deepEqual(states[0],['UPLOADING','LOCAL']);
});
test('account boundary during an upload prevents late result and remaining page dispatch',async()=>{
  let active=true;let calls=0;const updates:RxPage[][]=[];
  await assert.rejects(uploadPrescriptionPages([page('one'),page('two')],'',undefined,async()=>{calls++;active=false;return {id:'late'};},rows=>updates.push(rows),()=>active,String),{name:'AbortError'});
  assert.equal(calls,1);assert.equal(updates.length,1);assert.equal(updates[0][0].state,'UPLOADING');
});
test('100% transfer is still UPLOADING until confirmed; failure remains retryable and resets progress',async()=>{
  const updates:RxPage[][]=[];
  const failed=await uploadPrescriptionPages([page('one')],'notes',undefined,async(p,progress)=>{
    progress(0.3);progress(1);
    assert.equal(updates.at(-1)?.[0].state,'UPLOADING');assert.equal(updates.at(-1)?.[0].documentId,undefined);
    throw new Error('storage unavailable');
  },rows=>updates.push(rows),()=>true,String);
  assert.equal(failed[0].state,'FAILED');assert.equal(failed[0].uploadProgress,1);assert.equal(failed[0].documentId,undefined);
  const retried=await uploadPrescriptionPages(failed,'changed','one',async(p,progress)=>{
    assert.equal(p.uploadProgress,null);assert.equal(p.uploadNotes,'notes');assert.equal(p.requestKey,'one');progress(0.5);return {id:'confirmed'};
  },()=>{},()=>true,String);
  assert.equal(retried[0].state,'UPLOADED');assert.equal(retried[0].uploadProgress,1);
});
test('late progress across an account boundary cannot update another Doctor state',async()=>{
  let active=true;let count=0;
  await assert.rejects(uploadPrescriptionPages([page('one')],'',undefined,async(p,progress)=>{
    active=false;progress(0.75);return {id:'late'};
  },()=>count++,()=>active,String),{name:'AbortError'});
  assert.equal(count,1);
});
