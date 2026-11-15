import test from 'node:test';
import assert from 'node:assert/strict';
import {activityQuery, humanize} from '../src/lib/activityQuery.ts';

const base = {area: 'all' as const, search: '', mine: false, before: null};

test('defaults send only the limit', () => {
  assert.equal(activityQuery(base), 'limit=50');
});

test('filters are encoded as query parameters', () => {
  const q = new URLSearchParams(activityQuery({...base, area: 'content', search: ' publish ', mine: true, before: '2026-11-14T08:00:00Z'}));
  assert.equal(q.get('area'), 'content');
  assert.equal(q.get('search'), 'publish');
  assert.equal(q.get('mine'), 'true');
  assert.equal(q.get('before'), '2026-11-14T08:00:00Z');
});

test('search with characters the server rejects is never sent', () => {
  for (const bad of ["<script>alert(1)</script>", "x' OR 1=1 --", '%', 'a;b']) {
    assert.equal(new URLSearchParams(activityQuery({...base, search: bad})).has('search'), false, bad);
  }
});

test('codes are humanized for display', () => {
  assert.equal(humanize('ADMIN_ARTICLE_PUBLISHED'), 'Admin article published');
  assert.equal(humanize('SUCCESS'), 'Success');
});
