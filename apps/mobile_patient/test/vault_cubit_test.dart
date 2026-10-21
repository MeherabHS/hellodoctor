import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/vault_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/vault_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/vault/vault_repository.dart';

const _prescriptionJson = {
  'kind': 'PRESCRIPTION',
  'id': 'rx-1',
  'appointment_id': 'apt-1',
  'rx_number': 'RX-20261010-ABCD',
  'page_number': 1,
  'mime_type': 'image/jpeg',
  'doctor_display_name': 'Dr. Sabrina Akter',
  'primary_specialty': 'Internal Medicine',
  'created_at': '2026-10-10T08:00:00Z',
};

const _intakeJson = {
  'kind': 'INTAKE',
  'id': 'doc-1',
  'appointment_id': 'apt-1',
  'page_number': 1,
  'mime_type': 'application/pdf',
  'created_at': '2026-10-09T08:00:00Z',
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `family_profiles_cubit_test.dart`'s harness).
class FakeVaultRepository extends VaultRepository {
  FakeVaultRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  List<PatientDocumentDto>? documentsResult;
  Object? documentsError;
  String? lastProfileId;
  int callCount = 0;

  @override
  Future<List<PatientDocumentDto>> documents({String? profileId}) async {
    callCount++;
    lastProfileId = profileId;
    if (documentsError != null) throw documentsError!;
    return documentsResult ?? const [];
  }

  @override
  Future<List<PatientDocumentDto>> cachedDocuments() async => const [];
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
  late FakeVaultRepository repo;
  late VaultCubit cubit;

  setUp(() {
    repo = FakeVaultRepository();
    cubit = VaultCubit(repo);
  });

  tearDown(() => cubit.close());

  group('load', () {
    test('populates both document kinds on success', () async {
      repo.documentsResult = [
        PatientDocumentDto.fromJson(_prescriptionJson),
        PatientDocumentDto.fromJson(_intakeJson),
      ];
      await cubit.load();
      expect(cubit.state.documents, hasLength(2));
      expect(cubit.state.error, isNull);
      final prescription =
          cubit.state.documents.firstWhere((d) => d.isPrescription);
      expect(prescription.rxNumber, 'RX-20261010-ABCD');
      expect(prescription.doctorDisplayName, 'Dr. Sabrina Akter');
    });

    test('surfaces a repository failure as state.error', () async {
      repo.documentsError = const ApiException(500, 'Server error.');
      await cubit.load();
      expect(cubit.state.error, isNotNull);
      expect(cubit.state.documents, isEmpty);
    });

    test('requests the self profile (profile_id omitted) by default', () async {
      await cubit.load();
      expect(repo.lastProfileId, isNull);
    });
  });

  group('setActiveProfile', () {
    test('re-queries scoped to the dependent id', () async {
      cubit.setActiveProfile('dep-1');
      await Future<void>.delayed(Duration.zero);
      expect(repo.lastProfileId, 'dep-1');
    });

    test('a no-op for the same id already active does not re-query', () async {
      await cubit.load();
      final callsAfterInitialLoad = repo.callCount;
      cubit.setActiveProfile(null);
      expect(repo.callCount, callsAfterInitialLoad);
    });

    test('switching back to null re-queries the self profile', () async {
      cubit.setActiveProfile('dep-1');
      await Future<void>.delayed(Duration.zero);
      cubit.setActiveProfile(null);
      await Future<void>.delayed(Duration.zero);
      expect(repo.lastProfileId, isNull);
    });
  });
}
