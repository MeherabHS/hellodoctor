/**
 * Builds the backend path for the same-origin proxy. Each segment must be a
 * plain URL-safe token: no "." or ".." (path traversal out of /api/v1), no
 * encoded slashes, backslashes, "?", "#", "%" or control characters. Returns
 * null for anything else so the proxy refuses it before contacting the backend.
 */
const SEGMENT = /^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$/;

export function backendPath(segments: readonly string[]): string | null {
  if (segments.length === 0 || segments.length > 12) return null;
  for (const s of segments) {
    if (!SEGMENT.test(s) || s.includes('..')) return null;
  }
  return segments.map(encodeURIComponent).join('/');
}
