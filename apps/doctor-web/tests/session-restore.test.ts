import test from 'node:test';
import assert from 'node:assert/strict';
import { restoresSilently } from '../src/lib/sessionRestore.ts';

// This is the exact decision behind the Doctor Web refresh-loop fix: a tab
// regaining visibility or the network reconnecting must re-validate the
// session WITHOUT ever blocking the UI (which previously meant the whole
// app -- including an in-progress registration form -- got replaced by a
// bare spinner and remounted from scratch, over and over, on every single
// tab-focus-regain).

test('a tab regaining visibility re-validates silently -- never blocks the UI', () => {
  assert.equal(restoresSilently('visibility-regained'), true);
});

test('the network reconnecting re-validates silently -- never blocks the UI', () => {
  assert.equal(restoresSilently('reconnected'), true);
});

test('the initial mount is allowed to block -- there is no existing view to disrupt yet', () => {
  assert.equal(restoresSilently('mount'), false);
});

test('an explicit user-initiated retry is allowed to block -- the user asked for it', () => {
  assert.equal(restoresSilently('manual-retry'), false);
});
