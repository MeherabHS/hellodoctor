import test from 'node:test';
import assert from 'node:assert/strict';
import { encodeWithinRxBudget, RX_PAGE_MAX_BYTES } from '../src/lib/rx-compression.ts';

test('40 KB is a hard byte boundary, not a rounded size or best-effort quality floor', async () => {
  const attempts: number[] = [];
  const blob = await encodeWithinRxBudget(3000, 1500, 'image/jpeg', async (width, height, quality) => {
    assert.equal(width, 2000); assert.equal(height, 1000); attempts.push(quality);
    return new Blob([new Uint8Array(RX_PAGE_MAX_BYTES + (quality === 0.8 ? 1 : 0))], { type: 'image/jpeg' });
  });
  assert.deepEqual(attempts, [0.8, 0.65]); assert.equal(blob.size, RX_PAGE_MAX_BYTES);
});

test('quality exhaustion causes resizing, retaining aspect ratio and retrying quality before returning', async () => {
  const attempts: [number, number, number][] = [];
  const blob = await encodeWithinRxBudget(2000, 1000, 'image/jpeg', async (width, height, quality) => {
    attempts.push([width, height, quality]);
    return new Blob([new Uint8Array(width === 2000 ? RX_PAGE_MAX_BYTES + 1 : RX_PAGE_MAX_BYTES - 1)], { type: 'image/jpeg' });
  });
  assert.equal(attempts[6][2], 0.01); assert.deepEqual(attempts[7], [1000, 500, 0.8]); assert.ok(blob.size <= RX_PAGE_MAX_BYTES);
});

test('PNG remains PNG and is downscaled rather than using a nonexistent quality control', async () => {
  const attempts: number[] = [];
  await encodeWithinRxBudget(1800, 900, 'image/png', async (width, height, quality) => {
    attempts.push(width); assert.equal(height, width / 2); assert.equal(quality, 1);
    return new Blob([new Uint8Array(width > 900 ? RX_PAGE_MAX_BYTES + 1 : 5000)], { type: 'image/png' });
  });
  assert.deepEqual(attempts, [1800, 900]);
});

test('unusable encoders and oversized decoded images cannot bypass the hard cap', async () => {
  await assert.rejects(encodeWithinRxBudget(8, 8, 'image/jpeg', async () => new Blob([new Uint8Array(RX_PAGE_MAX_BYTES + 1)], { type: 'image/jpeg' })), /40 KB/);
  await assert.rejects(encodeWithinRxBudget(100, 100, 'image/jpeg', async () => new Blob(['invalid'], { type: 'image/png' })), /Unable to compress/);
  await assert.rejects(encodeWithinRxBudget(8193, 100, 'image/jpeg', async () => { throw new Error('must not encode'); }), /too large/);
});

test('session or consultation switch while encoding prevents publishing a photo', async () => {
  let current = true;
  await assert.rejects(encodeWithinRxBudget(100, 100, 'image/jpeg', async () => {
    current = false; return new Blob(['image'], { type: 'image/jpeg' });
  }, () => current), { name: 'AbortError' });
});
