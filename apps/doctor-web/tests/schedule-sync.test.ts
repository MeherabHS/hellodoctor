import test from 'node:test';
import assert from 'node:assert/strict';
import { startScheduleSync } from '../src/lib/schedule-sync.ts';

test('visible schedule refreshes on poll, focus and reconnect, and stops after unmount', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
  let tick = () => {};
  let calls = 0;
  let cancelled = false;
  const stop = startScheduleSync(async () => {calls++;}, {
    window:window as Window, document:document as Document, online:()=>true,
    every:(callback,ms)=>{assert.equal(ms,15000);tick=callback;return 42;},
    cancel:timer=>{assert.equal(timer,42);cancelled=true;}
  });
  tick(); await Promise.resolve();
  window.dispatchEvent(new Event('focus')); await Promise.resolve();
  window.dispatchEvent(new Event('online')); await Promise.resolve();
  assert.equal(calls,3);
  stop(); tick(); window.dispatchEvent(new Event('focus')); await Promise.resolve();
  assert.equal(calls,3);assert.equal(cancelled,true);
});

test('hidden/offline schedule does not poll and concurrent triggers do not overlap', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'hidden'});
  let tick = () => {};
  let online = true;
  let calls = 0;
  let finish = () => {};
  const stop = startScheduleSync(()=>{calls++;return new Promise<void>(resolve=>{finish=resolve;});}, {
    window:window as Window, document:document as Document, online:()=>online,
    every:callback=>{tick=callback;return 1;},cancel:()=>{}
  });
  tick();assert.equal(calls,0);
  document.visibilityState='visible';online=false;tick();assert.equal(calls,0);
  online=true;window.dispatchEvent(new Event('online'));tick();window.dispatchEvent(new Event('focus'));
  assert.equal(calls,1);
  finish();await Promise.resolve();tick();assert.equal(calls,2);
  finish();stop();
});
