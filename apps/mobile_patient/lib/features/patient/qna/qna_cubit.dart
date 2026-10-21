import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/network/api_client.dart';
import 'qna_models.dart';
import 'qna_repository.dart';

const _unset = Object();

class QnaState {
  const QnaState({
    this.loading = false,
    this.error,
    this.questions = const [],
    this.submitting = false,
    this.submitError,
    this.submitted = false,
  });
  final bool loading, submitting, submitted;
  final String? error, submitError;
  final List<AnsweredQuestionDto> questions;

  QnaState copyWith({
    bool? loading,
    Object? error = _unset,
    List<AnsweredQuestionDto>? questions,
    bool? submitting,
    Object? submitError = _unset,
    bool? submitted,
  }) =>
      QnaState(
        loading: loading ?? this.loading,
        error: identical(error, _unset) ? this.error : error as String?,
        questions: questions ?? this.questions,
        submitting: submitting ?? this.submitting,
        submitError: identical(submitError, _unset)
            ? this.submitError
            : submitError as String?,
        submitted: submitted ?? this.submitted,
      );
}

/// Owns the Free Health Q&A answered feed (plan's gap #6, M8) and the
/// submit-a-question flow. The feed itself is global, not profile-scoped
/// (any patient may read any doctor-answered question) — only submitting one
/// is profile-scoped, via the optional `profileId` passed to [submit].
class QnaCubit extends Cubit<QnaState> {
  QnaCubit(this.repo) : super(const QnaState());
  final QnaRepository repo;

  Future<void> load() async {
    emit(state.copyWith(loading: true, error: null));
    try {
      final questions = await repo.feed();
      if (isClosed) return;
      emit(state.copyWith(loading: false, error: null, questions: questions));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        error:
            e is ApiException ? e.userMessage : 'Unable to load the Q&A feed.',
      ));
    }
  }

  Future<bool> submit({
    required String questionText,
    String? category,
    String? profileId,
  }) async {
    if (state.submitting) return false;
    final text = questionText.trim();
    if (text.isEmpty || text.runes.length > 4000) {
      emit(state.copyWith(
          submitError: 'Enter a question with 1 to 4,000 characters.',
          submitted: false));
      return false;
    }
    emit(state.copyWith(submitting: true, submitError: null, submitted: false));
    try {
      final created = await repo.submit(
          questionText: text, category: category, profileId: profileId);
      if (isClosed) return true;
      emit(state.copyWith(submitting: false, submitted: true, questions: [
        created,
        ...state.questions.where((q) => q.id != created.id)
      ]));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
        submitting: false,
        submitError: e is ApiException
            ? e.userMessage
            : 'Unable to submit this question.',
      ));
      return false;
    }
  }

  void resetSubmitState() =>
      emit(state.copyWith(submitted: false, submitError: null));
}
