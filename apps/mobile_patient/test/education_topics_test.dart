import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blog_detail_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/education/education_tabs.dart';
import 'package:hellodoctor_mobile_patient/features/patient/education/education_topics.dart';
import 'package:hellodoctor_mobile_patient/features/patient/education/education_widgets.dart';
import 'package:hellodoctor_mobile_patient/features/patient/education/topic_articles_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/modules/patient_modules.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class _Tokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

Map<String, dynamic> _admin(String id, String title,
        {String? topic,
        String? video,
        Map<String, dynamic>? quiz,
        bool standalone = false}) =>
    {
      'id': id,
      'title': title,
      'summary': '',
      'body':
          '## What is it?\n\n${List.filled(450, 'word').join(' ')}\n\n- Tip one',
      'article_type': 'ADMIN',
      'content_format': 'markdown',
      'category_name_en': 'Category',
      'status': 'PUBLISHED',
      'published_at': '2026-11-09T22:30:00Z', // Nov 10 in Kenya (UTC+3)
      if (topic != null) 'topic_key': topic,
      if (video != null) 'video_url': video,
      if (quiz != null) 'quiz': quiz,
      if (standalone) 'standalone_quiz': true,
    };

class _Repo extends BlogsRepository {
  _Repo(this.rows)
      : super(ApiClient(
            dio: Dio(BaseOptions(baseUrl: 'http://127.0.0.1:1/api/v1')),
            tokenStore: _Tokens()));
  final List<BlogArticleDto> rows;
  final calls = <(String?, bool)>[];
  int quizCalls = 0;
  @override
  Future<BlogPage> discover(
      {String search = '',
      String language = 'en',
      BlogCursor? cursor,
      int limit = 20,
      String? topic,
      bool videos = false,
      bool standaloneQuizzes = false}) async {
    if (standaloneQuizzes) {
      quizCalls++;
      return BlogPage(rows.where((a) => a.standaloneQuiz).toList(), null);
    }
    calls.add((topic, videos));
    return BlogPage(rows.where((a) => !a.standaloneQuiz).toList(), null);
  }

  @override
  Future<BlogArticleDto> detail(String id) async =>
      rows.firstWhere((a) => a.id == id);
}

const _nav = PatientShellNav(
    openServices: _noop, openMessages: _noop, openProfile: _noop, select: _sel);
void _noop() {}
void _sel(int _) {}

Widget _host(Widget child, {String language = 'en'}) => MaterialApp(
    locale: Locale(language),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: child);

void main() {
  const id1 = '11111111-1111-4111-8111-111111111111';
  const id2 = '22222222-2222-4222-8222-222222222222';

  group('model', () {
    test('reads topic and only the canonical YouTube link', () {
      final v = BlogArticleDto.fromJson(_admin(id1, 'V',
          topic: 'PREP', video: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ'));
      expect(v.topicKey, 'PREP');
      expect(v.isVideo, isTrue);
      for (final bad in [
        'https://evil.example/watch?v=dQw4w9WgXcQ',
        'https://youtu.be/dQw4w9WgXcQ',
        'javascript:alert(1)',
      ]) {
        expect(BlogArticleDto.fromJson(_admin(id1, 'V', video: bad)).isVideo,
            isFalse,
            reason: bad);
      }
    });

    test('reading time is words / 200, at least one minute', () {
      final a = BlogArticleDto.fromJson(_admin(id1, 'A'));
      expect(a.readMinutesFor('en'), 3); // ~455 words
    });

    test('every topic maps to its backend key and has artwork', () {
      expect(EducationTopic.values.map((t) => t.apiKey), [
        'HIV_BASICS',
        'PREP',
        'CONDOMS',
        'SRH',
        'STIGMA_DISCLOSURE',
        'HEALTHY_RELATIONSHIPS'
      ]);
      expect(EducationTopic.fromApiKey('SRH'), EducationTopic.srh);
      expect(EducationTopic.fromApiKey('nope'), isNull);
    });
  });

  group('topic page', () {
    for (final topic in EducationTopic.values) {
      testWidgets('${topic.name}: empty state uses the topic\'s own art',
          (tester) async {
        final repo = _Repo([]);
        await tester.pumpWidget(
            _host(TopicArticlesPage(topic: topic, repo: repo, nav: _nav)));
        await tester.pumpAndSettle();
        expect(repo.calls.single, (topic.apiKey, false));
        expect(find.text('Articles (0)'), findsOneWidget);
        expect(find.text('No articles yet'), findsOneWidget);
        final images = tester
            .widgetList<Image>(find.byType(Image))
            .map((i) => (i.image as AssetImage).assetName);
        expect(images, containsAll([topic.bannerAsset, topic.emptyAsset]));
        expect(tester.takeException(), isNull);
      });
    }

    testWidgets(
        'populated: count, rows with EAT date and read time; opens reader',
        (tester) async {
      final repo = _Repo([
        BlogArticleDto.fromJson(_admin(
            id1, 'Understanding HIV and Its Treatment',
            topic: 'HIV_BASICS')),
        BlogArticleDto.fromJson(
            _admin(id2, 'How HIV Spreads', topic: 'HIV_BASICS')),
      ]);
      await tester.pumpWidget(_host(TopicArticlesPage(
          topic: EducationTopic.hivBasics, repo: repo, nav: _nav)));
      await tester.pumpAndSettle();
      expect(find.text('Articles (2)'), findsOneWidget);
      expect(find.text('No articles yet'), findsNothing);
      expect(find.text('Nov 10, 2026'), findsNWidgets(2));
      expect(find.text('3 min read'), findsNWidgets(2));
      await tester.tap(find.text('How HIV Spreads'));
      await tester.pumpAndSettle();
      expect(find.byType(BlogDetailPage), findsOneWidget);
      // Topic chip, no bookmark or share controls.
      expect(find.text('HIV Basics'), findsOneWidget);
      expect(find.byIcon(Icons.bookmark_border), findsNothing);
      expect(find.byIcon(Icons.share), findsNothing);
      expect(find.byIcon(Icons.check_circle_rounded), findsWidgets);
    });

    testWidgets('Swahili and large text do not overflow', (tester) async {
      tester.view.physicalSize = const Size(360, 780);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      for (final topic in EducationTopic.values) {
        await tester.pumpWidget(MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(1.3)),
            child: _host(
                TopicArticlesPage(topic: topic, repo: _Repo([]), nav: _nav),
                language: 'sw')));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull, reason: topic.name);
      }
    });
  });

  group('hub tabs', () {
    testWidgets('Videos lists server videos; empty state otherwise',
        (tester) async {
      final repo = _Repo([
        BlogArticleDto.fromJson(_admin(id1, 'PrEP explained',
            topic: 'PREP',
            video: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ')),
      ]);
      await tester.pumpWidget(_host(Scaffold(
          body: SingleChildScrollView(
              child: EduTabFeed(kind: EduTabKind.videos, repo: repo)))));
      await tester.pumpAndSettle();
      expect(repo.calls.single, (null, true));
      expect(find.byType(EduVideoRow), findsOneWidget);
      expect(find.text('Watch on YouTube'), findsOneWidget);

      await tester.pumpWidget(_host(Scaffold(
          body: SingleChildScrollView(
              child: EduTabFeed(
                  key: const ValueKey('e'),
                  kind: EduTabKind.videos,
                  repo: _Repo([]))))));
      await tester.pumpAndSettle();
      expect(find.text('No videos yet'), findsOneWidget);
    });

    testWidgets('Quizzes lists only articles that carry a quiz',
        (tester) async {
      final repo = _Repo([
        BlogArticleDto.fromJson(_admin(id1, 'With quiz', quiz: {
          'schema_version': 1,
          'included': true,
          'revision_id': '33333333-3333-4333-8333-333333333333',
          'question_count': 4
        })),
        BlogArticleDto.fromJson(_admin(id2, 'No quiz')),
      ]);
      await tester.pumpWidget(_host(Scaffold(
          body: SingleChildScrollView(
              child: EduTabFeed(kind: EduTabKind.quizzes, repo: repo)))));
      await tester.pumpAndSettle();
      expect(find.text('With quiz'), findsOneWidget);
      expect(find.text('No quiz'), findsNothing);
      expect(find.text('4 questions'), findsOneWidget);
    });

    testWidgets('Quizzes lists standalone quizzes first, then article quizzes',
        (tester) async {
      const q = {
        'schema_version': 1,
        'included': true,
        'revision_id': '44444444-4444-4444-8444-444444444444',
        'question_count': 5
      };
      final repo = _Repo([
        BlogArticleDto.fromJson(_admin(id1, 'Article with quiz', quiz: q)),
        BlogArticleDto.fromJson(_admin(
            '55555555-5555-4555-8555-555555555555', 'PrEP knowledge check',
            topic: 'PREP', quiz: q, standalone: true)),
      ]);
      await tester.pumpWidget(_host(Scaffold(
          body: SingleChildScrollView(
              child: EduTabFeed(kind: EduTabKind.quizzes, repo: repo)))));
      await tester.pumpAndSettle();
      expect(repo.quizCalls, 1);
      final standaloneY =
          tester.getTopLeft(find.text('PrEP knowledge check')).dy;
      final articleY = tester.getTopLeft(find.text('Article with quiz')).dy;
      expect(standaloneY, lessThan(articleY));
      expect(find.text('5 questions'), findsNWidgets(2));
    });

    testWidgets('Audio shows its empty state', (tester) async {
      await tester.pumpWidget(_host(
          const Scaffold(body: SingleChildScrollView(child: EduAudioEmpty()))));
      expect(find.text('No audio yet'), findsOneWidget);
    });
  });

  test('EduColors are the sampled reference colours', () {
    expect(EduColors.sectionGreen, const Color(0xFF0F7B2B));
  });
}
