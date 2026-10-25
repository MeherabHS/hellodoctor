// Narrow, allowlist-adjacent rejections for request shapes that have no
// legitimate use in this app and exist only as evasion value against the
// real auth/CSRF/exposure boundaries already enforced elsewhere
// (`lib/exposure.ts`'s `exposureHeaders`, `lib/server.ts`'s `sameOrigin()`).
// Every check here is a pure function of already-available request data —
// no state, no I/O, no secrets — so `src/middleware.ts` can run them
// unconditionally and early, before any routing or auth decision.
//
// Two named layers, matching how a reverse proxy / WAF conventionally
// labels these: "SideDoor-Guard" (header-shape rejections, 400 — the
// request itself is malformed/suspicious regardless of what it targets)
// and "SideDoor-PerimeterDefense" (target-shape rejections, 403 — the
// request is well-formed but aimed at something this app will never
// legitimately serve).

/** Verb-tunneling headers: let a client disguise one HTTP method as another
 * to slip a destructive call past a method-based firewall/WAF rule. This
 * app's own routes never read these headers, so their mere presence has no
 * legitimate explanation. */
const METHOD_OVERRIDE_HEADERS = [
  'x-http-method-override',
  'x-http-method',
  'x-method-override',
  'x-http-destination',
];

/** Reverse-proxy routing-confusion headers: a front-end proxy and this app
 * disagreeing about the "real" request path/prefix is exactly the shape of
 * bug that lets a request reach an internal route the proxy meant to keep
 * private. This app's own proxying (`/api/proxy/[...path]`) never reads
 * these from the inbound client request either. */
const PROXY_REWRITE_HEADERS = [
  'x-original-url',
  'x-rewrite-url',
  'x-forwarded-prefix',
  'x-forwarded-path',
  'x-custom-url-rewrite',
];

/** SideDoor-Guard, HTTP header layer — 400 (the request shape itself is
 * malformed, independent of what path it targets). */
export function hasTunnelingHeader(headers: Headers): boolean {
  return [...METHOD_OVERRIDE_HEADERS, ...PROXY_REWRITE_HEADERS].some((name) => headers.has(name));
}

/** Cloud instance-metadata endpoints (AWS/GCP/Azure/Alibaba) and internal
 * cluster/runtime diagnostic endpoints — never legitimately requested by a
 * browser client of this app; their presence in an inbound request path
 * means something is probing for SSRF pivot points or a debug side door. */
const METADATA_AND_DEBUG_PATH_PATTERNS: RegExp[] = [
  /^\/latest\/meta-data\//i,
  /^\/computemetadata\//i,
  /169\.254\.169\.254/,
  /100\.100\.100\.200/,
  /^\/api\/v1\/namespaces\//i,
  /^\/actuator(\/|$)/i,
  /^\/debug\/pprof(\/|$)/i,
];

/** Leftover/backup-file harvesting probes — this app never serves these
 * extensions from any route, so a request path ending in one is never a
 * real user action. */
const BACKUP_FILE_PATH_PATTERN =
  /\.(bak|backup|old|orig|save|swp|sql|dump|tar|gz|zip)$|\.env(\.|$)/i;

/** SideDoor-PerimeterDefense, URL-shape layer — 403 (the request targets
 * something this app will never legitimately serve). Checked against the
 * raw, still-percent-encoded pathname (never a decoded/normalized form —
 * decoding first is exactly how a double-encoded traversal sequence slips
 * past a naively-written filter). */
export function isPerimeterBlockedPath(pathname: string): boolean {
  if (pathname.includes(';')) return true; // semicolon path-parameter bypass (e.g. /api;bypass/health)
  if (pathname.includes('\\') || pathname.includes('\0')) return true;
  if (/%2e%2e|%252e/i.test(pathname)) return true; // (double-)encoded traversal
  if (METADATA_AND_DEBUG_PATH_PATTERNS.some((pattern) => pattern.test(pathname))) return true;
  if (BACKUP_FILE_PATH_PATTERN.test(pathname)) return true;
  return false;
}

/** The exact three keys a prototype-pollution payload needs to reach
 * `Object.prototype` (or a constructor it can abuse) once an attacker-
 * controlled object is later merged/assigned into application state.
 * `JSON.parse` creates these as ordinary own, enumerable data properties
 * when the source text names them as object keys — unlike a normal
 * assignment through `[[Set]]`, it does not trigger the `__proto__`
 * accessor — so a plain recursive `Object.keys` walk over the *parsed*
 * value reliably finds them regardless of how deeply nested the payload is. */
const POLLUTION_KEYS = new Set(['__proto__', 'constructor', 'prototype']);

/** SideDoor-Guard, JSON payload layer — 403, applied before the payload
 * ever reaches this app's own Zod validation or is forwarded to the
 * backend. Deliberately inspects the *parsed* structure (recursing into
 * nested objects/arrays) rather than scanning the raw request text for the
 * bare substrings "__proto__"/"constructor"/"prototype": those words can
 * appear legitimately inside a free-text field a user typed (a grievance
 * description, a chat message) without ever becoming an *object key*, and a
 * raw-text scan would reject that real content as a false positive. Only an
 * actual own key named exactly one of these three is pollution-shaped. */
export function hasPrototypePollution(value: unknown, depth = 0): boolean {
  if (depth > 32 || value === null || typeof value !== 'object') return false;
  if (Array.isArray(value)) {
    return value.some((item) => hasPrototypePollution(item, depth + 1));
  }
  for (const key of Object.keys(value as Record<string, unknown>)) {
    if (POLLUTION_KEYS.has(key)) return true;
    if (hasPrototypePollution((value as Record<string, unknown>)[key], depth + 1)) return true;
  }
  return false;
}

/** True only for a request shape where body inspection is both possible and
 * worth the cost: a state-modifying verb carrying a JSON body. A GET/HEAD
 * request has no body to pollute, and a non-JSON content-type (file upload
 * multipart, etc.) was never going to be merged into a plain object via
 * `JSON.parse` in the first place. */
export function isJsonStateChangingRequest(method: string, contentType: string | null): boolean {
  if (!['POST', 'PUT', 'PATCH', 'DELETE'].includes(method.toUpperCase())) return false;
  return (contentType ?? '').toLowerCase().includes('application/json');
}
