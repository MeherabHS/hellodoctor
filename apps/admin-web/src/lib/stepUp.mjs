// Password step-up for sensitive Admin actions (backend:
// services/backend/src/services/admin_step_up.rs). Approving or rejecting a
// doctor or peer, money movements, account erasure, identity traces and
// revoking Admin invitations answer 403 `REAUTH_REQUIRED` until the Admin
// confirms their password; one confirmation unlocks this sign-in session for
// ten minutes.
//
// `request()` in api.ts detects that answer, asks the mounted
// <StepUpDialog/> for the password and retries the action once.
// Plain .mjs (like adminQueue.mjs) so Node's test runner can load it directly.

/** @typedef {() => Promise<boolean>} StepUpPrompt */

/** @type {StepUpPrompt | null} */
let prompt = null;
/** @type {Promise<boolean> | null} */
let pending = null;

/**
 * Registered by <StepUpDialog/>; returns an unregister function.
 * @param {StepUpPrompt} fn
 * @returns {() => void}
 */
export function registerStepUpPrompt(fn) {
  prompt = fn;
  return () => { if (prompt === fn) prompt = null; };
}

/**
 * Asks for the password. Concurrent callers share one dialog. Resolves true
 * once the backend accepted the password, false if the Admin cancelled or no
 * dialog is mounted.
 * @returns {Promise<boolean>}
 */
export function confirmStepUp() {
  if (!prompt) return Promise.resolve(false);
  if (!pending) pending = prompt().finally(() => { pending = null; });
  return pending;
}

/**
 * True for the backend's `REAUTH_REQUIRED` answer.
 * @param {number} status
 * @param {string} body
 * @returns {boolean}
 */
export function isReauthRequired(status, body) {
  if (status !== 403) return false;
  try {
    const parsed = JSON.parse(body);
    return parsed?.error?.code === 'REAUTH_REQUIRED';
  } catch {
    return false;
  }
}
