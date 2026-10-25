import 'package:flutter/foundation.dart';
import '../../../core/network/api_client.dart';
import 'quiz_models.dart';
import 'quiz_repository.dart';

enum QuizPhase {
  introduction,
  loading,
  question,
  complete,
  unavailable,
  changed
}

enum QuizFailure { load, answer, session, rateLimited }

/// One screen's memory only; asynchronous work cannot update a disposed session.
class ArticleQuizController extends ChangeNotifier {
  ArticleQuizController(this.repository, this.articleId, this.publication);
  final ArticleQuizRepository repository;
  final String articleId;
  final QuizPublication publication;
  QuizPhase phase = QuizPhase.introduction;
  QuizFailure? failure;
  PublishedQuiz? quiz;
  QuizAnswerFeedback? feedback;
  String? selected;
  int index = 0, attempted = 0, correct = 0;
  bool busy = false, _disposed = false;
  int _generation = 0;
  int get total => quiz?.questions.length ?? publication.questionCount;
  int get incorrect => attempted - correct;
  int get percentage => total == 0 ? 0 : attempted * 100 ~/ total;
  QuizQuestion get question => quiz!.questions[index];
  void _notify() {
    if (!_disposed) notifyListeners();
  }

  void _clear() {
    quiz = null;
    feedback = null;
    selected = null;
    index = 0;
    attempted = 0;
    correct = 0;
  }

  void _error(Object error, QuizFailure fallback) {
    failure = fallback;
    if (error is ApiException) {
      if (error.status == 404 || error.status == 409) {
        _clear();
        phase = error.status == 409 ? QuizPhase.changed : QuizPhase.unavailable;
        failure = null;
      }
      if (error.status == 401 || error.status == 403) {
        failure = QuizFailure.session;
      }
      if (error.status == 429) failure = QuizFailure.rateLimited;
    }
  }

  Future<void> start() async {
    if (busy || _disposed) return;
    final generation = ++_generation;
    _clear();
    busy = true;
    failure = null;
    phase = QuizPhase.loading;
    _notify();
    try {
      final loaded = await repository.read(articleId);
      if (_disposed || generation != _generation) return;
      if (loaded.articleId != articleId ||
          loaded.revisionId != publication.revisionId ||
          loaded.questions.length != publication.questionCount) {
        phase = QuizPhase.changed;
      } else {
        quiz = loaded;
        phase = QuizPhase.question;
      }
    } catch (error) {
      if (_disposed || generation != _generation) return;
      phase = QuizPhase.introduction;
      _error(error, QuizFailure.load);
    } finally {
      if (!_disposed && generation == _generation) {
        busy = false;
        _notify();
      }
    }
  }

  void select(String id) {
    if (busy ||
        phase != QuizPhase.question ||
        feedback != null ||
        !question.options.any((o) => o.id == id)) {
      return;
    }
    selected = id;
    failure = null;
    _notify();
  }

  Future<void> submit() async {
    if (_disposed ||
        busy ||
        phase != QuizPhase.question ||
        selected == null ||
        feedback != null) {
      return;
    }
    final generation = _generation, current = question;
    busy = true;
    failure = null;
    _notify();
    try {
      final result = await repository.check(
          articleId, quiz!.revisionId, current.id, selected!);
      if (_disposed || generation != _generation) return;
      if (result.articleId != articleId ||
          result.revisionId != quiz!.revisionId ||
          result.questionId != current.id ||
          !current.options.any((o) => o.id == result.correctOptionId)) {
        _clear();
        phase = QuizPhase.changed;
      } else {
        feedback = result;
        attempted++;
        if (result.correct) correct++;
      }
    } catch (error) {
      if (!_disposed && generation == _generation) {
        _error(error, QuizFailure.answer);
      }
    } finally {
      if (!_disposed && generation == _generation) {
        busy = false;
        _notify();
      }
    }
  }

  void next() {
    if (busy || phase != QuizPhase.question || feedback == null) return;
    if (index + 1 == total) {
      phase = QuizPhase.complete;
    } else {
      index++;
      feedback = null;
      selected = null;
      failure = null;
    }
    _notify();
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    _clear();
    super.dispose();
  }
}
