import test from 'node:test';
import assert from 'node:assert/strict';
import { refreshCoordinator } from '../src/lib/refresh.ts';
test('parallel Admin reads rotate one single-use refresh token once',async()=>{
  const rotate=refreshCoordinator(20);let calls=0;
  const issue=async()=>{calls++;await new Promise(resolve=>setTimeout(resolve,5));return {access_token:'synthetic-access',refresh_token:'synthetic-next'};};
  const results=await Promise.all([rotate('family-A',issue),rotate('family-A',issue),rotate('family-A',issue)]);
  assert.equal(calls,1);assert.deepEqual(results[0],results[1]);
  await rotate('family-B',issue);assert.equal(calls,2);
});
