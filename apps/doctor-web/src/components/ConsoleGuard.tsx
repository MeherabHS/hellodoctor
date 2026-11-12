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
//
// This component deliberately does NOT freeze `Object.prototype`. A frozen
// Object.prototype makes `toString`/`valueOf`/... read-only, and in strict
// mode any library doing `this.toString = ...` throws "Cannot assign to read
// only property 'toString'". The Agora Web SDK does exactly that while its
// module evaluates, which surfaced in production as the generic
// "stage=sdk_load name=TypeError" on Doctor Web. Prototype pollution is
// already rejected server-side (sideDoorGuard.ts), so the freeze bought
// little and broke video calls.
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
  }, []);
  return null;
}
