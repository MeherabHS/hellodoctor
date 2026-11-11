// The one decision behind the Doctor Web refresh-loop fix: which session
// re-validation triggers are allowed to block the UI with the full-tree
// loading replacement (DoctorApplication's `if (restoring) return <main>...
// <State loading/></main>`), and which must re-validate silently in the
// background instead.
//
// A tab regaining visibility or the network reconnecting happens constantly
// during ordinary use -- including mid-registration, before any session
// exists at all -- and must never force that full-tree replacement: doing
// so previously unmounted and remounted `<Auth/>` on every single
// tab-focus-regain, destroying typed fields, consent selections and the
// Turnstile widget each time, and (pre-login) firing a fresh
// `/doctor/profile` 401 every time too. Only the initial mount and an
// explicit user-initiated retry (the "Restore session" button) legitimately
// have no existing view to preserve, so only those may block.
export type RestoreTrigger = 'mount' | 'visibility-regained' | 'reconnected' | 'manual-retry';

export function restoresSilently(trigger: RestoreTrigger): boolean {
  return trigger === 'visibility-regained' || trigger === 'reconnected';
}
