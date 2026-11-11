import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/waiting_room_cubit.dart';

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `family_profiles_cubit_test.dart`'s harness).
class FakeBookingRepository extends BookingRepository {
  FakeBookingRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  AppointmentDetailDto? pollResult;
  Object? pollError;

  @override
  Future<AppointmentDetailDto> appointment(String appointmentId) async {
    if (pollError != null) throw pollError!;
    return pollResult ??
        const AppointmentDetailDto(id: 'apt-1', status: 'PENDING_PAYMENT', paymentPolicy: 'REQUIRED');
  }
}

class _NoopStore implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

void main() {
  late FakeBookingRepository repo;

  setUp(() {
    repo = FakeBookingRepository();
  });

  group('initial state', () {
    test('a PENDING_PAYMENT appointment is not terminal', () {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'PENDING_PAYMENT');
      expect(cubit.state.terminal, isFalse);
      cubit.close();
    });

    test('a CANCELLED appointment starts terminal', () {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'CANCELLED');
      expect(cubit.state.terminal, isTrue);
      cubit.close();
    });

    test('a REFUNDED appointment starts terminal', () {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'REFUNDED');
      expect(cubit.state.terminal, isTrue);
      cubit.close();
    });

    test('a DISPUTED appointment starts terminal', () {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'DISPUTED');
      expect(cubit.state.terminal, isTrue);
      cubit.close();
    });
  });

  group('poll', () {
    test('updates status and terminal from the server response', () async {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'PENDING_PAYMENT');
      repo.pollResult =
          const AppointmentDetailDto(id: 'apt-1', status: 'CONFIRMED', paymentPolicy: 'REQUIRED');
      await cubit.poll();
      expect(cubit.state.status, 'CONFIRMED');
      expect(cubit.state.terminal, isFalse);
      cubit.close();
    });

    test('a terminal status stops being joinable', () async {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'CONFIRMED');
      repo.pollResult =
          const AppointmentDetailDto(id: 'apt-1', status: 'COMPLETED', paymentPolicy: 'REQUIRED');
      await cubit.poll();
      expect(cubit.state.terminal, isTrue);
      cubit.close();
    });

    test('a transient failure is silent and does not change state', () async {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'PENDING_PAYMENT');
      repo.pollError = const ApiException(null, 'offline');
      await cubit.poll();
      expect(cubit.state.status, 'PENDING_PAYMENT');
      cubit.close();
    });

    test('a terminal state is never re-polled', () async {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'CANCELLED');
      repo.pollResult =
          const AppointmentDetailDto(id: 'apt-1', status: 'CONFIRMED', paymentPolicy: 'REQUIRED');
      await cubit.poll();
      expect(cubit.state.status, 'CANCELLED',
          reason: 'a terminal session must not revive');
      cubit.close();
    });

    test('a COMPLETED response carries the clinical outcome and completedAt',
        () async {
      final cubit = WaitingRoomCubit(repo,
          appointmentId: 'apt-1', initialStatus: 'CONFIRMED');
      repo.pollResult = const AppointmentDetailDto(
        id: 'apt-1',
        status: 'COMPLETED',
        paymentPolicy: 'REQUIRED',
        clinicalOutcome: 'COMPLETED_WITH_RX',
        completedAt: '2026-10-10T09:00:00Z',
      );
      await cubit.poll();
      expect(cubit.state.terminal, isTrue);
      expect(cubit.state.clinicalOutcome, 'COMPLETED_WITH_RX');
      expect(cubit.state.completedAt, '2026-10-10T09:00:00Z');
      cubit.close();
    });
  });
}
