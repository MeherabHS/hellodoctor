import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import 'qna_models.dart';

/// Backs `GET`/`POST /patients/me/qna/questions` (plan's gap #6, M8 — the
/// patient-facing half of the Free Health Q&A system; the Doctor answer-flow
/// side already existed under `/doctor/qna/*`).
class QnaRepository {
  QnaRepository(this.api);
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

  Future<List<AnsweredQuestionDto>> feed({int? limit, int? offset}) async {
    try {
      final raw =
          _listData(_data(await api.get('/patients/me/qna/questions', query: {
        'include_own_open': true,
        if (limit != null) 'limit': limit,
        if (offset != null) 'offset': offset,
      })));
      return raw
          .map((e) =>
              AnsweredQuestionDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .where((q) => !q.isInternalFixture)
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Qna', operation: 'load');
      rethrow;
    }
  }

  /// [profileId]: ask on behalf of a dependent profile (§5b); the caller's
  /// own self profile when `null`. Returns the newly created `OPEN` question
  /// (not yet visible in [feed] — it only appears once a doctor answers it).
  Future<AnsweredQuestionDto> submit({
    required String questionText,
    String? category,
    String? profileId,
  }) async {
    try {
      final response =
          await api.post('/patients/me/qna/questions', idempotent: true, data: {
        'question_text': questionText,
        if (category != null) 'category': category,
        if (profileId != null) 'profile_id': profileId,
      });
      final question = AnsweredQuestionDto.fromJson(
          Map<String, dynamic>.from(_data(response) as Map));
      if (question.id.isEmpty || question.status != 'OPEN') {
        throw const FormatException('Unexpected question creation response.');
      }
      return question;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Qna', operation: 'submit');
      rethrow;
    }
  }
}
