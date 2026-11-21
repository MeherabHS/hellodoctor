/**
 * Admin step-up (2026-11-21). Sensitive actions answer 403 STEP_UP_REQUIRED
 * until the Admin re-enters their password on this session; one confirmation
 * covers 5 minutes. `request()` calls `ensureStepUp()`, which shows the
 * password dialog (StepUpDialog), posts it to `/admin/step-up` and resolves
 * true once confirmed, so the original action can be retried. Simultaneous
 * requests share one prompt.
 */

export type StepUpPrompt = { error: string | null; busy: boolean };
type Listener = (prompt: StepUpPrompt | null) => void;
/** Posts the password; resolves on success, rejects with a shown message. */
export type StepUpSubmit = (password: string) => Promise<void>;

let current: StepUpPrompt | null = null;
let listeners: Listener[] = [];
let answer: ((password: string | null) => void) | null = null;
let pending: Promise<boolean> | null = null;

function publish(next: StepUpPrompt | null) {
  current = next;
  for (const listener of listeners) listener(current);
}

/** The dialog subscribes here; returns an unsubscribe function. */
export function subscribeStepUp(listener: Listener): () => void {
  listeners.push(listener);
  listener(current);
  return () => {
    listeners = listeners.filter((l) => l !== listener);
  };
}

/** Called by the dialog: the typed password, or null when cancelled. */
export function answerStepUp(password: string | null) {
  const resolve = answer;
  answer = null;
  resolve?.(password);
}

function ask(error: string | null): Promise<string | null> {
  publish({ error, busy: false });
  return new Promise((resolve) => {
    answer = resolve;
  });
}

/**
 * Prompts until the password is accepted (true) or the Admin cancels or a
 * non-retryable error occurs (false). Without a dialog on screen (tests,
 * server) it resolves false at once.
 */
export function ensureStepUp(submit: StepUpSubmit): Promise<boolean> {
  if (typeof window === 'undefined' || listeners.length === 0) return Promise.resolve(false);
  if (pending) return pending;
  pending = (async () => {
    let error: string | null = null;
    try {
      for (;;) {
        const password = await ask(error);
        if (password === null) return false;
        publish({ error: null, busy: true });
        try {
          await submit(password);
          return true;
        } catch (e) {
          error = e instanceof Error ? e.message : String(e);
        }
      }
    } finally {
      publish(null);
      pending = null;
    }
  })();
  return pending;
}
