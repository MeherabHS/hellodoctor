/** Classifies an Agora Web SDK error code (`AgoraRTCError.code`) into an
 * actionable, patient-safe message. Returns `null` for a code this doesn't
 * recognize, so the caller can fall back to a backend/generic message
 * instead of a wrong one. Deliberately takes a plain string rather than
 * importing `agora-rtc-sdk-ng`'s `AgoraRTCErrorCode` enum — that import
 * would need to resolve the whole SDK, which only matters once an actual
 * call is starting; this stays a dependency-free, directly testable leaf
 * module (the same shape as `call-telemetry.ts`). */
export function classifyAgoraErrorCode(code: string): string | null {
  if (code === 'PERMISSION_DENIED') return 'Camera/microphone access was denied. Allow access in your browser settings and retry.';
  if (code === 'DEVICE_NOT_FOUND' || code === 'NOT_READABLE' || code === 'CONSTRAINT_NOT_SATISFIED') return 'No camera/microphone could be used — check the device is connected and not in use by another application.';
  if (code === 'CAN_NOT_GET_GATEWAY_SERVER' || code === 'UID_CONFLICT' || code === 'WS_ERR' || code === 'WS_ABORT' || code === 'WS_DISCONNECT') return 'Unable to join this consultation right now. Please retry.';
  if (code === 'NETWORK_ERROR' || code === 'NETWORK_TIMEOUT' || code === 'API_INVOKE_TIMEOUT') return 'Network unavailable. Check your connection and retry.';
  return null;
}
