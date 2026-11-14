import test from 'node:test';
import assert from 'node:assert/strict';
import {backendPath} from '../src/lib/proxyPath.ts';

test('normal admin paths pass through', () => {
  assert.equal(backendPath(['admin', 'team', 'invitations', '3f2b8c1e-0d1a-4c7e-9a55-0f6b1c2d3e4f', 'resend']), 'admin/team/invitations/3f2b8c1e-0d1a-4c7e-9a55-0f6b1c2d3e4f/resend');
  assert.equal(backendPath(['admin', 'articles', 'v2.json']), 'admin/articles/v2.json');
});

test('traversal and smuggling segments are refused', () => {
  for (const bad of [['..'], ['.'], ['admin', '..', 'internal'], ['admin', '..%2f'], ['a/b'], ['a\b'], ['a?x=1'], ['a#b'], ['%2e%2e'], ['a b'], ['a\u0000'], [], ['x'.repeat(200)], ['admin', 'x..y']]) {
    assert.equal(backendPath(bad), null, JSON.stringify(bad));
  }
});
