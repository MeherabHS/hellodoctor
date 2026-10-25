import test from 'node:test';
import assert from 'node:assert/strict';
import { consultationAction, consultationActionLabel, chatActionLabel, chatLocked, safeBackendPath } from '../src/lib/contracts.ts';
import type { Conversation } from '../src/lib/contracts.ts';

test('paid CHAT and VIDEO expose only their respective start action', () => {
  assert.equal(consultationAction({modality:'CHAT',status:'CONFIRMED'}),'chat');
  assert.equal(consultationActionLabel({modality:'CHAT',status:'CONFIRMED'}),'Start chat consultation');
  assert.equal(consultationAction({modality:'VIDEO',status:'CONFIRMED'}),'call');
  assert.equal(consultationActionLabel({modality:'VIDEO',status:'CONFIRMED'}),'Start video consultation');
  for (const modality of ['CHAT','VIDEO']) for (const status of ['PENDING_PAYMENT','CANCELLED','REFUNDED','EXPIRED','PREMATURE_TERMINATION','DISPUTED','COMPLETED'])
    assert.equal(consultationAction({modality,status}),null);
  assert.equal(consultationAction({modality:'UNKNOWN',status:'CONFIRMED'}),null);
  assert.equal(consultationAction({modality:'VIDEO',status:'WAITING'}),null);
  assert.equal(consultationAction({modality:'CHAT',status:'WAITING'}),'chat');
  assert.equal(consultationAction({modality:'CHAT',status:'CONFIRMED',chat_locked:true}),null);
  assert.equal(consultationAction({modality:'CHAT',status:'CONFIRMED',chat_expires_at:'2000-01-01T00:00:00Z'}),null);
});
test('paid chat start opens existing conversation; closed and video follow-up wording is distinct', () => {
  const c={id:'c',appointment_id:'a',patient_display_name:'Meherab Shafin',modality:'CHAT',appointment_status:'CONFIRMED',is_locked:false,expires_at:'2099-01-01T00:00:00Z',last_message_at:null} satisfies Conversation;
  assert.equal(chatActionLabel(c,0),'Start chat consultation');
  assert.equal(chatActionLabel({...c,last_message_at:'2026-01-01'},0),'Open chat');
  assert.equal(chatActionLabel({...c,modality:'VIDEO'},0),'Open chat');
  for (const appointment_status of ['CANCELLED','REFUNDED','PENDING_PAYMENT','EXPIRED','PREMATURE_TERMINATION','DISPUTED']) {
    assert.equal(chatLocked({...c,appointment_status},0),true);
    assert.equal(chatActionLabel({...c,appointment_status},0),'View consultation history');
  }
  assert.equal(chatLocked(c,Date.parse(c.expires_at)),true);
});
test('BFF permits authorized conversation resolution, never creates a second consultation', () => {
  const path='appointments/11111111-1111-4111-8111-111111111111/conversation';
  assert.equal(safeBackendPath(path,'GET'),true);
  for (const method of ['POST','PUT','DELETE']) assert.equal(safeBackendPath(path,method),false);
  assert.equal(safeBackendPath(path+'/../patient','GET'),false);
});
