import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/home/home_repository.dart';

const _appointmentJson = {
  'id': 'a1',
  'appointment_number': 'HD-1',
  'doctor_id': 'd1',
  'doctor_display_name': 'Dr. Test',
  'primary_specialty': 'General',
  'modality': 'VIDEO',
  'status': 'CONFIRMED',
  'start_time': '2026-01-01T10:00:00Z',
  'end_time': '2026-01-01T10:10:00Z',
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `auth_cubit_test.dart`'s equivalent harness).
class FakeHomeRepository extends HomeRepository {
  FakeHomeRepository() : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  final calls = <String?>[];
  List<AppointmentSummaryDto>? result;

  @override
  Future<List<AppointmentSummaryDto>> upcomingAppointments(
      {String? profileId}) async {
    calls.add(profileId);
    return result ?? const [];
  }

  @override
  Future<List<AppointmentSummaryDto>> cachedUpcomingAppointments() async =>
      const [];
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
  late FakeHomeRepository repo;
  late HomeCubit cubit;

  setUp(() {
    repo = FakeHomeRepository();
    cubit = HomeCubit(repo);
  });

  tearDown(() => cubit.close());

  test('load() with no active profile queries appointments without profile_id',
      () async {
    await cubit.load();
    expect(repo.calls, [null]);
  });

  test('setActiveProfile reloads scoped to the dependent id', () async {
    await cubit.load();
    cubit.setActiveProfile('dep-1');
    await Future<void>.delayed(Duration.zero);
    expect(repo.calls, [null, 'dep-1']);
  });

  test('setActiveProfile with the same id already active is a no-op', () async {
    await cubit.load();
    cubit.setActiveProfile('dep-1');
    await Future<void>.delayed(Duration.zero);
    cubit.setActiveProfile('dep-1');
    await Future<void>.delayed(Duration.zero);
    expect(repo.calls, [null, 'dep-1']);
  });

  test('switching back to null re-queries the self profile', () async {
    repo.result = [AppointmentSummaryDto.fromJson(_appointmentJson)];
    await cubit.load();
    cubit.setActiveProfile('dep-1');
    await Future<void>.delayed(Duration.zero);
    repo.result = const [];
    cubit.setActiveProfile(null);
    await Future<void>.delayed(Duration.zero);
    expect(repo.calls, [null, 'dep-1', null]);
    expect(cubit.state.upcoming, isEmpty);
  });
}
