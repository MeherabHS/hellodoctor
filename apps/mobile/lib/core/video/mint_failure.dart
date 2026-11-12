import '../network/api_client.dart';
import 'call_controller.dart';
import 'join_eligibility.dart';

/// Translates a refused `POST /consultations/:id/rtc-token` into a typed,
/// actionable failure using the backend's stable error `code` (never its human
/// text), distinguishing: too early, window closed, invalid appointment state,
/// session needing review, already live elsewhere, auth, offline and service
/// errors. Deterministic denials are not retryable; offline/5xx are.
///
/// `opensAt` is the server-supplied room-open instant (used only to word a
/// too-early denial in EAT). The Agora engine is never touched on any of these.
CallAuthorizationException mapMintFailure(ApiException e,
    {DateTime? opensAt, DateTime? now}) {
  switch (e.code) {
    case 'NOT_YET_OPEN':
      return CallAuthorizationException('NOT_YET_OPEN',
          joinDenialText('NOT_YET_OPEN', opensAt: opensAt, now: now));
    case 'WINDOW_CLOSED':
    case 'INVALID_STATE':
    case 'NOT_VIDEO':
    case 'NEEDS_REVIEW':
      return CallAuthorizationException(e.code!, joinDenialText(e.code!));
  }
  if (e.status == 409) {
    return const CallAuthorizationException(
        'ALREADY_LIVE', 'You are already live in another consultation.');
  }
  if (e.status == 401) {
    return const CallAuthorizationException(
        'UNAUTHENTICATED', 'Your session expired. Please sign in again.');
  }
  if (e.status == 403) {
    return const CallAuthorizationException(
        'FORBIDDEN', 'You are not authorized to join this consultation.');
  }
  if (e.status == null || e.status! >= 500) {
    return CallAuthorizationException(
        e.status == null ? 'NETWORK' : 'SERVICE',
        e.status == null
            ? 'Network unavailable. Check your connection and try again.'
            : 'The consultation service is temporarily unavailable. Please try again.',
        retryable: true);
  }
  return const CallAuthorizationException(
      'DENIED', 'This consultation could not be joined right now.');
}
