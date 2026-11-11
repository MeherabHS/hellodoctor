import 'dart:typed_data';

import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import 'directory_models.dart';

/// Backs `GET /doctors` and `GET /doctors/:id`
/// (`services/backend/src/api/v1/doctor_handlers.rs`) — both fully real
/// today, no backend gap (plan §5 confirms this directly by router grep).
class DirectoryRepository {
  DirectoryRepository(this.api, {this.cache, this.connectivity});
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

  /// [modality]: `'VIDEO'` or `'CHAT'` — only doctors who charge a nonzero
  /// fee for that modality are returned (the backend's own filter).
  /// [specialty]: matched case-insensitively against the full specialty
  /// string. [search]: matched against name or hospital. All three
  /// deliberately map 1:1 onto `DoctorQuery` — no client-side filtering
  /// hides a server capability.
  Future<List<PublicDoctorDto>> search({
    String? modality,
    String? specialty,
    String? search,
  }) async {
    try {
      requireOnline();
      final raw = _listData(_data(await api.get('/doctors', query: {
        if (modality != null) 'modality': modality,
        if (specialty != null) 'specialty': specialty,
        if (search != null && search.isNotEmpty) 'search': search,
      })));
      final parsed = raw
          .map((e) =>
              PublicDoctorDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
      // Only the unfiltered "all doctors" result is cached — caching every
      // distinct filter combination would need a keyed-by-query cache slot
      // this store doesn't have, and the unfiltered list is the one Home's
      // offline fallback actually needs.
      if (modality == null &&
          specialty == null &&
          (search == null || search.isEmpty)) {
        await cache?.write('doctors', raw, ttl: const Duration(minutes: 15));
      }
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Directory', operation: 'search');
      rethrow;
    }
  }

  Future<List<PublicDoctorDto>> cachedDoctors() async {
    final saved = await cache?.read('doctors');
    if (saved?.data is List) {
      try {
        return (saved!.data as List)
            .map((e) =>
                PublicDoctorDto.fromJson(Map<String, dynamic>.from(e as Map)))
            .toList();
      } catch (_) {}
    }
    return const [];
  }

  Future<PublicDoctorDto> doctor(String id) async {
    try {
      requireOnline();
      final raw = _data(await api.get('/doctors/$id'));
      final parsed =
          PublicDoctorDto.fromJson(Map<String, dynamic>.from(raw as Map));
      await cache?.write('doctor_detail', raw,
          ttl: const Duration(minutes: 15));
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'DoctorProfile', operation: 'load');
      rethrow;
    }
  }

  /// Authenticated raw-bytes GET for a verified doctor's public photo
  /// (`GET /doctors/:id/photo` → `profile_picture_handlers.rs`, served via a
  /// `SECURITY DEFINER` DB function so any signed-in patient may fetch it).
  /// A 404 means the doctor genuinely has no photo set, not a transport
  /// failure — mirrors `ProfileRepository.fetchPhotoBytes`'s own 404-to-null
  /// handling for the patient's own photo.
  Future<Uint8List?> fetchDoctorPhotoBytes(String doctorId) async {
    try {
      final bytes = await api.getBytes('/doctors/$doctorId/photo');
      return Uint8List.fromList(bytes);
    } on ApiException catch (e) {
      if (e.status == 404) return null;
      rethrow;
    }
  }
}
