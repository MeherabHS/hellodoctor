import test from 'node:test';
import assert from 'node:assert/strict';
import { PREVIEW_BODY_LIMIT, PREVIEW_TITLE_LIMIT, truncateForPreview } from '../src/lib/notificationPreviewText.ts';

test('short text is shown verbatim and marked not truncated', () => {
  const result = truncateForPreview('Short title', 65);
  assert.equal(result.shown, 'Short title');
  assert.equal(result.truncated, false);
});

test('text at exactly the limit is not truncated', () => {
  const text = 'x'.repeat(10);
  assert.deepEqual(truncateForPreview(text, 10), { shown: text, truncated: false });
});

test('text over the limit is truncated with an ellipsis and marked truncated', () => {
  const text = 'x'.repeat(20);
  const result = truncateForPreview(text, 10);
  assert.equal(result.truncated, true);
  assert.equal(result.shown.length, 10);
  assert.ok(result.shown.endsWith('…'));
});

test('truncation trims trailing whitespace before the ellipsis so a cut mid-word is clean', () => {
  const text = 'word1 word2 word3';
  const result = truncateForPreview(text, 7); // cuts inside "word2" after a space boundary at 6
  assert.ok(!result.shown.slice(0, -1).endsWith(' '));
});

test('the default preview limits are realistic, conservative lock-screen lengths', () => {
  assert.ok(PREVIEW_TITLE_LIMIT > 0 && PREVIEW_TITLE_LIMIT < 100);
  assert.ok(PREVIEW_BODY_LIMIT > PREVIEW_TITLE_LIMIT);
});
