import test from 'node:test';
import assert from 'node:assert/strict';
import { articleValidation, topicLabel } from '../src/lib/articles.ts';
import { safeBackendPath } from '../src/lib/contracts.ts';

const id = '00000000-0000-4000-8000-000000000001';
const base = { title: 'Title', summary: 'Summary', body: 'Body', references_list: [] };

test('Doctor Web redesign proxies only the newly wired routes and methods', () => {
  assert.equal(safeBackendPath(`doctor/articles/${id}/cover`, 'POST'), true);
  assert.equal(safeBackendPath(`doctor/articles/${id}/cover`, 'GET'), false);
  assert.equal(safeBackendPath('doctor/verification/submissions', 'POST'), true);
  assert.equal(safeBackendPath('doctor/verification/submissions', 'GET'), true);
  assert.equal(safeBackendPath('doctor/verification/submissions', 'DELETE'), false);
  assert.equal(safeBackendPath('doctor/profile/photo', 'POST'), true);
  assert.equal(safeBackendPath('doctor/profile/photo', 'DELETE'), true);
  // The photo is binary and served by /api/photo, never through the JSON proxy.
  assert.equal(safeBackendPath('doctor/profile/photo', 'GET'), false);
  assert.equal(safeBackendPath('auth/change-password', 'POST'), true);
  assert.equal(safeBackendPath(`admin/doctors/${id}/verification-documents/${id}`, 'GET'), false);
});

test('a Swahili version is all three fields or none', () => {
  assert.equal(articleValidation({ ...base, extras: { topic_key: 'PREP', swahili: null } }), undefined);
  assert.equal(articleValidation({ ...base, extras: { topic_key: null, swahili: { title: 'Kichwa', summary: 'Muhtasari', body: 'Makala' } } }), undefined);
  assert.match(articleValidation({ ...base, extras: { topic_key: null, swahili: { title: 'Kichwa', summary: '', body: 'Makala' } } }) ?? '', /Swahili summary/);
});

test('topic labels cover the six patient education topics only', () => {
  assert.equal(topicLabel('PREP'), 'PrEP');
  assert.equal(topicLabel('HIV_BASICS'), 'HIV basics');
  assert.equal(topicLabel('UNKNOWN'), null);
  assert.equal(topicLabel(null), null);
});
