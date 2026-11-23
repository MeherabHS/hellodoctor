import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act, create, type ReactTestRenderer} from 'react-test-renderer';
import {BackupStatusCard, size, type BackupStatus} from '../src/components/operations/BackupStatusCard';
import {clearSoftReads} from '../src/lib/api';

const ok = (data: unknown) => new Response(JSON.stringify({success: true, data}));
const text = (v: ReactTestRenderer) => JSON.stringify(v.toJSON());

async function render(status: BackupStatus): Promise<string> {
  clearSoftReads();
  const previous = globalThis.fetch;
  const urls: string[] = [];
  globalThis.fetch = async url => { urls.push(String(url)); return ok(status); };
  let view: ReactTestRenderer | undefined;
  try {
    await act(async () => { view = create(<BackupStatusCard/>); });
    assert.ok(urls.some(u => u.includes('/admin/system/backup-status')));
    return text(view!);
  } finally { globalThis.fetch = previous; view?.unmount(); }
}

const base: BackupStatus = {
  state: 'HEALTHY', checked_at: '2026-11-24T06:00:00Z', overdue_after_hours: 26,
  last_run: {at: '2026-11-24T00:00:05Z', result: 'ok'},
  last_success: {at: '2026-11-24T00:00:05Z', file: 'hellodoctor-2026-11-24_0300.dump', size_bytes: 743138, dumps_kept: 3, total_bytes: 2229414, keep: 14},
  last_restore_test: null,
};

test('a healthy backup shows its size, copies kept and that the test restore is pending', async () => {
  const t = await render(base);
  assert.ok(t.includes('Healthy'));
  assert.ok(t.includes('726 KB'));
  assert.ok(t.includes('3 of 14'));
  assert.ok(t.includes('Not yet (runs on Sundays)'));
});

test('a failed run shows the reason so it can be fixed before the next night', async () => {
  const t = await render({...base, state: 'FAILED', last_run: {at: '2026-11-24T00:00:05Z', result: 'failed', message: 'only 900 MB free, need at least 2048 MB'}});
  assert.ok(t.includes('Failed'));
  assert.ok(t.includes('only 900 MB free, need at least 2048 MB'));
});

test('not set up hides the details instead of showing empty values', async () => {
  const t = await render({...base, state: 'NOT_CONFIGURED', last_run: null, last_success: null});
  assert.ok(t.includes('Not set up'));
  assert.ok(!t.includes('Last successful backup'));
});

test('sizes read naturally', () => {
  assert.equal(size(0), '—');
  assert.equal(size(512), '512 B');
  assert.equal(size(743138), '726 KB');
  assert.equal(size(5 * 1024 * 1024 + 300000), '5.3 MB');
});
