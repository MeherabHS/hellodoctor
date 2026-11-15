import test from 'node:test';
import assert from 'node:assert/strict';
import {isUnpublished,stepsBeforePublish} from '../src/lib/publishFlow.ts';

test('publishing resumes the backend steps from any unpublished state',()=>{
  assert.deepEqual(stepsBeforePublish('DRAFT'),['submit','approve']);
  assert.deepEqual(stepsBeforePublish('PENDING_REVIEW'),['approve']);
  assert.deepEqual(stepsBeforePublish('APPROVED'),[]);
  assert.deepEqual(stepsBeforePublish('PUBLISHED'),[]);
});

test('only draft-like states can be published',()=>{
  for(const s of ['DRAFT','PENDING_REVIEW','APPROVED'])assert.ok(isUnpublished(s),s);
  for(const s of ['SCHEDULED','PUBLISHED','ARCHIVED','SENDING','COMPLETED','CANCELLED'])assert.ok(!isUnpublished(s),s);
});
