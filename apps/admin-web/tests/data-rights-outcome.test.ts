import test from 'node:test';
import assert from 'node:assert/strict';
import {dataRightsNotice} from '../src/lib/dataRightsOutcome.ts';

for (const subject of ['Patient','Doctor'] as const) {
  test(`${subject}: an accepted pending erasure never claims completed anonymization`,()=>{
    const notice=dataRightsNotice({outcome:'PENDING',job_id:'synthetic-erasure-job'},subject);
    assert.match(notice.title,/pending/i);
    assert.equal(notice.level,'warning');
    assert.match(notice.message,/synthetic-erasure-job/);
    assert.doesNotMatch(notice.message,/was permanently deleted|were scrubbed/);
  });
  test(`${subject}: confirmed hard deletion is distinguished from retained-history anonymization`,()=>{
    const notice=dataRightsNotice({outcome:'HARD_DELETED'},subject);
    assert.equal(notice.title,`${subject} Data Erased`);
    assert.match(notice.message,/permanently deleted/);
    assert.equal(notice.level,'success');
  });
  test(`${subject}: confirmed anonymization explains retained records`,()=>{
    const notice=dataRightsNotice({outcome:'ANONYMIZED'},subject);
    assert.equal(notice.title,`${subject} Data Anonymized`);
    assert.match(notice.message,/record stays on file/);
    assert.doesNotMatch(notice.message,/permanently deleted/);
  });
}
test('a failed follow-up dashboard read preserves the pending mutation receipt and reference',()=>{
  const notice=dataRightsNotice({outcome:'PENDING',job_id:'synthetic-recovery-reference'},'Patient',false);
  assert.match(notice.title,/pending/i);
  assert.match(notice.message,/synthetic-recovery-reference/);
  assert.match(notice.message,/refresh failed/i);
  assert.equal(notice.level,'warning');
});
test('a failed dashboard refresh does not describe a confirmed erasure as a rejected request',()=>{
  const notice=dataRightsNotice({outcome:'HARD_DELETED'},'Doctor',false);
  assert.equal(notice.title,'Doctor Data Erased');
  assert.match(notice.message,/reload/i);
  assert.equal(notice.level,'warning');
});
test('an unexpected server outcome cannot fall through to completed anonymization',()=>{
  assert.throws(()=>dataRightsNotice({outcome:'UNRECOGNIZED' as 'ANONYMIZED'},'Patient'),/Unrecognized erasure outcome/);
});
