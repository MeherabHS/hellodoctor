import type { Appointment } from './contracts';
import type { CallPhase } from './agora-client';

/** Phase 6's required human-readable states, derived from the real Agora
 * connection phase for an eligible appointment, or from the appointment's
 * own status (when there is nothing to connect to yet). `eligible` is the
 * caller's own `consultationAction(a) === 'call'` result — this module takes
 * it as a plain boolean rather than importing `consultationAction` itself,
 * so it stays a dependency-free, directly unit-testable leaf module (the
 * same shape as `call-telemetry.ts`). */
export function callStatusText(a: Appointment | null, eligible: boolean, phase: CallPhase | 'idle', remotePresent: boolean): string {
  if (!a) return 'No consultation selected';
  if (!eligible) {
    if (a.status === 'COMPLETED') return 'Consultation completed';
    if (['CANCELLED', 'EXPIRED', 'PREMATURE_TERMINATION', 'DISPUTED', 'REFUNDED'].includes(a.status)) return 'Call ended';
    if (a.status === 'PENDING_PAYMENT') return 'Waiting for payment confirmation';
    return 'Upcoming appointment';
  }
  switch (phase) {
    case 'idle': return 'Ready to join';
    case 'connecting': return 'Connecting…';
    case 'waiting-for-remote': return 'Waiting for Patient…';
    case 'connected': return remotePresent ? 'Connected' : 'Waiting for Patient…';
    case 'reconnecting': return 'Reconnecting…';
    case 'failed': return 'Connection failed';
    case 'ended': return 'Call ended';
    default: return 'Ready to join';
  }
}

export function callStatusTone(phase: CallPhase | 'idle', eligible: boolean): 'green' | 'amber' | 'red' | 'slate' {
  if (!eligible) return 'slate';
  if (phase === 'connected') return 'green';
  if (phase === 'connecting' || phase === 'waiting-for-remote' || phase === 'reconnecting') return 'amber';
  if (phase === 'failed') return 'red';
  return 'slate';
}
