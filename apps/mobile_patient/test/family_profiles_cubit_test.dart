import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_models.dart';

const _selfJson = {
  'id': 'self-1',
  'user_id': 'user-1',
  'full_name': 'Rafiq Ahmed',
  'date_of_birth': '1992-01-01',
  'gender': 'MALE',
  'guardian_patient_id': null,
  'relationship': null,
};

const _dependentJson = {
  'id': 'dep-1',
  'user_id': null,
  'full_name': 'Amina Ahmed',
  'date_of_birth': '1994-05-05',
  'gender': 'FEMALE',
  'guardian_patient_id': 'self-1',
  'relationship': 'Spouse',
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `auth_cubit_test.dart`'s equivalent harness).
class FakeFamilyProfilesRepository extends FamilyProfilesRepository {
  FakeFamilyProfilesRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  List<PatientProfileDto>? listResult;
  Object? listError;
  PatientProfileDto? createResult;
  Object? createError;
  int listCalls = 0;
  Map<String, dynamic>? lastCreateArgs;

  @override
  Future<List<PatientProfileDto>> list() async {
    listCalls++;
    if (listError != null) throw listError!;
    return listResult ?? const [];
  }

  @override
  Future<List<PatientProfileDto>> cachedList() async => const [];

  @override
  Future<PatientProfileDto> createDependent({
    required String fullName,
    required String dateOfBirth,
    required String gender,
    required String relationship,
    String? bloodGroup,
    String? weightKg,
    String? emergencyContactPhone,
    String? emergencyContactRelation,
  }) async {
    lastCreateArgs = {
      'fullName': fullName,
      'dateOfBirth': dateOfBirth,
      'gender': gender,
      'relationship': relationship,
    };
    if (createError != null) throw createError!;
    return createResult!;
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
  late FakeFamilyProfilesRepository repo;
  late FamilyProfilesCubit cubit;

  setUp(() {
    repo = FakeFamilyProfilesRepository();
    cubit = FamilyProfilesCubit(repo);
  });

  tearDown(() => cubit.close());

  group('load', () {
    test('populates profiles and defaults activeProfile to the self row',
        () async {
      repo.listResult = [
        PatientProfileDto.fromJson(_dependentJson),
        PatientProfileDto.fromJson(_selfJson),
      ];
      await cubit.load();
      expect(cubit.state.profiles, hasLength(2));
      expect(cubit.state.activeProfileId, isNull);
      expect(cubit.state.activeProfile?.id, 'self-1');
      expect(cubit.state.activeProfile?.isSelf, isTrue);
    });

    test('surfaces a repository failure as state.error', () async {
      repo.listError = const ApiException(500, 'Server error.');
      await cubit.load();
      expect(cubit.state.error, isNotNull);
      expect(cubit.state.profiles, isEmpty);
    });
  });

  group('setActiveProfile', () {
    test('switches the active profile to a known dependent id', () async {
      repo.listResult = [
        PatientProfileDto.fromJson(_selfJson),
        PatientProfileDto.fromJson(_dependentJson),
      ];
      await cubit.load();
      cubit.setActiveProfile('dep-1');
      expect(cubit.state.activeProfileId, 'dep-1');
      expect(cubit.state.activeProfile?.fullName, 'Amina Ahmed');
      expect(cubit.state.activeProfile?.isSelf, isFalse);
    });

    test('ignores an id that is not in the loaded profile list', () async {
      repo.listResult = [PatientProfileDto.fromJson(_selfJson)];
      await cubit.load();
      cubit.setActiveProfile('not-a-real-id');
      expect(cubit.state.activeProfileId, isNull);
    });

    test('switching back to null returns to the self profile', () async {
      repo.listResult = [
        PatientProfileDto.fromJson(_selfJson),
        PatientProfileDto.fromJson(_dependentJson),
      ];
      await cubit.load();
      cubit.setActiveProfile('dep-1');
      cubit.setActiveProfile(null);
      expect(cubit.state.activeProfileId, isNull);
      expect(cubit.state.activeProfile?.isSelf, isTrue);
    });
  });

  group('createDependent', () {
    test('appends the created dependent to state.profiles on success',
        () async {
      repo.listResult = [PatientProfileDto.fromJson(_selfJson)];
      await cubit.load();
      repo.createResult = PatientProfileDto.fromJson(_dependentJson);
      final created = await cubit.createDependent(
        fullName: 'Amina Ahmed',
        dateOfBirth: '1994-05-05',
        gender: 'FEMALE',
        relationship: 'Spouse',
      );
      expect(created?.id, 'dep-1');
      expect(cubit.state.profiles, hasLength(2));
      expect(cubit.state.creating, isFalse);
      expect(repo.lastCreateArgs?['relationship'], 'Spouse');
    });

    test('a double-submit while creating is in flight is ignored', () async {
      repo.listResult = [PatientProfileDto.fromJson(_selfJson)];
      await cubit.load();
      repo.createResult = PatientProfileDto.fromJson(_dependentJson);
      final first = cubit.createDependent(
        fullName: 'Amina Ahmed',
        dateOfBirth: '1994-05-05',
        gender: 'FEMALE',
        relationship: 'Spouse',
      );
      final second = await cubit.createDependent(
        fullName: 'Amina Ahmed',
        dateOfBirth: '1994-05-05',
        gender: 'FEMALE',
        relationship: 'Spouse',
      );
      expect(second, isNull);
      await first;
      expect(cubit.state.profiles, hasLength(2));
    });

    test('surfaces a repository failure as state.createError, not thrown',
        () async {
      repo.listResult = [PatientProfileDto.fromJson(_selfJson)];
      await cubit.load();
      repo.createError = const ApiException(422, 'full_name is required.');
      final created = await cubit.createDependent(
        fullName: '',
        dateOfBirth: '1994-05-05',
        gender: 'FEMALE',
        relationship: 'Spouse',
      );
      expect(created, isNull);
      expect(cubit.state.createError, isNotNull);
      expect(cubit.state.profiles, hasLength(1));
    });
  });
}
