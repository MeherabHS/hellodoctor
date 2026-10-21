import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import 'home_models.dart';

class HomeRepository {
  HomeRepository(this.api, {this.cache, this.connectivity});
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

  /// [profileId]: `null` (the default) means the caller's own self profile;
  /// a dependent's id scopes the query to that dependent instead (§5b
  /// multi-profile/family support — `AuthContext::acting_patient_profile_id`
  /// on the backend). Only the self-profile result is cached (see
  /// [cachedUpcomingAppointments]) — the fixed `PatientCacheStore` key
  /// namespace has no per-profile slot, and silently showing one profile's
  /// cached appointments while viewing another would be a real
  /// cross-profile data leak, not just a staleness issue.
  Future<List<AppointmentSummaryDto>> upcomingAppointments(
      {String? profileId}) async {
    try {
      requireOnline();
      final raw =
          _listData(_data(await api.get('/patients/me/appointments', query: {
        'status': 'upcoming',
        if (profileId != null) 'profile_id': profileId,
      })));
      final parsed = raw
          .map((e) => AppointmentSummaryDto.fromJson(
              Map<String, dynamic>.from(e as Map)))
          .toList();
      if (profileId == null) {
        await cache?.write('appointments', raw,
            ttl: const Duration(minutes: 5));
      }
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Home', operation: 'load');
      rethrow;
    }
  }

  /// Only ever returns the self profile's cached appointments — see
  /// [upcomingAppointments]'s [profileId] doc. Callers must not use this as
  /// an offline fallback while a dependent is the active profile.
  Future<List<AppointmentSummaryDto>> cachedUpcomingAppointments() async {
    final saved = await cache?.read('appointments');
    if (saved?.data is List) {
      try {
        return (saved!.data as List)
            .map((e) => AppointmentSummaryDto.fromJson(
                Map<String, dynamic>.from(e as Map)))
            .toList();
      } catch (_) {}
    }
    return const [];
  }
}
