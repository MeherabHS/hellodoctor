typedef QuizJson = Map<String, dynamic>;
bool quizId(dynamic value) =>
    value is String &&
    RegExp(r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$')
        .hasMatch(value);

class QuizPublication {
  const QuizPublication(this.revisionId, this.questionCount);
  final String revisionId;
  final int questionCount;
  static QuizPublication? parse(dynamic value) {
    if (value is! Map ||
        value['schema_version'] != 1 ||
        value['included'] != true ||
        !quizId(value['revision_id']) ||
        value['question_count'] is! int ||
        value['question_count'] < 1 ||
        value['question_count'] > 50) {
      return null;
    }
    return QuizPublication(
        value['revision_id'] as String, value['question_count'] as int);
  }
}

class QuizOption {
  const QuizOption(this.id, this.textEn, this.textSw);
  final String id, textEn, textSw;
}

class QuizQuestion {
  const QuizQuestion(this.id, this.textEn, this.textSw, this.options);
  final String id, textEn, textSw;
  final List<QuizOption> options;
  bool useSw(String language) =>
      language == 'sw' &&
      textSw.trim().isNotEmpty &&
      options.every((o) => o.textSw.trim().isNotEmpty);
  String textFor(String language) => useSw(language) ? textSw : textEn;
  String optionFor(QuizOption option, String language) =>
      useSw(language) ? option.textSw : option.textEn;
}

class PublishedQuiz {
  const PublishedQuiz(this.articleId, this.revisionId, this.questions);
  final String articleId, revisionId;
  final List<QuizQuestion> questions;
  factory PublishedQuiz.fromJson(QuizJson json) {
    if (json['schema_version'] != 1 ||
        json['included'] != true ||
        !quizId(json['article_id']) ||
        !quizId(json['revision_id']) ||
        json['questions'] is! List) {
      throw const FormatException('Invalid published quiz contract.');
    }
    final raw = json['questions'] as List;
    if (raw.isEmpty ||
        raw.length > 50 ||
        json['question_count'] != raw.length) {
      throw const FormatException('Invalid quiz question count.');
    }
    final ids = <String>{}, optionIds = <String>{};
    final questions = <QuizQuestion>[];
    for (final value in raw) {
      if (value is! Map ||
          !quizId(value['id']) ||
          !ids.add(value['id'] as String) ||
          value['text_en'] is! String ||
          (value['text_en'] as String).trim().isEmpty ||
          value['text_sw'] is! String ||
          value['options'] is! List ||
          value.containsKey('correct_option_id') ||
          value.containsKey('explanation_en') ||
          value.containsKey('explanation_sw')) {
        throw const FormatException('Invalid public question contract.');
      }
      final choices = value['options'] as List;
      if (choices.length < 2 || choices.length > 6) {
        throw const FormatException('Invalid quiz options.');
      }
      final options = <QuizOption>[];
      for (final choice in choices) {
        if (choice is! Map ||
            !quizId(choice['id']) ||
            !optionIds.add(choice['id'] as String) ||
            choice['text_en'] is! String ||
            (choice['text_en'] as String).trim().isEmpty ||
            choice['text_sw'] is! String) {
          throw const FormatException('Invalid public option contract.');
        }
        options.add(
            QuizOption(choice['id'], choice['text_en'], choice['text_sw']));
      }
      questions.add(QuizQuestion(value['id'], value['text_en'],
          value['text_sw'], List.unmodifiable(options)));
    }
    return PublishedQuiz(
        json['article_id'], json['revision_id'], List.unmodifiable(questions));
  }
}

class QuizAnswerFeedback {
  const QuizAnswerFeedback(
      this.articleId,
      this.revisionId,
      this.questionId,
      this.correct,
      this.correctOptionId,
      this.explanationEn,
      this.explanationSw);
  final String articleId,
      revisionId,
      questionId,
      correctOptionId,
      explanationEn,
      explanationSw;
  final bool correct;
  String explanationFor(String language) =>
      language == 'sw' && explanationSw.trim().isNotEmpty
          ? explanationSw
          : explanationEn;
  factory QuizAnswerFeedback.fromJson(QuizJson json) {
    if (json['schema_version'] != 1 ||
        !quizId(json['article_id']) ||
        !quizId(json['revision_id']) ||
        !quizId(json['question_id']) ||
        !quizId(json['correct_option_id']) ||
        json['correct'] is! bool ||
        json['explanation_en'] is! String ||
        json['explanation_sw'] is! String) {
      throw const FormatException('Invalid quiz feedback contract.');
    }
    return QuizAnswerFeedback(
        json['article_id'],
        json['revision_id'],
        json['question_id'],
        json['correct'],
        json['correct_option_id'],
        json['explanation_en'],
        json['explanation_sw']);
  }
}
