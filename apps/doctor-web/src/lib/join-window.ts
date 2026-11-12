// Client view of the server's authoritative consultation join policy
// (`services/backend/src/domain/join_window.rs`). This module carries NO
// window rules of its own: it only re-evaluates the server-supplied absolute
// instants (`join_open_at` / `join_close_at`) against a clock corrected with
// the server's own `server_time`, so a countdown can reach zero without a
// refetch. The backend still revalidates at the real join boundary (the
// RTC-token mint) — this is presentation, never authorization.
//
// Dependency-free apart from `eat.ts`, so it is directly unit-testable.
import { eatTime } from './eat.ts';

export interface JoinInfo {
  eligible: boolean;
  /** Stable machine code: OK | NOT_YET_OPEN | WINDOW_CLOSED | INVALID_STATE | NOT_VIDEO | NEEDS_REVIEW */
  reason: string;
  join_open_at: string;
  join_close_at: string;
  server_time: string;
}

export type JoinGateState = 'open' | 'locked' | 'closed' | 'unavailable';
export interface JoinGate {
  state: JoinGateState;
  reason: string;
  opensAt?: string;
  closesAt?: string;
  msUntilOpen?: number;
}

const UNAVAILABLE_REASONS = ['INVALID_STATE', 'NOT_VIDEO', 'NEEDS_REVIEW'];

// serverNow = deviceNow + offset. Captured the first time a given server_time
// is seen (i.e. right after the response arrives), so a device clock that is
// hours fast/slow, or on any timezone, still evaluates the window correctly.
const offsets = new Map<string, number>();
export function serverOffsetMs(join: JoinInfo, nowMs: number): number {
  const cached = offsets.get(join.server_time);
  if (cached !== undefined) return cached;
  if (offsets.size > 200) offsets.clear();
  const value = Date.parse(join.server_time) - nowMs;
  offsets.set(join.server_time, Number.isFinite(value) ? value : 0);
  return offsets.get(join.server_time)!;
}

export function joinGate(join: JoinInfo | null | undefined, nowMs: number = Date.now()): JoinGate {
  // A backend that predates the field: the server still enforces at token time.
  if (!join) return { state: 'open', reason: 'UNKNOWN' };
  if (UNAVAILABLE_REASONS.includes(join.reason)) return { state: 'unavailable', reason: join.reason };
  const serverNow = nowMs + serverOffsetMs(join, nowMs);
  const open = Date.parse(join.join_open_at);
  const close = Date.parse(join.join_close_at);
  const base = { opensAt: join.join_open_at, closesAt: join.join_close_at };
  if (serverNow < open) return { ...base, state: 'locked', reason: 'NOT_YET_OPEN', msUntilOpen: open - serverNow };
  if (serverNow > close) return { ...base, state: 'closed', reason: 'WINDOW_CLOSED' };
  return { ...base, state: 'open', reason: 'OK' };
}

const eatDay = (iso: string) => new Intl.DateTimeFormat('en-CA', { timeZone: 'Africa/Nairobi', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date(iso));

/** "11:24 PM EAT", prefixed with the EAT date when it is not today (EAT). */
export function eatDateTime(iso: string, nowMs: number = Date.now()): string {
  const sameDay = eatDay(iso) === eatDay(new Date(nowMs).toISOString());
  if (sameDay) return eatTime(iso);
  const day = new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', timeZone: 'Africa/Nairobi' });
  return `${day}, ${eatTime(iso)}`;
}

export function formatCountdown(ms: number): string {
  if (ms <= 0) return 'now';
  const total = Math.ceil(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${String(m).padStart(2, '0')}m`;
  return `${m}m ${String(s).padStart(2, '0')}s`;
}

/** Locked-state copy. Returns `null` when the room is open. */
export function lockedMessage(gate: JoinGate, nowMs: number = Date.now()): string | null {
  switch (gate.state) {
    case 'locked': return `Room opens at ${eatDateTime(gate.opensAt!, nowMs)}`;
    case 'closed': return `Joining window closed at ${eatDateTime(gate.closesAt!, nowMs)}`;
    case 'unavailable':
      if (gate.reason === 'NEEDS_REVIEW') return 'This consultation needs review before it can be rejoined';
      if (gate.reason === 'NOT_VIDEO') return 'Video access is only available for video consultations';
      return 'This appointment can no longer be joined';
    default: return null;
  }
}

/** Stage-aware, actionable text for a failed join. `error` is the plain
 * `{status, code}` of a backend ApiError (or any thrown value). The stable
 * backend `code` decides the message — never the human text. */
export function describeJoinFailure(error: { status?: number; code?: string } | null | undefined, opensAt?: string, nowMs: number = Date.now()): string | null {
  const code = error?.code;
  const status = error?.status;
  if (code === 'NOT_YET_OPEN') return opensAt ? `The consultation room is not open yet. It opens at ${eatDateTime(opensAt, nowMs)}.` : 'The consultation room is not open yet.';
  if (code === 'WINDOW_CLOSED') return 'The joining window for this consultation has closed.';
  if (code === 'INVALID_STATE') return 'This appointment can no longer be joined because it is not in an active state.';
  if (code === 'NOT_VIDEO') return 'Video access is only available for video consultations.';
  if (code === 'NEEDS_REVIEW') return 'This consultation needs review before it can be rejoined.';
  if (code === 'RESOURCE_CONFLICT' && status === 409) return 'You are already live in another consultation.';
  if (status === 401) return 'Your session has expired. Sign in again to join.';
  if (status === 403 || code === 'FORBIDDEN') return 'You are not authorized to join this consultation.';
  if (status === 404) return 'This consultation could not be found.';
  return null;
}
