import type { BookingPolicy } from './api';

/** Derives the Admin UI's financial-visibility gate from `GET /config/booking-policy`.
 * Defaults to `true` (the safe default — never hide real financial actions) until
 * the server has explicitly reported the module as deferred. Mirrors Doctor Web's
 * `paymentRequiredFromPolicy` in `apps/doctor-web/src/lib/contracts.ts` exactly, so
 * all three clients apply the identical fail-safe rule. */
export function paymentRequiredFromPolicy(data: Partial<BookingPolicy> | null | undefined): boolean {
  return data?.payment_required !== false;
}
