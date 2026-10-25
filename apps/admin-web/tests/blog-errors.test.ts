import test from 'node:test';
import assert from 'node:assert/strict';
import {blogErrorMessage,independentApprovalMessage} from '../src/lib/blogErrors.ts';
test('CMS self-approval errors become a plain editorial instruction',()=>{
  const error=new Error('API Error [403]: {"success":false,"error":{"code":"FORBIDDEN","message":"Sensitive health articles require approval by another Admin. This article remains pending review."}}');
  assert.equal(blogErrorMessage(error),independentApprovalMessage);
});
test('CMS error parsing handles structured, string, malformed and server errors without raw envelopes',()=>{
  assert.equal(blogErrorMessage(new Error('API Error [422]: {"error":{"message":"Select a category."}}')),'Select a category.');
  assert.equal(blogErrorMessage(new Error('API Error [409]: {"error":"Article changed."}')),'Article changed.');
  assert.equal(blogErrorMessage(new Error('API Error [403]: broken JSON')),'You do not have permission to perform this action.');
  assert.equal(blogErrorMessage(new Error('API Error [500]: {"error":{"message":"private diagnostic"}}')),'The server is temporarily unavailable. Please retry.');
  assert.ok(!blogErrorMessage(null).includes('undefined'));
});
test('unconfigured media is reported as Azure integration deferral, never upload success',()=>{
  const message=blogErrorMessage(new Error('Article media storage is not configured.'));
  assert.ok(message.includes('awaiting Azure Blob integration'));assert.ok(message.includes('No cover was uploaded.'));
});
