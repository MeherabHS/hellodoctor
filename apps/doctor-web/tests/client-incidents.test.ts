import test from 'node:test';
import assert from 'node:assert/strict';
import {createIncidentReporter, safeIncident, sanitizedStack, operationContext} from '../src/lib/client-incidents.ts';
import {deliverCallTelemetry} from '../src/lib/call-telemetry.ts';
import {safeBackendPath} from '../src/lib/contracts.ts';

test('privacy boundary discards arbitrary tokens/PHI before transmission',()=>{
  const error=new TypeError('JWT=secret refresh=private Authorization=secret OTP=123456 patient=Jane Test diagnosis=asthma prescription=drug chat=private +254712345678 account=123456 https://private.test?token=secret');
  error.stack='patient=Jane Test diagnosis=asthma\n at send (https://app.test/_next/static/chunks/app/page.js:22:8)\nAuthorization: Bearer private';
  const payload=safeIncident(error,{screen:'Jane Test',operation:'diagnosis_asthma'});
  const serialized=JSON.stringify(payload);
  for(const secret of ['secret','private','Jane Test','asthma','123456','https://']) assert.ok(!serialized.includes(secret));
  assert.equal(payload.stack_trace,'/_next/static/chunks/app/page.js:22:8');
  assert.equal(payload.screen,'Application'); assert.equal(payload.operation,'unhandled_async');
});
test('stack retains bounded source locations',()=>{
  const stack=Array(1000).fill('at send (https://app.test/_next/static/chunks/page.js:22:8)').join('\n');
  const result=sanitizedStack(stack);
  assert.ok(result.length<=6000); assert.ok(result.split('\n').length<=24);
});
test('authenticated report succeeds; pre-auth report is dropped',async()=>{
  let authenticated=false, sends=0;
  const report=createIncidentReporter(()=>authenticated,async()=>{sends++;});
  assert.equal(await report(new TypeError('synthetic')),false);
  authenticated=true;
  assert.equal(await report(new TypeError('synthetic')),true); assert.equal(sends,1);
});
test('normal 409/422/connectivity/abort do not create incidents',async()=>{
  let sends=0; const report=createIncidentReporter(()=>true,async()=>{sends++;});
  for(const status of [0,400,401,403,404,409,422,429]) assert.equal(await report({status}),false);
  assert.equal(await report(new DOMException('Cancelled','AbortError')),false);
  assert.equal(sends,0);
});
test('system failures including 503 carry generic operations and references',async()=>{
  const bodies:unknown[]=[]; const report=createIncidentReporter(()=>true,async b=>{bodies.push(b);});
  for(const path of ['/api/proxy/chat/id/messages','/api/proxy/doctor/slots','/api/auth/login','/api/proxy/consultations/id/telemetry/events']) {
    assert.equal(await report({status:503},operationContext(path,'POST'),'API_FAILURE'),true);
  }
  assert.deepEqual(bodies.map((b:any)=>b.operation),['send_message','create_slot','login','call_telemetry']);
});
test('reporter failure/reentrancy cannot recurse or release another active send',async()=>{
  let sends=0; let report:ReturnType<typeof createIncidentReporter>;
  report=createIncidentReporter(()=>true,async()=>{
    sends++;
    assert.equal(await report(new Error('nested')),false);
    assert.equal(await report(new Error('nested again')),false);
    throw new Error('reporting transport failure');
  });
  assert.equal(await report(new Error('synthetic')),false); assert.equal(sends,1);
});
test('same browser exception crossing multiple hooks only reports once',async()=>{
  let sends=0; const report=createIncidentReporter(()=>true,async()=>{sends++;});
  const error=new Error('synthetic');
  assert.equal(await report(error),true); assert.equal(await report(error),false); assert.equal(sends,1);
});
test('doctor BFF admits authorized call telemetry routes without admin access',()=>{
  const id='11111111-1111-4111-8111-111111111111';
  for(const path of [`consultations/${id}/telemetry/events`,`consultations/${id}/telemetry`,`consultations/${id}/rtc-token`]) assert.equal(safeBackendPath(path,'POST'),true);
  assert.equal(safeBackendPath(`admin/consultations/${id}/timeline`,'GET'),false);
});
test('real SDK callback adapter sends only actual measured values and fails safely',async()=>{
  const calls:any[]=[]; const id='11111111-1111-4111-8111-111111111111';
  assert.equal(await deliverCallTelemetry(id,'DOCTOR_DISCONNECTED',async(path,body)=>{calls.push({path,body});},{rtt_ms:120,packet_loss_percent:2.5}),true);
  assert.equal(calls[0].body.events[0].rtt_ms,120);
  assert.equal(await deliverCallTelemetry(id,'DOCTOR_RECONNECTED',async()=>{throw new Error('system failure');}),false);
  assert.equal(await deliverCallTelemetry(id,'NETWORK_DEGRADED',async()=>assert.fail(),{packet_loss_percent:101}),false);
});
