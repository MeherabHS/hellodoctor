'use client';
// Wires the privacy-boundary incident shaper (client-incidents.ts) to a real
// delivery path, mirroring apps/doctor-web/src/lib/client.ts's `reportIncident`
// + `diagnostic` wiring (lines defining `reportIncident`/`diagnostic`).
//
// Doctor Web gates delivery on an in-memory `scope !== null` flag set during
// its own session-restore flow. Admin-web has no equivalent SPA-style restore
// step — protected pages only render once middleware has already seen the
// `hd_admin_session` marker cookie (see src/middleware.ts) — so that same
// non-HttpOnly marker is the direct equivalent "believe we're logged in"
// signal here, and we check for it the same way the middleware's own comment
// describes it: "gates only the shell", never a credential.
import { createIncidentReporter, IncidentContext } from './client-incidents';

function hasSessionMarker(): boolean {
  if (typeof document === 'undefined') return false;
  return document.cookie.split('; ').some((c) => c.startsWith('hd_admin_session='));
}

const reportIncident = createIncidentReporter(hasSessionMarker, async (body) => {
  const response = await fetch('/api/diagnostics', {
    method: 'POST', credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    signal: AbortSignal.timeout(3000), keepalive: true,
  });
  if (!response.ok) throw new Error('Incident delivery failed');
});

export function diagnostic(category: string, error?: unknown, context: IncidentContext = {}) {
  void reportIncident(error ?? new Error('Unexpected client exception'),
    { ...context, screen: context.screen ?? 'Application',
      operation: context.operation ?? (category === 'render-error' ? 'render' : category === 'browser-error' ? 'browser_error' : 'unhandled_async') });
}
