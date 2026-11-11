// Stage-specific classification of a failed call attempt. Pure and
// dependency-light so it is directly unit-testable; the Agora session feeds it
// whatever was thrown at a known stage. A backend error is recognised by shape
// (`status` + stable `code`), never by its human text.
//
// Diagnostics are sanitized by construction: only the stage, an HTTP status and
// short error code/name are ever emitted — never tokens, channel names, App IDs,
// request bodies or any clinical data.
import { classifyAgoraErrorCode } from './agora-errors.ts';
import { describeJoinFailure } from './join-window.ts';

export type CallStage = 'token' | 'sdk_load' | 'client_join' | 'media_devices' | 'publish';

interface ApiLike { status: number; code: string; message: string }
const isApiLike = (e: unknown): e is ApiLike =>
  typeof e === 'object' && e !== null && typeof (e as ApiLike).status === 'number' && typeof (e as ApiLike).code === 'string';
const short = (v: unknown) => String(v).replace(/[^A-Za-z0-9_.:-]/g, '').slice(0, 48);

export function callFailureDiagnostic(stage: CallStage, error: unknown): string {
  const parts = [`stage=${stage}`];
  if (isApiLike(error)) parts.push(`http=${error.status}`, `code=${short(error.code)}`);
  else if (typeof error === 'object' && error !== null) {
    const e = error as { code?: unknown; name?: unknown };
    if (e.code !== undefined) parts.push(`code=${short(e.code)}`);
    if (e.name !== undefined) parts.push(`name=${short(e.name)}`);
  } else parts.push('type=' + typeof error);
  return parts.join(' ');
}

export function describeCallFailure(stage: CallStage, error: unknown, opensAt?: string, nowMs: number = Date.now()): string {
  if (isApiLike(error)) {
    return describeJoinFailure(error, opensAt, nowMs) ?? error.message ?? 'The consultation service could not authorize this call. Please retry.';
  }
  const code = typeof error === 'object' && error !== null && 'code' in error ? String((error as { code: unknown }).code) : '';
  if (code === 'JOIN_TIMEOUT') return 'Could not reach the video service. Check that this browser and network allow connections to agora.io and sd-rtn.com, then retry.';
  const classified = classifyAgoraErrorCode(code);
  if (classified) return classified;
  switch (stage) {
    case 'token': return 'Network unavailable. Check your connection and retry.';
    case 'sdk_load': return 'The video component could not be loaded. Refresh the page and retry.';
    case 'media_devices': return 'Camera or microphone could not be started. Check permissions and that no other app is using them, then retry.';
    case 'publish': return 'Connected, but your audio/video could not be sent. Retry joining.';
    default: return 'Unable to connect to the video service. Retry, or refresh the page if it keeps failing.';
  }
}
