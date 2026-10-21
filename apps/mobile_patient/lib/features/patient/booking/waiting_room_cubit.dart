import 'package:flutter_bloc/flutter_bloc.dart';

import 'booking_repository.dart';

/// Appointment statuses a Waiting Room session can no longer act on — no
/// further polling or joining makes sense once here.
const kWaitingRoomTerminalStatuses = {'CANCELLED', 'COMPLETED', 'EXPIRED'};

class WaitingRoomState {
  const WaitingRoomState({
    required this.status,
    this.terminal = false,
    this.clinicalOutcome,
    this.completedAt,
  });
  final String status;
  final bool terminal;
  final String? clinicalOutcome, completedAt;

  WaitingRoomState copyWith({
    String? status,
    bool? terminal,
    String? clinicalOutcome,
    String? completedAt,
  }) =>
      WaitingRoomState(
        status: status ?? this.status,
        terminal: terminal ?? this.terminal,
        clinicalOutcome: clinicalOutcome ?? this.clinicalOutcome,
        completedAt: completedAt ?? this.completedAt,
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
  }) : super(WaitingRoomState(
          status: initialStatus,
          terminal: kWaitingRoomTerminalStatuses.contains(initialStatus),
        ));
  final BookingRepository repo;
  final String appointmentId;

  /// Best-effort background refresh — a transient failure must not surface
  /// as an error the patient didn't act on, and must not flip `terminal`
  /// based on stale/absent data.
  Future<void> poll() async {
    if (state.terminal) return;
    try {
      final detail = await repo.appointment(appointmentId);
      if (isClosed) return;
      emit(state.copyWith(
        status: detail.status,
        terminal: kWaitingRoomTerminalStatuses.contains(detail.status),
        clinicalOutcome: detail.clinicalOutcome,
        completedAt: detail.completedAt,
      ));
    } catch (_) {
      // Intentionally silent — see doc comment above.
    }
  }
}
