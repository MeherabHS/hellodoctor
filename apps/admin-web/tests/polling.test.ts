import test from 'node:test';
import assert from 'node:assert/strict';
import { startScheduleSync } from '../src/lib/polling.ts';

async function flush() { for (let i = 0; i < 8; i++) await Promise.resolve(); }

test('visible schedule refreshes on poll, focus and reconnect, and stops after unmount', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
  let tick = () => {};
  let calls = 0;
  let cancelled = false;
  const stop = startScheduleSync(async () => {calls++;}, {
    window:window as Window, document:document as Document, online:()=>true,
    every:(callback,ms)=>{assert.ok(ms===15000 || (ms>=250 && ms<=2000));tick=callback;return 42;},
    cancel:timer=>{assert.equal(timer,42);cancelled=true;}
  });
  tick(); await flush();
  window.dispatchEvent(new Event('focus')); assert.equal(calls,1); tick(); await flush();
  window.dispatchEvent(new Event('online')); assert.equal(calls,2); tick(); await flush();
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
  finish();await flush();tick();assert.equal(calls,2);
  finish();stop();
});

test('repeated hasError polls back off (doubling, capped) and reset on success', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
  const delays:number[] = [];
  let tick = () => {};
  let failing = true;
  const stop = startScheduleSync(async () => {}, {
    window:window as Window, document:document as Document, online:()=>true,
    every:(callback,ms)=>{delays.push(ms);tick=callback;return ms;},
    cancel:()=>{}
  }, 1000, {hasError:()=>failing, jitterRatio:0});
  assert.equal(delays[0], 1000);
  tick(); await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  assert.equal(delays[1], 2000);
  tick(); await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  assert.equal(delays[2], 4000);
  failing = false;
  tick(); await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  assert.equal(delays[3], 1000);
  stop();
});

test('shared recovery spreads 1000 clients and coalesces online/focus/visibility bursts', async () => {
  const delays:number[] = [];
  const callbacks:Array<() => void> = [];
  const stops:Array<() => void> = [];
  let calls = 0;
  for (let client=0;client<1000;client++) {
    const window = new EventTarget();
    const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
    let scheduled = () => {};
    let arms = 0;
    let delay = 0;
    const stop = startScheduleSync(async () => {calls++;}, {
      window:window as Window,document:document as Document,online:()=>true,
      every:(callback,ms)=>{scheduled=callback;delay=ms;arms++;return arms;},cancel:()=>{},
    },15000,{random:()=> (client + 0.5)/1000});
    window.dispatchEvent(new Event('online'));
    window.dispatchEvent(new Event('focus'));
    document.dispatchEvent(new Event('visibilitychange'));
    assert.equal(arms,2,'one normal timer plus one coalesced recovery timer');
    delays.push(delay);callbacks.push(scheduled);stops.push(stop);
  }
  assert.equal(calls,0,'no immediate requests on a shared recovery event');
  assert.ok(delays.every(delay=>delay>=250 && delay<=2000));
  assert.ok(new Set(delays).size>=900,'deterministic clients must not recover in lockstep');
  callbacks.forEach(callback=>callback());await flush();
  assert.equal(calls,1000);
  stops.forEach(stop=>stop());callbacks.forEach(callback=>callback());await flush();
  assert.equal(calls,1000,'disposed recovery callbacks cannot run');
});

test('recovery retains outage history until a successful refresh', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
  let tick = () => {};
  let delay = 0;
  const stop=startScheduleSync(async()=>{}, {
    window:window as Window,document:document as Document,online:()=>true,
    every:(callback,ms)=>{tick=callback;delay=ms;return ms;},cancel:()=>{},
  },1000,{hasError:()=>true,jitterRatio:0,random:()=>0});
  tick();await flush();assert.equal(delay,2000);
  window.dispatchEvent(new Event('online'));assert.equal(delay,250);
  tick();await flush();assert.equal(delay,4000);
  stop();
});

test('in-flight work and disposed completion cannot create extra polling chains', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
  let finish = () => {};
  let arms = 0;
  let calls = 0;
  const stop=startScheduleSync(()=>{calls++;return new Promise<void>(resolve=>{finish=resolve;});}, {
    window:window as Window,document:document as Document,online:()=>true,
    every:()=>{arms++;return arms;},cancel:()=>{},
  },1000,{initial:true});
  window.dispatchEvent(new Event('online'));window.dispatchEvent(new Event('focus'));
  assert.equal(calls,1);assert.equal(arms,0);
  stop();finish();await flush();assert.equal(arms,0);assert.equal(calls,1);
});

test('backoff jitter never drops below the base backoff for that step', async () => {
  const window = new EventTarget();
  const document = Object.assign(new EventTarget(), {visibilityState:'visible'});
  const delays:number[] = [];
  let tick = () => {};
  const stop = startScheduleSync(async () => {}, {
    window:window as Window, document:document as Document, online:()=>true,
    every:(callback,ms)=>{delays.push(ms);tick=callback;return ms;},
    cancel:()=>{}
  }, 1000, {hasError:()=>true});
  for (let i=0;i<4;i++) { tick(); await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); }
  // step after 4 failures: backoff=min(1000*2^4,maxInterval=8000)=8000, jitter in [0,20%]
  assert.ok(delays[4] >= 8000 && delays[4] <= 8000*1.2, `delay ${delays[4]} out of bounds`);
  stop();
});
