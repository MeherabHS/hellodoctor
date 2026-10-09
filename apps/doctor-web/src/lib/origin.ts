// A local preview has one canonical origin for cookies and CSRF checks.
// Redirect only the equivalent loopback hostname, never an arbitrary Host.
export function canonicalLoopbackRedirect(requestUrl: string, configured?: string, requestHost?: string | null): string | null {
  if (!configured) return null;
  try {
    const current = new URL(requestUrl), expected = new URL(configured);
    // Next may represent req.url using its internal localhost origin. Use the
    // actual HTTP Host to avoid redirecting an already-canonical request.
    if (requestHost) current.host = requestHost;
    const loopback = new Set(['localhost', '127.0.0.1']);
    if (!loopback.has(current.hostname) || !loopback.has(expected.hostname)) return null;
    if (current.hostname === expected.hostname || current.port !== expected.port || current.protocol !== expected.protocol) return null;
    if (expected.username || expected.password || expected.pathname !== '/' || expected.search || expected.hash) return null;
    return new URL(current.pathname + current.search, expected.origin).href;
  } catch { return null; }
}
