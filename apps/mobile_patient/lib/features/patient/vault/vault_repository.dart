import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import 'vault_models.dart';

/// Backs `GET /patients/me/documents` (plan's gap #2, M7 Health Vault) and
/// resolving a real, short-lived download URL for one document via the
/// pre-existing `GET /appointments/:id/documents/:document_id`.
class VaultRepository {
  VaultRepository(this.api, {this.cache, this.connectivity});
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

  /// [profileId]: `null` lists the caller's own self profile's documents; a
  /// dependent's id scopes to that dependent instead (§5b). Only the
  /// self-profile result is cached, matching every other profile-scoped
  /// repository in this app — a dependent's cached documents could otherwise
  /// leak into view while offline under the wrong active profile.
  Future<List<PatientDocumentDto>> documents({String? profileId}) async {
    try {
      requireOnline();
      final raw =
          _listData(_data(await api.get('/patients/me/documents', query: {
        if (profileId != null) 'profile_id': profileId,
      })));
      final parsed = raw
          .map((e) =>
              PatientDocumentDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
      if (profileId == null) {
        await cache?.write('documents', raw, ttl: const Duration(minutes: 15));
      }
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'HealthVault', operation: 'load');
      rethrow;
    }
  }

  Future<List<PatientDocumentDto>> cachedDocuments() async {
    final saved = await cache?.read('documents');
    if (saved?.data is List) {
      try {
        return (saved!.data as List)
            .map((e) => PatientDocumentDto.fromJson(
                Map<String, dynamic>.from(e as Map)))
            .toList();
      } catch (_) {}
    }
    return const [];
  }

  /// Resolves an absolute, openable URL for one document — the response's
  /// `download_url` is a server-relative path
  /// (`/api/v1/documents/download?token=...`) backed by a one-time,
  /// short-lived Redis grant (`prescription_handlers::get_document_download_url`),
  /// not a client-supplied id, so this must be re-resolved every time a
  /// document is opened rather than cached.
  Future<Uri> resolveDownloadUrl(
      String appointmentId, String documentId) async {
    try {
      requireOnline();
      final raw = _data(
          await api.get('/appointments/$appointmentId/documents/$documentId'));
      final path =
          Map<String, dynamic>.from(raw as Map)['download_url'] as String;
      return api.resolveServerUrl(path);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'HealthVault', operation: 'resolve_download_url');
      rethrow;
    }
  }

  /// Remediation E2: mints a fresh, single-use document-download grant via
  /// [resolveDownloadUrl] and immediately redeems it for the actual bytes,
  /// in-memory — never via `launchUrl`/an external application. The grant is
  /// one-time (server-side `GETDEL`), so this must be called again for every
  /// retry rather than reusing a previously-resolved URL.
  Future<TypedBytes> fetchDocumentBytes(
      String appointmentId, String documentId) async {
    final url = await resolveDownloadUrl(appointmentId, documentId);
    try {
      return await api.getBytesWithType(url.toString());
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'HealthVault', operation: 'fetch_document_bytes');
      rethrow;
    }
  }
}
