import '../../../core/utils/time_format.dart';

/// Client view of the server's authoritative consultation join policy
/// (`services/backend/src/domain/join_window.rs`, exposed as the `join`
/// object on appointment lists and `GET /consultations/:id/join-eligibility`).
///
/// This carries NO window rules of its own. It only re-evaluates the
/// server-supplied absolute instants against a clock corrected with the
/// server's own `server_time`, so a countdown can reach zero without a
/// refetch and a wrong device clock/timezone is harmless. The backend still
/// revalidates at the real join boundary (the RTC-token mint): this is
/// presentation, never authorization.
enum JoinGateState { open, locked, closed, unavailable }

class JoinEligibility {
  const JoinEligibility({
    required this.eligible,
    required this.reason,
    required this.joinOpenAt,
    required this.joinCloseAt,
    required this.serverTime,
    required this.clockOffset,
  });

  final bool eligible;

  /// Stable machine code: OK | NOT_YET_OPEN | WINDOW_CLOSED | INVALID_STATE |
  /// NOT_VIDEO | NEEDS_REVIEW.
  final String reason;
  final DateTime joinOpenAt, joinCloseAt, serverTime;

  /// `serverNow = deviceNow + clockOffset`, captured when the response was
  /// received.
  final Duration clockOffset;

  static const _unavailable = {'INVALID_STATE', 'NOT_VIDEO', 'NEEDS_REVIEW'};

  /// Returns `null` when the payload has no usable `join` object (an older
  /// backend), in which case the caller must not invent a lock: the server
  /// still enforces the window when the token is requested.
  static JoinEligibility? tryParse(Object? raw, {DateTime? receivedAt}) {
    if (raw is! Map) return null;
    final open = DateTime.tryParse('${raw['join_open_at']}');
    final close = DateTime.tryParse('${raw['join_close_at']}');
    final server = DateTime.tryParse('${raw['server_time']}');
    if (open == null || close == null || server == null) return null;
    return JoinEligibility(
      eligible: raw['eligible'] == true,
      reason: '${raw['reason'] ?? 'OK'}',
      joinOpenAt: open.toUtc(),
      joinCloseAt: close.toUtc(),
      serverTime: server.toUtc(),
      clockOffset:
          server.toUtc().difference((receivedAt ?? DateTime.now()).toUtc()),
    );
  }

  DateTime _serverNow(DateTime? now) =>
      (now ?? DateTime.now()).toUtc().add(clockOffset);

  JoinGateState gate([DateTime? now]) {
    if (_unavailable.contains(reason)) return JoinGateState.unavailable;
    final serverNow = _serverNow(now);
    if (serverNow.isBefore(joinOpenAt)) return JoinGateState.locked;
    if (serverNow.isAfter(joinCloseAt)) return JoinGateState.closed;
    return JoinGateState.open;
  }

  bool isOpen([DateTime? now]) => gate(now) == JoinGateState.open;

  /// Time until the room opens (zero once open).
  Duration untilOpen([DateTime? now]) {
    final remaining = joinOpenAt.difference(_serverNow(now));
    return remaining.isNegative ? Duration.zero : remaining;
  }
}

/// Convenience for call sites holding a nullable eligibility: no information
/// means "do not lock" (the server decides at token time).
bool joinIsOpen(JoinEligibility? join, [DateTime? now]) =>
    join == null || join.isOpen(now);

String formatCountdown(Duration d) {
  if (d <= Duration.zero) return 'now';
  final total = d.inSeconds + (d.inMilliseconds % 1000 == 0 ? 0 : 1);
  final h = total ~/ 3600;
  final m = (total % 3600) ~/ 60;
  final s = total % 60;
  if (h > 0) return '${h}h ${m.toString().padLeft(2, '0')}m';
  return '${m}m ${s.toString().padLeft(2, '0')}s';
}

/// Authoritative, user-facing wording for a join denial reason.
/// `opensAt`/`closesAt` are rendered in EAT, with the date when not today.
String joinDenialText(String reason, {DateTime? opensAt, DateTime? now}) {
  switch (reason) {
    case 'NOT_YET_OPEN':
      return opensAt == null
          ? 'Your consultation room is not open yet.'
          : 'Your consultation room opens at ${formatEatDateTime(opensAt, now: now)}.';
    case 'WINDOW_CLOSED':
      return 'The joining window for this consultation has closed.';
    case 'INVALID_STATE':
      return 'This appointment can no longer be joined because it is not in an active state.';
    case 'NOT_VIDEO':
      return 'Video access is only available for video consultations.';
    case 'NEEDS_REVIEW':
      return 'This consultation needs review before it can be rejoined.';
    default:
      return 'This consultation cannot be joined right now.';
  }
}
