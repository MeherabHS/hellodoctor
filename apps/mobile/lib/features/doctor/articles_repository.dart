import 'dart:math';
import '../../core/network/api_client.dart';

String articleCreationKey() =>
    List.generate(16, (_) => Random.secure().nextInt(256))
        .map((v) => v.toRadixString(16).padLeft(2, '0'))
        .join();

class DoctorArticle {
  DoctorArticle.fromJson(Map<String, dynamic> value)
      : id = value['id'] as String,
        title = value['title'] as String,
        summary = value['summary'] as String,
        body = value['body'] as String,
        specialty = value['specialty'] as String,
        status = value['status'] as String,
        version = value['version'] as int,
        references = (value['references_list'] as List).cast<String>() {
    if (!['DRAFT', 'PUBLISHED', 'ARCHIVED'].contains(status)) {
      throw const ApiException(502, 'Unknown article status.');
    }
  }
  final String id, title, summary, body, specialty, status;
  final int version;
  final List<String> references;
  bool get editable => status != 'ARCHIVED';
  bool get publishable => status == 'DRAFT';
}

class ArticlesRepository {
  ArticlesRepository(this.api);
  final ApiClient api;
  Future<DoctorArticle> get(String id) async {
    final response = await api.get('/doctor/articles/$id');
    return DoctorArticle.fromJson(
        Map<String, dynamic>.from(response.data['data']));
  }

  Future<List<DoctorArticle>> list({int offset = 0}) async {
    final response = await api.get('/doctor/articles?limit=20&offset=$offset');
    return (response.data['data'] as List)
        .map((v) => DoctorArticle.fromJson(Map<String, dynamic>.from(v)))
        .toList();
  }

  Future<DoctorArticle> save(
      {DoctorArticle? article,
      required String creationKey,
      required Map<String, dynamic> content}) async {
    final response = article == null
        ? await api.post('/doctor/articles',
            data: {'creation_key': creationKey, 'content': content})
        : await api.patch('/doctor/articles/${article.id}',
            data: {'version': article.version, 'content': content});
    return DoctorArticle.fromJson(
        Map<String, dynamic>.from(response.data['data']));
  }

  Future<DoctorArticle> transition(DoctorArticle article,
      {required bool publish}) async {
    final response = await api.post(
        '/doctor/articles/${article.id}/${publish ? 'publish' : 'archive'}',
        data: {'version': article.version});
    return DoctorArticle.fromJson(
        Map<String, dynamic>.from(response.data['data']));
  }
}
