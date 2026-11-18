import assert from 'node:assert/strict';
import {test} from 'node:test';
import {isNoticeNew, MAX_VIEWED, withViewed} from '../src/lib/noticeSeen.ts';

const at = '2026-11-17T10:00:00Z';
const before = new Date('2026-11-17T09:00:00Z').getTime();
const after = new Date('2026-11-17T11:00:00Z').getTime();

test('a notice newer than the bell watermark is new until it is viewed', () => {
  assert.equal(isNoticeNew(at, 'n1', before, []), true);
  // Opening it from its pop-up ("View") clears it without opening the bell.
  assert.equal(isNoticeNew(at, 'n1', before, withViewed([], 'n1')), false);
  // Other unseen notices stay new.
  assert.equal(isNoticeNew(at, 'n2', before, withViewed([], 'n1')), true);
});

test('opening the bell clears every notice up to its watermark', () => {
  assert.equal(isNoticeNew(at, 'n1', after, []), false);
});

test('viewed IDs are de-duplicated and capped to the newest', () => {
  assert.deepEqual(withViewed(['a'], 'a'), ['a']);
  let v: string[] = [];
  for (let i = 0; i < MAX_VIEWED + 5; i++) v = withViewed(v, `id${i}`);
  assert.equal(v.length, MAX_VIEWED);
  assert.equal(v[0], 'id5');
  assert.equal(v.at(-1), `id${MAX_VIEWED + 4}`);
});
