import 'dart:typed_data';

import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import 'facility_models.dart';

/// Published facilities. Photos are fetched through the authenticated API and
/// kept in a small in-memory cache keyed by their versioned path, so a new
/// Admin photo (new path) is fetched without an app update.
class FacilityRepository {
  FacilityRepository(this.api);
  final ApiClient api;

  static final _photos = <String, Uint8List>{};
  static const _maxPhotos = 40;

  dynamic _data(dynamic body) =>
      body is Map && body.containsKey('data') ? body['data'] : body;

  Future<FacilityPage> list(FacilityFilter filter,
      {int offset = 0, int limit = 20}) async {
    try {
      final response = await api.get('/facilities', query: {
        'limit': limit,
        'offset': offset,
        if (filter.search.trim().isNotEmpty) 'search': filter.search.trim(),
        if (filter.serviceCodes.isNotEmpty)
          'services': filter.serviceCodes.join(','),
        if (filter.youthFriendly) 'youth_friendly': true,
      });
      final body = response.data;
      final rows = _data(body);
      if (rows is! List) throw const FormatException('Unexpected response.');
      return FacilityPage(
        [
          for (final r in rows)
            if (r is Map) FacilityDto.fromJson(Map<String, dynamic>.from(r)),
        ],
        body is Map && body['total'] is int
            ? body['total'] as int
            : rows.length,
        body is Map && body['next_offset'] is int
            ? body['next_offset'] as int
            : null,
      );
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Facilities', operation: 'list');
      rethrow;
    }
  }

  Future<FacilityDto> detail(String id) async {
    try {
      final raw = _data((await api.get('/facilities/$id')).data);
      return FacilityDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Facilities', operation: 'detail');
      rethrow;
    }
  }

  /// Cached photo bytes, if already loaded.
  Uint8List? cachedPhoto(String path) => _photos[path];

  Future<Uint8List> photo(String path) async {
    final cached = _photos[path];
    if (cached != null) return cached;
    final bytes = Uint8List.fromList(await api.getBytes(path));
    if (_photos.length >= _maxPhotos) _photos.remove(_photos.keys.first);
    _photos[path] = bytes;
    return bytes;
  }
}
