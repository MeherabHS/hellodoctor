import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_repository.dart';

const _answeredJson = {
  'id': 'q-1',
  'question_text': 'Is a 100.4F fever with body pain normal on day 2?',
  'category': 'Fevers & Infections',
  'created_at': '2026-10-10T08:00:00Z',
  'answer_text': 'Continue oral hydration and paracetamol.',
  'answered_at': '2026-10-10T09:00:00Z',
  'answering_doctor': 'Dr. Sabrina Akter',
  'answering_doctor_specialty': 'Internal Medicine',
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `vault_cubit_test.dart`'s harness).
class FakeQnaRepository extends QnaRepository {
  FakeQnaRepository() : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  List<AnsweredQuestionDto>? feedResult;
  Object? feedError;
  Object? submitError;
  String? lastSubmittedText, lastCategory, lastProfileId;

  @override
  Future<List<AnsweredQuestionDto>> feed({int? limit, int? offset}) async {
    if (feedError != null) throw feedError!;
    return feedResult ?? const [];
  }

  @override
  Future<AnsweredQuestionDto> submit({
    required String questionText,
    String? category,
    String? profileId,
  }) async {
    lastSubmittedText = questionText;
    lastCategory = category;
    lastProfileId = profileId;
    if (submitError != null) throw submitError!;
    return AnsweredQuestionDto.fromJson({
      'id': 'created-1',
      'question_text': questionText,
      'category': category,
      'status': 'OPEN',
      'created_at': '2026-10-14T08:00:00Z',
    });
  }
}

class _NoopStore implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

void main() {
  late FakeQnaRepository repo;
  late QnaCubit cubit;

  setUp(() {
    repo = FakeQnaRepository();
    cubit = QnaCubit(repo);
  });

  tearDown(() => cubit.close());

  group('load', () {
    test('populates the answered feed on success', () async {
      repo.feedResult = [AnsweredQuestionDto.fromJson(_answeredJson)];
      await cubit.load();
      expect(cubit.state.questions, hasLength(1));
      expect(cubit.state.error, isNull);
      expect(cubit.state.questions.first.answeringDoctor, 'Dr. Sabrina Akter');
    });

    test('surfaces a repository failure as state.error', () async {
      repo.feedError = const ApiException(500, 'Server error.');
      await cubit.load();
      expect(cubit.state.error, isNotNull);
      expect(cubit.state.questions, isEmpty);
    });
  });

  group('submit', () {
    test('forwards text/category/profileId and marks submitted on success',
        () async {
      final ok = await cubit.submit(
          questionText: 'A question',
          category: 'General Health',
          profileId: 'dep-1');
      expect(ok, isTrue);
      expect(cubit.state.submitted, isTrue);
      expect(cubit.state.submitError, isNull);
      expect(repo.lastSubmittedText, 'A question');
      expect(repo.lastCategory, 'General Health');
      expect(repo.lastProfileId, 'dep-1');
    });

    test('surfaces a repository failure as state.submitError', () async {
      repo.submitError = const ApiException(422, 'Invalid question.');
      final ok = await cubit.submit(questionText: 'A question');
      expect(ok, isFalse);
      expect(cubit.state.submitted, isFalse);
      expect(cubit.state.submitError, isNotNull);
    });

    test('resetSubmitState clears submitted and submitError', () async {
      await cubit.submit(questionText: 'A question');
      expect(cubit.state.submitted, isTrue);
      cubit.resetSubmitState();
      expect(cubit.state.submitted, isFalse);
      expect(cubit.state.submitError, isNull);
    });
  });
}
