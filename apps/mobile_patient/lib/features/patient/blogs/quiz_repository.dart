import '../../../core/network/api_client.dart';
import 'quiz_models.dart';

/// No caches, incident payloads, answer logs or persisted attempt records.
class ArticleQuizRepository {
  ArticleQuizRepository(this.api);
  final ApiClient api;
  String _path(String articleId) {
    if (!quizId(articleId)) {
      throw const FormatException('Invalid article ID.');
    }
    return '/articles/$articleId/quiz';
  }

  QuizJson _data(dynamic value) {
    if (value is! Map || value['data'] is! Map) {
      throw const FormatException('Invalid quiz response.');
    }
    return Map<String, dynamic>.from(value['data'] as Map);
  }

  Future<PublishedQuiz> read(String articleId) async =>
      PublishedQuiz.fromJson(_data((await api.get(_path(articleId))).data));
  Future<QuizAnswerFeedback> check(String articleId, String revisionId,
      String questionId, String optionId) async {
    if (![revisionId, questionId, optionId].every(quizId)) {
      throw const FormatException('Invalid quiz selection.');
    }
    final response = await api.post('${_path(articleId)}/check', data: {
      'revision_id': revisionId,
      'question_id': questionId,
      'selected_option_id': optionId
    });
    return QuizAnswerFeedback.fromJson(_data(response.data));
  }
}
