// Password step-up for sensitive Admin actions (backend:
// services/backend/src/services/admin_step_up.rs). Approving or rejecting a
// doctor or peer, money movements, account erasure, identity traces and
// revoking Admin invitations answer 403 `REAUTH_REQUIRED` until the Admin
// confirms their password; one confirmation unlocks this sign-in session for
// two minutes (product owner, 2026-11-21), shown as a countdown chip.
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

/** @type {number | null} */
let expiresAt = null;
/** @type {((expiresAt: number | null) => void)[]} */
let expiryListeners = [];

/**
 * Starts the countdown after a confirmation the backend accepted.
 * @param {number} seconds how long the confirmation lasts
 */
export function setStepUpExpiry(seconds) {
  expiresAt = seconds > 0 ? Date.now() + seconds * 1000 : null;
  for (const listener of expiryListeners) listener(expiresAt);
}

/** The backend asked again: whatever the countdown said, it has lapsed. */
export function clearStepUpExpiry() {
  if (expiresAt === null) return;
  expiresAt = null;
  for (const listener of expiryListeners) listener(expiresAt);
}

/**
 * The countdown chip subscribes here (epoch ms, or null when locked).
 * @param {(expiresAt: number | null) => void} listener
 * @returns {() => void}
 */
export function subscribeStepUpExpiry(listener) {
  expiryListeners.push(listener);
  listener(expiresAt);
  return () => { expiryListeners = expiryListeners.filter((l) => l !== listener); };
}
