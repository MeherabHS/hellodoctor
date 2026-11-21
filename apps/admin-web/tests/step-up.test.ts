import test from 'node:test';
import assert from 'node:assert/strict';
import {answerStepUp, ensureStepUp, subscribeStepUp, type StepUpPrompt} from '../src/lib/stepUp.ts';

test('without a dialog on screen step-up is declined at once', async () => {
  assert.equal(await ensureStepUp(async () => {}), false);
});

test('prompts, retries after a wrong password, shares one prompt and can be cancelled', async () => {
  (globalThis as {window?: unknown}).window = {};
  const prompts: (StepUpPrompt | null)[] = [];
  const unsubscribe = subscribeStepUp((p) => prompts.push(p));
  const tick = () => new Promise((r) => setTimeout(r, 0));
  try {
    const tried: string[] = [];
    const submit = async (password: string) => {
      tried.push(password);
      if (password !== 'right') throw new Error('Your current password is incorrect.');
    };
    // Two actions needing step-up at once: one prompt.
    const first = ensureStepUp(submit);
    const second = ensureStepUp(submit);
    await tick();
    answerStepUp('wrong');
    await tick();
    assert.equal(prompts.at(-1)?.error, 'Your current password is incorrect.');
    answerStepUp('right');
    assert.deepEqual(await Promise.all([first, second]), [true, true]);
    assert.deepEqual(tried, ['wrong', 'right']);
    assert.equal(prompts.at(-1), null, 'dialog closes');

    const cancelled = ensureStepUp(submit);
    await tick();
    answerStepUp(null);
    assert.equal(await cancelled, false);
  } finally {
    unsubscribe();
    delete (globalThis as {window?: unknown}).window;
  }
});

test('a confirmation starts the 2-minute countdown; a new server request clears it', async () => {
  const {markStepUpExpired, subscribeStepUpExpiry} = await import('../src/lib/stepUp.ts');
  (globalThis as {window?: unknown}).window = {};
  const unsubscribe = subscribeStepUp(() => {});
  let expiry: number | null = null;
  const stop = subscribeStepUpExpiry((value) => { expiry = value; });
  try {
    const before = Date.now();
    const done = ensureStepUp(async () => 120);
    await new Promise((r) => setTimeout(r, 0));
    answerStepUp('right');
    assert.equal(await done, true);
    assert.ok(expiry !== null && expiry >= before + 119_000 && expiry <= Date.now() + 120_000);
    markStepUpExpired();
    assert.equal(expiry, null);
  } finally {
    stop();
    unsubscribe();
    delete (globalThis as {window?: unknown}).window;
  }
});
