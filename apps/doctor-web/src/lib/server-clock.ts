// The server's clock (product owner, 2026-11-24: server time, Kenya,
// decides). Learned from the `Date` header of every live response, so a PC
// set to the wrong time still shows the right deadlines. Until the first
// response it is the device clock.
let offsetMs = 0;

/** The server's current time, in epoch milliseconds. */
export function serverNow(): number { return Date.now() + offsetMs; }

/** Records the server time from a response's `Date` header. */
export function observeServerDate(header: string | null | undefined, receivedAt = Date.now()): void {
  if (!header) return;
  const server = Date.parse(header);
  if (!Number.isFinite(server)) return;
  const offset = server - receivedAt;
  // The header has one-second resolution: ignore sub-second noise.
  offsetMs = Math.abs(offset) < 2000 ? 0 : offset;
}

/** Tests only. */
export function setServerClockOffset(ms: number): void { offsetMs = ms; }
