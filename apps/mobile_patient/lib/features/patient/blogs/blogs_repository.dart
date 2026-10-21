import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import 'blogs_models.dart';

/// Backs `GET /articles` and `GET /articles/:id` — fully real today, no
/// backend gap (plan confirms this directly). Plain repository, no cubit:
/// this is a single fetch-then-browse screen with no cross-screen state to
/// own, matching the plan's own "Blogs ... single-fetch ... no cubit needed"
/// architecture note.
class BlogsRepository {
  BlogsRepository(this.api);
  final ApiClient api;

  dynamic _data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  List<dynamic> _listData(dynamic value) {
    if (value is List) return value;
    throw const FormatException('Unexpected response shape.');
  }

  Future<List<BlogArticleDto>> feed({int? limit, int? offset}) async {
    try {
      final raw = _listData(_data(await api.get('/articles', query: {
        if (limit != null) 'limit': limit,
        if (offset != null) 'offset': offset,
      })));
      return raw
          .map((e) =>
              BlogArticleDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Blogs', operation: 'load');
      rethrow;
    }
  }

  Future<BlogArticleDto> detail(String id) async {
    try {
      final raw = _data(await api.get('/articles/$id'));
      return BlogArticleDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Blogs', operation: 'load_detail');
      rethrow;
    }
  }
}
