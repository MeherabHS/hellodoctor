import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/directory_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/directory_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/directory_repository.dart';

const _sabrinaJson = {
  'id': 'doc-1',
  'full_name': 'Dr. Sabrina Akter',
  'license_number': '45821',
  'license_authority': 'KMPDC',
  'license_country': 'Kenya',
  'primary_specialty': 'Internal Medicine',
  'experience_years': 8,
  'current_hospital': 'Square Hospital',
  'qualifications': ['MBBS', 'FCPS'],
  'consultation_fee_video': '630.00',
  'consultation_fee_chat': '300.00',
  'is_on_duty': true,
};

const _karimJson = {
  'id': 'doc-2',
  'full_name': 'Dr. Karim Hossain',
  'license_number': '31204',
  'license_authority': 'KMPDC',
  'license_country': 'Kenya',
  'primary_specialty': 'Cardiology',
  'experience_years': 12,
  'current_hospital': 'Heart Foundation',
  'qualifications': ['MD'],
  'consultation_fee_video': '800.00',
  'consultation_fee_chat': '300.00',
  'is_on_duty': false,
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `family_profiles_cubit_test.dart`'s harness).
class FakeDirectoryRepository extends DirectoryRepository {
  FakeDirectoryRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  List<PublicDoctorDto>? searchResult;
  Object? searchError;
  String? lastModality;
  int searchCalls = 0;

  @override
  Future<List<PublicDoctorDto>> search({
    String? modality,
    String? specialty,
    String? search,
  }) async {
    searchCalls++;
    lastModality = modality;
    if (searchError != null) throw searchError!;
    return searchResult ?? const [];
  }

  @override
  Future<List<PublicDoctorDto>> cachedDoctors() async => const [];
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
  late FakeDirectoryRepository repo;
  late DirectoryCubit cubit;

  setUp(() {
    repo = FakeDirectoryRepository();
    cubit = DirectoryCubit(repo);
  });

  tearDown(() => cubit.close());

  group('load', () {
    test('populates doctors on success', () async {
      repo.searchResult = [
        PublicDoctorDto.fromJson(_sabrinaJson),
        PublicDoctorDto.fromJson(_karimJson),
      ];
      await cubit.load();
      expect(cubit.state.doctors, hasLength(2));
      expect(cubit.state.visible, hasLength(2));
      expect(cubit.state.error, isNull);
    });

    test('surfaces a repository failure as state.error', () async {
      repo.searchError = const ApiException(500, 'Server error.');
      await cubit.load();
      expect(cubit.state.error, isNotNull);
      expect(cubit.state.doctors, isEmpty);
    });
  });

  group('setModalityTab', () {
    test('a no-op for the same tab does not re-query', () async {
      await cubit.load();
      expect(repo.searchCalls, 1);
      cubit.setModalityTab('all');
      expect(repo.searchCalls, 1);
    });

    test('switching to video re-queries with the VIDEO modality filter',
        () async {
      await cubit.load();
      cubit.setModalityTab('video');
      await Future<void>.delayed(Duration.zero);
      expect(cubit.state.modalityTab, 'video');
      expect(repo.lastModality, 'VIDEO');
    });

    test('switching to chat re-queries with the CHAT modality filter',
        () async {
      await cubit.load();
      cubit.setModalityTab('chat');
      await Future<void>.delayed(Duration.zero);
      expect(repo.lastModality, 'CHAT');
    });
  });

  group('specialty and search filtering (state.visible)', () {
    setUp(() async {
      repo.searchResult = [
        PublicDoctorDto.fromJson(_sabrinaJson),
        PublicDoctorDto.fromJson(_karimJson),
      ];
      await cubit.load();
    });

    test('specialty narrows the visible list without a new request', () async {
      cubit.setSpecialty('Cardiology');
      expect(repo.searchCalls, 1);
      expect(cubit.state.visible, hasLength(1));
      expect(cubit.state.visible.single.fullName, 'Dr. Karim Hossain');
    });

    test('search matches name, specialty, or hospital case-insensitively', () {
      cubit.setSearch('square');
      expect(cubit.state.visible.single.fullName, 'Dr. Sabrina Akter');
    });

    test('resetFilters clears both specialty and search', () {
      cubit.setSpecialty('Cardiology');
      cubit.setSearch('nonexistent');
      expect(cubit.state.visible, isEmpty);
      cubit.resetFilters();
      expect(cubit.state.visible, hasLength(2));
    });
  });
}
