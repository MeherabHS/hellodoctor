import test from 'node:test';
import assert from 'node:assert/strict';
import { createInflightReads } from '../src/lib/inflight.mjs';
test('equivalent in-flight reads share one transport and completed reads refetch',async()=>{
 const reads=createInflightReads();let calls=0;let finish=()=>{};
 const load=()=>{calls++;return new Promise<void>(resolve=>{finish=resolve;});};
 const a=reads.run('account1/queue',load),b=reads.run('account1/queue',load);assert.equal(a,b);await Promise.resolve();assert.equal(calls,1);
 finish();await a;const c=reads.run('account1/queue',load);await Promise.resolve();assert.equal(calls,2);finish();await c;
});
test('different account, route and cursor reads stay separate',async()=>{
 const reads=createInflightReads();let calls=0;const load=async()=>++calls;
 const results=await Promise.all(['a/chat?after=1','a/chat?after=2','b/chat?after=1','a/queue'].map(key=>reads.run(key,load)));
 assert.equal(calls,4);assert.deepEqual(results,[1,2,3,4]);
});
test('failed reads remain failed and subsequent retry is not cached',async()=>{
 const reads=createInflightReads();let calls=0;const load=async()=>{calls++;throw new Error('Denied');};
 await assert.rejects(reads.run('protected',load),/Denied/);await assert.rejects(reads.run('protected',load),/Denied/);assert.equal(calls,2);
});
test('logout clears coalescing namespace without late completion deleting new work',async()=>{
 const reads=createInflightReads();let finishOld=()=>{},finishNew=()=>{};
 const a=reads.run('protected',()=>new Promise<void>(resolve=>{finishOld=resolve;}));await Promise.resolve();reads.clear();
 const b=reads.run('protected',()=>new Promise<void>(resolve=>{finishNew=resolve;}));await Promise.resolve();assert.notEqual(a,b);
 finishOld();await a;assert.equal(reads.run('protected',async()=>{}),b);finishNew();await b;
});
