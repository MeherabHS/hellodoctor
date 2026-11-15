import test from 'node:test';
import assert from 'node:assert/strict';
import {describeNotice} from '../src/lib/notificationText.ts';

test('events route to the page that handles them', () => {
  const at = '2026-11-14T08:00:00Z';
  assert.deepEqual(describeNotice({id: '1', kind: 'grievance', at, grievance_number: 'GRV-2026-0001'}).page, 'grievances');
  const inc = describeNotice({id: '2', kind: 'incident', at, severity: 'CRITICAL', component: 'M-Pesa'});
  assert.equal(inc.page, 'logs'); assert.equal(inc.tone, 'urgent');
  const doc = describeNotice({id: '3', kind: 'activity', at, action: 'DOCTOR_PROFILE_CREATED', resource_type: 'DOCTOR_PROFILE', result: 'SUCCESS', actor_role: 'DOCTOR', actor_email: null});
  assert.equal(doc.title, 'New doctor application'); assert.equal(doc.page, 'verification');
  const pub = describeNotice({id: '4', kind: 'activity', at, action: 'ADMIN_ARTICLE_PUBLISHED', resource_type: 'ADMIN_ARTICLE', result: 'SUCCESS', actor_role: 'ADMIN', actor_email: 'ops@example.org'});
  assert.equal(pub.title, 'Admin article published'); assert.equal(pub.body, 'By ops@example.org.'); assert.equal(pub.page, 'blogs');
  const erase = describeNotice({id: '5', kind: 'activity', at, action: 'DATA_RIGHTS_PATIENT_ERASED', resource_type: 'PATIENT_PROFILE', result: 'SUCCESS', actor_role: 'ADMIN', actor_email: 'ops@example.org'});
  assert.equal(erase.title, 'Patient account deleted'); assert.equal(erase.page, 'patients');
  const forged = describeNotice({id: '6', kind: 'activity', at, action: 'AUTH_ROLE_MISMATCH', resource_type: 'AUTH_SESSION', result: 'DENIED', actor_role: 'PATIENT', actor_email: null});
  assert.equal(forged.tone, 'urgent'); assert.equal(forged.body, 'By a patient.'); assert.equal(forged.page, 'activity');
});

test('a Peer Navigator sign-up is an attention notice that opens Peer Navigators', () => {
  const n = describeNotice({id: '7', kind: 'activity', at: '2026-11-15T08:00:00Z', action: 'PEER_NAVIGATOR_CREATED', resource_type: 'PEER_NAVIGATOR', result: 'SUCCESS', actor_role: 'PEER', actor_email: null});
  assert.equal(n.title, 'New Peer Navigator application');
  assert.equal(n.page, 'peers');
  assert.equal(n.tone, 'warning');
  assert.equal(n.body, 'A Peer Navigator signed up and is waiting for approval.');
});
