import test from 'node:test';
import assert from 'node:assert/strict';
import { chatLocked, money, payoutLabel, paymentRequiredFromPolicy, safeBackendPath, statementLabel, upcomingFirst } from '../src/lib/contracts.ts';
import type { Appointment, Conversation } from '../src/lib/contracts.ts';

const appointment=(id:string,status:string,time:string)=>({id,status,doctor_lifecycle:status,start_time:time} as Appointment);
test('default Queue is nearest active/upcoming first, then terminal history',()=>{
  const rows=[appointment('done','COMPLETED','2026-10-08T13:00:00Z'),appointment('late','CONFIRMED','2026-10-08T16:00:00Z'),appointment('cancel','CANCELLED','2026-10-08T08:00:00Z'),appointment('early','CONFIRMED','2026-10-08T09:00:00Z'),appointment('middle','CONFIRMED','2026-10-08T10:30:00Z')];
  assert.deepEqual(upcomingFirst(rows).map(a=>a.id),['early','middle','late','done','cancel']);
  assert.equal(rows[0].id,'done');
});
test('locked, expired and invalid expiry chat cannot be editable',()=>{
  const chat={is_locked:false,expires_at:'2026-10-08T09:00:00Z'} as Conversation;
  assert.equal(chatLocked(chat,Date.parse('2026-10-08T08:59:59Z')),false);
  assert.equal(chatLocked(chat,Date.parse(chat.expires_at)),true);
  assert.equal(chatLocked({...chat,is_locked:true},0),true);
  assert.equal(chatLocked({...chat,expires_at:''},0),true);
});
test('statement labels follow server finalization, never calendar inference',()=>{
  assert.equal(statementLabel({status:'IN_PROGRESS',amount_label:'ESTIMATED_PAYOUT'}),'Estimated payout');
  assert.equal(statementLabel({status:'FINALIZED',amount_label:'FINAL_PAYABLE'}),'Final payable');
  assert.equal(statementLabel({status:'FINALIZED',amount_label:'ESTIMATED_PAYOUT'}),'Estimated payout');
});
test('prepared, approved, processing and simulation never claim payment received',()=>{
  for(const status of ['PREPARED','APPROVED','PROCESSING','PAYMENT_HELD'])assert.notEqual(payoutLabel(status,'MPESA'),'Paid');
  assert.match(payoutLabel('PAID','DEV_SIMULATED'),/simulation/);
  assert.equal(payoutLabel('PAID','MPESA'),'Paid');
});
test('KES formatting preserves large decimal strings without financial calculations',()=>{
  assert.equal(money('9007199254740993.25'),'KES 9,007,199,254,740,993.25');
  assert.equal(money('0'),'KES 0.00');assert.equal(money(null),'—');assert.equal(money('invalid'),'—');
});
test('BFF blocks admin endpoints, arbitrary patient lookup, traversal, methods and token routes',()=>{
  const id='11111111-1111-4111-8111-111111111111';
  assert.equal(safeBackendPath(`doctor/appointments/${id}/patient-history`,'GET'),true);
  assert.equal(safeBackendPath(`doctor/slots/${id}`,'DELETE'),true);
  for(const path of ['admin/doctors','auth/refresh','doctor/wallet/../profile',`patients/${id}/history`,'doctor/appointments?token=secret'])assert.equal(safeBackendPath(path,'GET'),false);
  assert.equal(safeBackendPath('doctor/wallet/summary','POST'),false);
});
test('notification BFF admits only own-inbox operations supported by current Rust routes',()=>{
  const id='11111111-1111-4111-8111-111111111111';
  assert.equal(safeBackendPath('notifications','GET'),true);
  assert.equal(safeBackendPath('notifications/unread-count','GET'),true);
  assert.equal(safeBackendPath(`notifications/${id}/read`,'POST'),true);
  assert.equal(safeBackendPath('notifications/read-all','POST'),true);
  assert.equal(safeBackendPath('notifications','POST'),false);
  assert.equal(safeBackendPath('notifications/devices','POST'),false);
  assert.equal(safeBackendPath(`notifications/${id}/read`,'GET'),false);
});
test('BFF admits the server-authoritative booking-policy read only as GET',()=>{
  assert.equal(safeBackendPath('config/booking-policy','GET'),true);
  assert.equal(safeBackendPath('config/booking-policy','POST'),false);
  assert.equal(safeBackendPath('config/booking-policy','PATCH'),false);
});
test('FINANCIAL_MODULE_MODE: the Doctor UI fails safe to paymentRequired until the server explicitly defers it',()=>{
  // Undefined/missing/not-yet-loaded policy must never hide real financial actions.
  assert.equal(paymentRequiredFromPolicy(undefined),true);
  assert.equal(paymentRequiredFromPolicy({}),true);
  assert.equal(paymentRequiredFromPolicy({payment_required:true}),true);
  // Only an explicit `false` from the server defers the financial module.
  assert.equal(paymentRequiredFromPolicy({payment_required:false}),false);
});
