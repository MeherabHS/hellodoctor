import 'dart:convert';
import 'dart:typed_data';

import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import 'profile_models.dart';

class ProfileRepository {
  ProfileRepository(this.api, {this.cache, this.connectivity});
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

  Future<CachedResource?> cached(String key) =>
      cache?.read(key) ?? Future<CachedResource?>.value();

  Future<void> cacheWrite(String key, dynamic value, {Duration? ttl}) =>
      cache?.write(key, value, ttl: ttl) ?? Future<void>.value();

  dynamic data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  Map<String, dynamic> mapData(dynamic value) => value is Map
      ? Map<String, dynamic>.from(value)
      : throw const FormatException('Unexpected response shape.');

  Future<PatientProfileDto> profile() async {
    try {
      requireOnline();
      final raw = mapData(data(await api.get('/patients/me')));
      await cacheWrite('profile', raw, ttl: const Duration(hours: 12));
      return PatientProfileDto.fromJson(raw);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Profile', operation: 'load');
      rethrow;
    }
  }

  Future<PatientProfileDto?> cachedProfile() async {
    final saved = await cached('profile');
    if (saved?.data is Map) {
      try {
        return PatientProfileDto.fromJson(
            Map<String, dynamic>.from(saved!.data as Map));
      } catch (_) {}
    }
    return null;
  }

  /// `PATCH /patients/me` — every argument is optional; omitted fields are
  /// left unchanged server-side (`patient_repo::update`'s COALESCE
  /// contract). Values are sent as plain strings matching the JSON shapes
  /// `PatientProfileUpdate` (`patient_handlers.rs`) deserializes
  /// (`date_of_birth` as `YYYY-MM-DD`, `weight_kg` as a decimal string).
  Future<void> updateProfile({
    String? fullName,
    String? dateOfBirth,
    String? gender,
    String? bloodGroup,
    String? weightKg,
    String? emergencyContactPhone,
    String? emergencyContactRelation,
  }) async {
    try {
      requireOnline();
      await api.patch('/patients/me', data: {
        if (fullName != null) 'full_name': fullName,
        if (dateOfBirth != null) 'date_of_birth': dateOfBirth,
        if (gender != null) 'gender': gender,
        if (bloodGroup != null) 'blood_group': bloodGroup,
        if (weightKg != null) 'weight_kg': weightKg,
        if (emergencyContactPhone != null)
          'emergency_contact_phone': emergencyContactPhone,
        if (emergencyContactRelation != null)
          'emergency_contact_relation': emergencyContactRelation,
      });
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Profile', operation: 'update_profile');
      rethrow;
    }
  }

  /// `profileId` targets a guardian-managed dependent's photo; omitted
  /// (self) is the common case. Compression/resizing and the 40KB ceiling
  /// are entirely server-side (`UploadSecurityService`) — this client
  /// deliberately sends the picked image's bytes unmodified, the same
  /// "server is the sole source of truth on size" choice already made for
  /// prescription intake uploads (see `upload_validation.dart`).
  Future<void> uploadPhoto({
    required Uint8List bytes,
    required String mimeType,
    String? profileId,
  }) async {
    try {
      requireOnline();
      await api.post('/patients/me/photo', data: {
        'mime_type': mimeType,
        'base64_content': base64Encode(bytes),
        if (profileId != null) 'profile_id': profileId,
      });
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Profile', operation: 'upload_photo');
      rethrow;
    }
  }

  Future<void> deletePhoto({String? profileId}) async {
    try {
      requireOnline();
      await api.delete('/patients/me/photo',
          query: profileId != null ? {'profile_id': profileId} : null);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Profile', operation: 'delete_photo');
      rethrow;
    }
  }

  /// Returns `null` when no photo is set (a 404 from the backend) rather
  /// than throwing — "no photo yet" is an expected, common state for this
  /// optional feature, not an error condition the UI needs to surface.
  /// Any other failure (network, 5xx, ...) still rethrows, since those ARE
  /// genuine problems the caller should be able to distinguish and retry.
  Future<Uint8List?> fetchPhotoBytes({String? profileId}) async {
    try {
      requireOnline();
      final bytes = await api.getBytes('/patients/me/photo',
          query: profileId != null ? {'profile_id': profileId} : null);
      return Uint8List.fromList(bytes);
    } on ApiException catch (error, stack) {
      if (error.status == 404) return null;
      ClientErrorReporter.observe(error, stack,
          screen: 'Profile', operation: 'fetch_photo');
      rethrow;
    }
  }
}
