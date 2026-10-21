import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import 'profile_models.dart';

/// Backs `GET/POST /patients/me/profiles` and
/// `PATCH /patients/me/profiles/:id` (`patient_handlers.rs`) — the
/// multi-profile/family-support API (migration
/// `20261101000001_patient_profile_dependents`). Deliberately a separate
/// repository from `ProfileRepository`, which owns only the caller's own
/// self profile: this one owns the whole guardian+dependents list, matching
/// the backend's own split between `/patients/me` and
/// `/patients/me/profiles`.
class FamilyProfilesRepository {
  FamilyProfilesRepository(this.api, {this.cache, this.connectivity});
  final ApiClient api;
  final PatientCacheStore? cache;
  final ConnectivityService? connectivity;

  bool get isOffline => connectivity?.isOffline == true;

  void requireOnline() {
    if (isOffline) {
      throw const ApiException(
          null, 'Internet connection is required for this action.');
    }
  }

  dynamic _data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  List<dynamic> _listData(dynamic value) {
    if (value is List) return value;
    throw const FormatException('Unexpected response shape.');
  }

  Future<List<PatientProfileDto>> list() async {
    try {
      requireOnline();
      final raw = _listData(_data(await api.get('/patients/me/profiles')));
      final parsed = raw
          .map((e) =>
              PatientProfileDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
      await cache?.write('family_profiles', raw,
          ttl: const Duration(hours: 12));
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'FamilyProfiles', operation: 'load');
      rethrow;
    }
  }

  Future<List<PatientProfileDto>> cachedList() async {
    final saved = await cache?.read('family_profiles');
    if (saved?.data is List) {
      try {
        return (saved!.data as List)
            .map((e) =>
                PatientProfileDto.fromJson(Map<String, dynamic>.from(e as Map)))
            .toList();
      } catch (_) {}
    }
    return const [];
  }

  /// `POST /patients/me/profiles` — `idempotent: true` because this creates
  /// a new, undeletable-by-design dependent row (the backend has no
  /// soft-delete column for `patient_profiles`); a retried double-submit
  /// must never create two.
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
    try {
      requireOnline();
      final raw = _data(
          await api.post('/patients/me/profiles', idempotent: true, data: {
        'full_name': fullName,
        'date_of_birth': dateOfBirth,
        'gender': gender,
        'relationship': relationship,
        if (bloodGroup != null) 'blood_group': bloodGroup,
        if (weightKg != null) 'weight_kg': weightKg,
        if (emergencyContactPhone != null)
          'emergency_contact_phone': emergencyContactPhone,
        if (emergencyContactRelation != null)
          'emergency_contact_relation': emergencyContactRelation,
      }));
      return PatientProfileDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'FamilyProfiles', operation: 'create_dependent');
      rethrow;
    }
  }

  /// `PATCH /patients/me/profiles/:id` — every argument optional, same
  /// COALESCE-on-the-server contract as `ProfileRepository.updateProfile`.
  Future<void> updateDependent(
    String id, {
    String? fullName,
    String? dateOfBirth,
    String? gender,
    String? bloodGroup,
    String? weightKg,
    String? emergencyContactPhone,
    String? emergencyContactRelation,
    String? relationship,
  }) async {
    try {
      requireOnline();
      await api.patch('/patients/me/profiles/$id', data: {
        if (fullName != null) 'full_name': fullName,
        if (dateOfBirth != null) 'date_of_birth': dateOfBirth,
        if (gender != null) 'gender': gender,
        if (bloodGroup != null) 'blood_group': bloodGroup,
        if (weightKg != null) 'weight_kg': weightKg,
        if (emergencyContactPhone != null)
          'emergency_contact_phone': emergencyContactPhone,
        if (emergencyContactRelation != null)
          'emergency_contact_relation': emergencyContactRelation,
        if (relationship != null) 'relationship': relationship,
      });
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'FamilyProfiles', operation: 'update_dependent');
      rethrow;
    }
  }
}
