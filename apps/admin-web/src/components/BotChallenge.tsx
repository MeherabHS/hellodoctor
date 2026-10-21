'use client';
import Script from 'next/script';
import { useEffect, useRef, useState } from 'react';

declare global {
  interface Window {
    turnstile?: {
      render(container: HTMLElement, options: Record<string, unknown>): string;
      remove(id: string): void;
    };
  }
}

// null means pending/failed; an empty string is allowed only by the server's
// explicit development/test policy. Widget state is never authentication authority.
export function BotChallenge({ action, reset, onToken }: {
  action: string; reset: number; onToken: (token: string | null) => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const [config, setConfig] = useState<{ required: boolean; siteKey: string } | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    const timeout = setTimeout(() => { setFailed(true); controller.abort(); }, 5000);
    fetch('/api/bot-challenge', { cache: 'no-store', signal: controller.signal })
      .then(response => { if (!response.ok) throw new Error(); return response.json(); })
      .then(value => {
        if (typeof value.required !== 'boolean' || typeof value.siteKey !== 'string') throw new Error();
        setConfig(value);
      }).catch(() => { if (!controller.signal.aborted) setFailed(true); })
      .finally(() => clearTimeout(timeout));
    return () => { clearTimeout(timeout); controller.abort(); };
  }, []);
  useEffect(() => {
    onToken(null);
    if (!config) return;
    if (!config.required) { onToken(''); return; }
    if (!config.siteKey) return;
    const api = window.turnstile;
    if (!loaded || !api || !container.current) return;
    let active = true;
    let widget: string;
    try { widget = api.render(container.current, { sitekey: config.siteKey, action,
      'response-field': false,
      callback: (token: string) => { if (active) { setFailed(false); onToken(token); } },
      'expired-callback': () => { if (active) { onToken(null); setFailed(true); } },
      'error-callback': () => { if (active) { onToken(null); setFailed(true); } },
    }); } catch {
      // Report the external widget failure after effect setup, while honoring
      // teardown if this challenge was replaced or the component unmounted.
      queueMicrotask(() => { if (active) setFailed(true); });
      return () => { active = false; };
    }
    return () => { active = false; api.remove(widget); };
  }, [config, loaded, action, reset, onToken]);
  return <div>
    {config?.required && config.siteKey && <Script id="turnstile" src="https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit"
      onReady={() => setLoaded(true)} onError={() => { onToken(null); setFailed(true); }} />}
    <div ref={container} />
    {(failed || (config?.required && !config.siteKey)) && <p role="status">We couldn&apos;t complete verification. <button type="button" onClick={() => window.location.reload()}>Try again</button></p>}
  </div>;
}
