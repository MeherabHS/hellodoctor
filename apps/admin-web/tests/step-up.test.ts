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
