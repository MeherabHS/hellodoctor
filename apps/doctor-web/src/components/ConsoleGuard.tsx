'use client';
import { useEffect } from 'react';

// Client-side anti-tamper, defense-in-depth only — the real boundary for
// both concerns below is already server-side: `middleware.ts`'s CSP sets
// `frame-ancestors 'none'`, which a compliant browser enforces before this
// component's JS ever runs (the frame simply never loads), and
// `sideDoorGuard.ts`'s prototype-pollution check rejects a polluted JSON
// body before it reaches this app's own state. This component exists only
// to also degrade gracefully in a browser that ignores CSP, or to block a
// pollution vector introduced by something *other* than a JSON request body
// (a crafted query string parsed client-side, a vulnerable dependency,
// etc.) — never a substitute for either server-side check.
export function ConsoleGuard() {
  useEffect(() => {
    try {
      if (window.top && window.top !== window.self) {
        window.top.location.href = window.self.location.href;
      }
    } catch {
      // `window.top` was cross-origin and inaccessible — this page really is
      // framed by something CSP should already have refused to load. Hide
      // the content rather than risk rendering inside an attacker's frame.
      document.body.style.display = 'none';
    }
    try {
      Object.freeze(Object.prototype);
    } catch {
      // Already frozen (e.g. a second mount in dev's StrictMode double-
      // invoke) or the runtime disallows it — nothing further to do either way.
    }
  }, []);
  return null;
}
