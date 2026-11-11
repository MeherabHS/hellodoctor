import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/article_quiz_section.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blog_detail_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/quiz_controller.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/quiz_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/quiz_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

String id(int n) => '00000000-0000-4000-8000-${n.toString().padLeft(12, '0')}';
Map<String, dynamic> quizJson({bool translated = true}) => {
      'schema_version': 1,
      'article_id': id(1),
      'revision_id': id(2),
      'included': true,
      'question_count': 2,
      'questions': [
        for (var i = 0; i < 2; i++)
          {
            'id': id(10 + i),
            'text_en': 'Question ${i + 1}?',
            'text_sw': translated ? 'Swali ${i + 1}?' : '',
            'options': [
              {
                'id': id(20 + i * 2),
                'text_en': 'First',
                'text_sw': translated ? 'Kwanza' : ''
              },
              {
                'id': id(21 + i * 2),
                'text_en': 'Second',
                'text_sw': translated ? 'Pili' : ''
              }
            ]
          }
      ]
    };
QuizAnswerFeedback feedback(String question, String selected,
        {bool translated = true, bool? correct}) =>
    QuizAnswerFeedback.fromJson({
      'schema_version': 1,
      'article_id': id(1),
      'revision_id': id(2),
      'question_id': question,
      'correct': correct ?? (selected == id(20) || selected == id(22)),
      'correct_option_id': question == id(10) ? id(20) : id(22),
      'explanation_en': 'Educational explanation',
      'explanation_sw': translated ? 'Maelezo ya kujifunza' : ''
    });

class MemoryTokens implements TokenStore {
  final writes = <String>[];
  @override
  Future<String?> read(String key) async =>
      key == 'access_token' ? 'fixture-token' : null;
  @override
  Future<void> write(String key, String value) async {
    writes.add(key);
  }

  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

ApiClient fixtureApi([Dio? dio, MemoryTokens? tokens]) => ApiClient(
    dio: dio ?? Dio(BaseOptions(baseUrl: 'http://127.0.0.1:8080/api/v1')),
    tokenStore: tokens ?? MemoryTokens());

class FakeQuizRepo extends ArticleQuizRepository {
  FakeQuizRepo() : super(fixtureApi());
  int reads = 0, checks = 0;
  bool translated = true;
  Object? readError, checkError;
  Completer<PublishedQuiz>? pendingRead;
  Completer<QuizAnswerFeedback>? pendingCheck;
  @override
  Future<PublishedQuiz> read(String articleId) async {
    reads++;
    if (readError != null) throw readError!;
    if (pendingRead != null) return pendingRead!.future;
    return PublishedQuiz.fromJson(quizJson(translated: translated));
  }

  @override
  Future<QuizAnswerFeedback> check(String articleId, String revisionId,
      String questionId, String optionId) async {
    checks++;
    if (checkError != null) throw checkError!;
    if (pendingCheck != null) return pendingCheck!.future;
    return feedback(questionId, optionId, translated: translated);
  }
}

class QuizAdapter implements HttpClientAdapter {
  final requests = <RequestOptions>[];
  @override
  Future<ResponseBody> fetch(RequestOptions options,
      Stream<Uint8List>? requestStream, Future<dynamic>? cancelFuture) async {
    requests.add(options);
    final data = options.method == 'GET'
        ? quizJson()
        : {
            'schema_version': 1,
            'article_id': id(1),
            'revision_id': id(2),
            'question_id': id(10),
            'correct': true,
            'correct_option_id': id(20),
            'explanation_en': 'Explanation',
            'explanation_sw': ''
          };
    return ResponseBody.fromString(
        jsonEncode({'success': true, 'data': data}), 200,
        headers: {
          Headers.contentTypeHeader: [Headers.jsonContentType]
        });
  }

  @override
  void close({bool force = false}) {}
}

class QuizBlogs extends BlogsRepository {
  QuizBlogs(this.quizRepo, {this.admin = true, this.included = true})
      : super(fixtureApi(), quizzes: quizRepo);
  final FakeQuizRepo quizRepo;
  final bool admin, included;
  int loads = 0;
  @override
  Future<List<BlogArticleDto>> feed({int? limit, int? offset}) async =>
      [await detail(id(1))];
  @override
  Future<BlogPage> discover(
          {String search = '',
          String language = 'en',
          BlogCursor? cursor,
          int limit = 20}) async =>
      BlogPage([await detail(id(1))], null);
  @override
  Future<BlogArticleDto> detail(String articleId) async {
    loads++;
    return BlogArticleDto.fromJson({
      'id': id(1),
      'article_type': admin ? 'ADMIN' : 'DOCTOR',
      'status': 'PUBLISHED',
      'title': 'Article title',
      'summary': 'Doctor summary',
      'body': 'Article body',
      'author_name': 'Dr. Fixture',
      'specialty': 'General practice',
      'quiz': {
        'schema_version': 1,
        'included': included,
        'revision_id': id(2),
        'question_count': 2
      }
    });
  }
}

Widget host(Widget child, {String language = 'en', double scale = 1}) =>
    MaterialApp(
        locale: Locale(language),
        theme: PatientTokens.theme(),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context).copyWith(
                disableAnimations: true, textScaler: TextScaler.linear(scale)),
            child: child!),
        home: child);
Widget section(FakeQuizRepo repo, {VoidCallback? refresh}) => Scaffold(
    body: SingleChildScrollView(
        child: ArticleQuizSection(
            articleId: id(1),
            publication: QuizPublication(id(2), 2),
            repository: repo,
            onRefreshArticle: refresh ?? () {})));
Future<void> tap(WidgetTester tester, String text) async {
  final finder = find.text(text);
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

void main() {
  test(
      'article metadata is optional, versioned and ignored for Doctor articles',
      () {
    final data = {
      'article_type': 'ADMIN',
      'status': 'PUBLISHED',
      'quiz': {
        'schema_version': 1,
        'included': true,
        'revision_id': id(2),
        'question_count': 2
      }
    };
    expect(BlogArticleDto.fromJson(data).quiz?.questionCount, 2);
    data['article_type'] = 'DOCTOR';
    expect(BlogArticleDto.fromJson(data).quiz, isNull);
    for (final value in [
      null,
      {
        'schema_version': 2,
        'included': true,
        'revision_id': id(2),
        'question_count': 2
      },
      {
        'schema_version': 1,
        'included': false,
        'revision_id': id(2),
        'question_count': 2
      },
      {
        'schema_version': 1,
        'included': true,
        'revision_id': 'invalid',
        'question_count': 2
      }
    ]) {
      expect(QuizPublication.parse(value), isNull);
    }
  });
  test(
      'published DTO preserves question/option order and rejects leaked keys or invalid counts',
      () {
    final quiz = PublishedQuiz.fromJson(quizJson());
    expect(quiz.questions.map((q) => q.id), [id(10), id(11)]);
    expect(quiz.questions[0].options.map((o) => o.id), [id(20), id(21)]);
    final raw = quizJson();
    (raw['questions'] as List)[0]['correct_option_id'] = id(20);
    expect(() => PublishedQuiz.fromJson(raw), throwsFormatException);
    final empty = quizJson()..['questions'] = [];
    expect(() => PublishedQuiz.fromJson(empty), throwsFormatException);
  });
  test('question and feedback use complete English fallback', () {
    final quiz = PublishedQuiz.fromJson(quizJson(translated: false));
    expect(quiz.questions[0].textFor('sw'), 'Question 1?');
    expect(quiz.questions[0].optionFor(quiz.questions[0].options[0], 'sw'),
        'First');
    expect(feedback(id(10), id(20), translated: false).explanationFor('sw'),
        'Educational explanation');
  });
  test(
      'repository uses canonical authenticated transport and never persists selections',
      () async {
    final adapter = QuizAdapter(), tokens = MemoryTokens();
    final dio = Dio(BaseOptions(baseUrl: 'http://127.0.0.1:8080/api/v1'))
      ..httpClientAdapter = adapter;
    final api = fixtureApi(dio, tokens);
    var incidents = 0;
    api.onUnexpectedError = (_, __) => incidents++;
    final repo = ArticleQuizRepository(api);
    await repo.read(id(1));
    await repo.check(id(1), id(2), id(10), id(20));
    expect(adapter.requests.map((r) => r.path),
        ['/articles/${id(1)}/quiz', '/articles/${id(1)}/quiz/check']);
    expect(
        adapter.requests
            .every((r) => r.headers['Authorization'] == 'Bearer fixture-token'),
        true);
    expect(adapter.requests.last.data, {
      'revision_id': id(2),
      'question_id': id(10),
      'selected_option_id': id(20)
    });
    expect(adapter.requests.last.headers.containsKey('Idempotency-Key'), false);
    expect(tokens.writes, isEmpty);
    expect(incidents, 0);
  });
  test(
      'local session scores feedback once and prevents accidental duplicate submissions',
      () async {
    final repo = FakeQuizRepo()..pendingCheck = Completer<QuizAnswerFeedback>();
    final c = ArticleQuizController(repo, id(1), QuizPublication(id(2), 2));
    await c.start();
    c.select(id(20));
    final pending = c.submit();
    await c.submit();
    expect(repo.checks, 1);
    repo.pendingCheck!.complete(feedback(id(10), id(20), correct: false));
    await pending;
    expect(c.correct, 0);
    expect(c.incorrect, 1);
    expect(c.attempted, 1);
    expect(c.percentage, 50);
    await c.submit();
    expect(repo.checks, 1);
    c.dispose();
  });
  test('restart and disposal clear memory without storing a learning profile',
      () async {
    final repo = FakeQuizRepo();
    final c = ArticleQuizController(repo, id(1), QuizPublication(id(2), 2));
    await c.start();
    c.select(id(20));
    await c.submit();
    c.next();
    c.select(id(22));
    await c.submit();
    c.next();
    expect(c.phase, QuizPhase.complete);
    expect(c.correct, 2);
    await c.start();
    expect(c.attempted, 0);
    expect(c.selected, isNull);
    expect(repo.reads, 2);
    c.dispose();
    expect(c.quiz, isNull);
  });
  test('disposed controller ignores delayed feedback', () async {
    final repo = FakeQuizRepo()..pendingCheck = Completer<QuizAnswerFeedback>();
    final c = ArticleQuizController(repo, id(1), QuizPublication(id(2), 2));
    await c.start();
    c.select(id(20));
    final pending = c.submit();
    c.dispose();
    repo.pendingCheck!.complete(feedback(id(10), id(20)));
    await pending;
    expect(c.quiz, isNull);
    expect(c.attempted, 0);
  });
  test('superseded or unavailable responses clear questions and local score',
      () async {
    for (final status in [404, 409]) {
      final repo = FakeQuizRepo()..checkError = ApiException(status, '');
      final c = ArticleQuizController(repo, id(1), QuizPublication(id(2), 2));
      await c.start();
      c.select(id(20));
      await c.submit();
      expect(
          c.phase, status == 409 ? QuizPhase.changed : QuizPhase.unavailable);
      expect(c.quiz, isNull);
      expect(c.attempted, 0);
      c.dispose();
    }
  });
  test('mismatched read revision never appears below a stale article',
      () async {
    final repo = FakeQuizRepo();
    final c = ArticleQuizController(repo, id(1), QuizPublication(id(99), 2));
    await c.start();
    expect(c.phase, QuizPhase.changed);
    expect(c.quiz, isNull);
    c.dispose();
  });
  testWidgets(
      'published Admin quiz appears below the article and completes/restarts',
      (tester) async {
    final repo = FakeQuizRepo();
    await tester.pumpWidget(host(BlogsPage(repo: QuizBlogs(repo))));
    await tester.pumpAndSettle();
    await tap(tester, 'Article title');
    expect(find.text('Article body'), findsOneWidget);
    expect(find.text('Start quiz'), findsOneWidget);
    expect(tester.getTopLeft(find.byType(ArticleQuizSection)).dy,
        greaterThan(tester.getBottomLeft(find.text('Article body')).dy));
    expect(repo.reads, 0);
    await tap(tester, 'Start quiz');
    expect(find.text('Question 1?'), findsOneWidget);
    expect(find.text('Educational explanation'), findsNothing);
    await tap(tester, 'B. Second');
    expect(
        find.byWidgetPredicate(
            (w) => w is Semantics && w.properties.selected == true),
        findsOneWidget);
    await tap(tester, 'Submit answer');
    expect(find.text('Not quite'), findsOneWidget);
    expect(find.text('Educational explanation'), findsOneWidget);
    await tap(tester, 'Next question');
    expect(find.text('Question 2 of 2'), findsOneWidget);
    await tap(tester, 'A. First');
    await tap(tester, 'Submit answer');
    expect(find.text('Correct'), findsOneWidget);
    await tap(tester, 'See summary');
    expect(find.text('Quiz complete'), findsOneWidget);
    expect(find.text('Correct answers: 1'), findsOneWidget);
    expect(find.text('Incorrect answers: 1'), findsOneWidget);
    expect(find.text('Completion: 100%'), findsOneWidget);
    await tap(tester, 'Restart quiz');
    expect(find.text('Question 1?'), findsOneWidget);
    expect(find.text('Educational explanation'), findsNothing);
    expect(repo.reads, 2);
  });
  testWidgets(
      'Doctor articles and inclusion OFF render normally without fetching quizzes',
      (tester) async {
    for (final admin in [false, true]) {
      final repo = FakeQuizRepo();
      await tester.pumpWidget(host(BlogDetailPage(
          key: ValueKey(admin),
          articleId: id(1),
          repo: QuizBlogs(repo, admin: admin, included: false))));
      await tester.pumpAndSettle();
      expect(find.text('Article body'), findsOneWidget);
      expect(find.byType(ArticleQuizSection), findsNothing);
      expect(repo.reads, 0);
      if (!admin) expect(find.text('Doctor summary'), findsOneWidget);
    }
  });
  testWidgets(
      'language changes translate current question and feedback without resubmission',
      (tester) async {
    final repo = FakeQuizRepo();
    await tester.pumpWidget(host(section(repo)));
    await tap(tester, 'Start quiz');
    await tap(tester, 'A. First');
    await tap(tester, 'Submit answer');
    await tester.pumpWidget(host(section(repo), language: 'sw'));
    await tester.pumpAndSettle();
    expect(find.text('Swali 1?'), findsOneWidget);
    expect(find.text('Maelezo ya kujifunza'), findsOneWidget);
    expect(repo.checks, 1);
    expect(repo.reads, 1);
  });
  testWidgets('Kiswahili locale gets full English content fallback',
      (tester) async {
    final repo = FakeQuizRepo()..translated = false;
    await tester.pumpWidget(host(section(repo), language: 'sw'));
    await tap(tester, 'Anza maswali');
    expect(find.text('Question 1?'), findsOneWidget);
    expect(find.text('A. First'), findsOneWidget);
    await tap(tester, 'A. First');
    await tap(tester, 'Wasilisha jibu');
    expect(find.text('Educational explanation'), findsOneWidget);
  });
  testWidgets('loading respects reduced motion and network errors offer retry',
      (tester) async {
    final repo = FakeQuizRepo()..pendingRead = Completer<PublishedQuiz>();
    await tester.pumpWidget(host(section(repo)));
    await tester.tap(find.text('Start quiz'));
    await tester.pump();
    expect(find.text('Loading…'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsNothing);
    repo.pendingRead!.completeError(const ApiException(null, ''));
    await tester.pumpAndSettle();
    expect(find.text('The quiz could not be loaded. Please retry.'),
        findsOneWidget);
    repo.pendingRead = null;
    await tap(tester, 'Retry');
    expect(find.text('Question 1?'), findsOneWidget);
  });
  testWidgets(
      'answer errors preserve selection and can retry without counting twice',
      (tester) async {
    final repo = FakeQuizRepo()..checkError = const ApiException(503, '');
    await tester.pumpWidget(host(section(repo)));
    await tap(tester, 'Start quiz');
    await tap(tester, 'A. First');
    await tap(tester, 'Submit answer');
    expect(find.text('Your answer could not be checked. Please retry.'),
        findsOneWidget);
    repo.checkError = null;
    await tap(tester, 'Submit answer');
    expect(find.text('Correct'), findsOneWidget);
    expect(repo.checks, 2);
  });
  testWidgets(
      'superseded quiz hides stale questions and refreshes the whole article',
      (tester) async {
    final repo = FakeQuizRepo()..checkError = const ApiException(409, '');
    var refreshes = 0;
    await tester.pumpWidget(host(section(repo, refresh: () => refreshes++)));
    await tap(tester, 'Start quiz');
    await tap(tester, 'A. First');
    await tap(tester, 'Submit answer');
    expect(find.text('Question 1?'), findsNothing);
    expect(
        find.text(
            'The article or quiz has changed. Refresh the article to start the current quiz.'),
        findsOneWidget);
    await tap(tester, 'Refresh article');
    expect(refreshes, 1);
  });
  testWidgets(
      'empty/malformed quiz response has a retry state rather than an empty assessment',
      (tester) async {
    final repo = FakeQuizRepo()
      ..readError = const FormatException('Invalid quiz response.');
    await tester.pumpWidget(host(section(repo)));
    await tap(tester, 'Start quiz');
    expect(find.text('Retry'), findsOneWidget);
    expect(find.text('The quiz could not be loaded. Please retry.'),
        findsOneWidget);
  });
  testWidgets(
      'large text and narrow layouts keep answers readable without overflow',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(320, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = FakeQuizRepo();
    await tester.pumpWidget(host(section(repo), scale: 2));
    await tap(tester, 'Start quiz');
    expect(find.text('Question 1?'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
