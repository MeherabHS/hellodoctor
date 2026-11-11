import 'package:flutter_bloc/flutter_bloc.dart';

import 'booking_repository.dart';
import 'join_eligibility.dart';

/// Appointment statuses a Waiting Room session can no longer act on — no
/// further polling or joining makes sense once here.
const kWaitingRoomTerminalStatuses = {
  'CANCELLED',
  'COMPLETED',
  'EXPIRED',
  'PREMATURE_TERMINATION',
  'REFUNDED',
  'DISPUTED',
};

class WaitingRoomState {
  const WaitingRoomState({
    required this.status,
    this.terminal = false,
    this.clinicalOutcome,
    this.completedAt,
    this.join,
  });
  final String status;
  final bool terminal;
  final String? clinicalOutcome, completedAt;

  /// Server-derived join eligibility for a video appointment; `null` when not
  /// yet known (older backend / not fetched), in which case nothing is locked
  /// client-side and the server decides at token time.
  final JoinEligibility? join;

  WaitingRoomState copyWith({
    String? status,
    bool? terminal,
    String? clinicalOutcome,
    String? completedAt,
    JoinEligibility? join,
  }) =>
      WaitingRoomState(
        status: status ?? this.status,
        terminal: terminal ?? this.terminal,
        clinicalOutcome: clinicalOutcome ?? this.clinicalOutcome,
        completedAt: completedAt ?? this.completedAt,
        join: join ?? this.join,
      );
}

/// Owns one Waiting Room session's polled status (M5). Constructed fresh per
/// push of [WaitingRoomPage] — this is a one-screen concern, not cross-tab
/// state. There is no patient-initiated cancellation: the product explicitly
/// does not allow cancelling a booked consultation, so this cubit is
/// read-only by design, not a scoped-down version of one that writes.
class WaitingRoomCubit extends Cubit<WaitingRoomState> {
  WaitingRoomCubit(
    this.repo, {
    required this.appointmentId,
    required String initialStatus,
    JoinEligibility? initialJoin,
    this.trackJoin = false,
  }) : super(WaitingRoomState(
          status: initialStatus,
          terminal: kWaitingRoomTerminalStatuses.contains(initialStatus),
          join: initialJoin,
        ));
  final BookingRepository repo;
  final String appointmentId;

  /// Video rooms only: also refresh the server's join eligibility on every poll
  /// (so a server-side state change, or the window opening, is picked up).
  final bool trackJoin;

  /// Best-effort background refresh — a transient failure must not surface
  /// as an error the patient didn't act on, and must not flip `terminal`
  /// based on stale/absent data.
  Future<void> poll() async {
    if (state.terminal) return;
    try {
      final detail = await repo.appointment(appointmentId);
      if (isClosed) return;
      JoinEligibility? join;
      final terminal = kWaitingRoomTerminalStatuses.contains(detail.status);
      if (trackJoin && !terminal) {
        try {
          join = await repo.joinEligibility(appointmentId);
        } catch (_) {
          // Keep the last known eligibility; the server still decides at token time.
        }
      }
      if (isClosed) return;
      emit(state.copyWith(
        status: detail.status,
        terminal: terminal,
        clinicalOutcome: detail.clinicalOutcome,
        completedAt: detail.completedAt,
        join: join,
      ));
    } catch (_) {
      // Intentionally silent — see doc comment above.
    }
  }
}
