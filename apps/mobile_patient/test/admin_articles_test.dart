import 'dart:typed_data';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/article_content.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blog_detail_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

const articleId = 'e0f20dc7-61b7-432e-805c-ded77ab2ee78';
Map<String, dynamic> doctorJson() => {
      'id': 'doctor-existing',
      'title': 'Doctor guide',
      'summary': 'Physician summary',
      'body': 'Doctor **plain** body',
      'author_name': 'Dr. Test',
      'specialty': 'General practice',
      'references_list': ['Reference'],
      'status': 'PUBLISHED'
    };
Map<String, dynamic> adminJson() => {
      'id': articleId,
      'title': 'Editorial guide',
      'summary': 'Guidance',
      'body': '## Guidance\n\nA **strong** paragraph.\n\n- One\n- Two',
      'title_sw': 'Mwongozo',
      'body_sw': '## Afya\n\nSoma kwa makini.',
      'author_name': 'Untrusted doctor title',
      'specialty': 'Untrusted specialty',
      'article_type': 'ADMIN',
      'content_format': 'markdown',
      'category_name_en': 'Health',
      'category_name_sw': 'Afya',
      'status': 'PUBLISHED',
      'published_at': '2026-10-25T08:00:00Z'
    };

class Tokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

class BlogApi extends ApiClient {
  BlogApi()
      : super(
            dio: Dio(BaseOptions(baseUrl: 'http://127.0.0.1:8080/api/v1')),
            tokenStore: Tokens());
  final paths = <String>[];
  dynamic response;
  @override
  Future<Response<dynamic>> get(String path,
      {Map<String, dynamic>? query}) async {
    paths.add(path);
    return Response(
        requestOptions: RequestOptions(path: path),
        data: {'success': true, 'data': response});
  }

  @override
  Future<List<int>> getBytes(String path, {Map<String, dynamic>? query}) async {
    paths.add(path);
    return Uint8List.fromList([1, 2, 3]);
  }
}

class BlogFixture extends BlogsRepository {
  BlogFixture() : super(BlogApi());
  final rows = [
    BlogArticleDto.fromJson(doctorJson()),
    BlogArticleDto.fromJson(adminJson())
  ];
  final opened = <String>[];
  @override
  Future<List<BlogArticleDto>> feed({int? limit, int? offset}) async => rows;
  @override
  Future<BlogPage> discover(
          {String search = '',
          String language = 'en',
          BlogCursor? cursor,
          int limit = 20}) async =>
      BlogPage(rows, null);
  @override
  Future<BlogArticleDto> detail(String id) async {
    opened.add(id);
    return rows.firstWhere((a) => a.id == id);
  }
}

Widget host(Widget child, {String language = 'en'}) => MaterialApp(
    locale: Locale(language),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: child);

void main() {
  test('existing Doctor DTO keeps identity, body and reference contract', () {
    final article = BlogArticleDto.fromJson(doctorJson());
    expect(article.isAdmin, false);
    expect(article.contentFormat, 'plain');
    expect(article.bodyFor('sw'), 'Doctor **plain** body');
    expect(article.authorName, 'Dr. Test');
    expect(article.referencesList, ['Reference']);
  });
  test('Admin DTO identifies Editorial publisher and supports bilingual fields',
      () {
    final article = BlogArticleDto.fromJson(adminJson());
    expect(article.isAdmin, true);
    expect(article.authorName, 'AfyaMkononi Editorial');
    expect(article.specialty, 'Editorial');
    expect(article.titleFor('sw'), 'Mwongozo');
    expect(article.bodyFor('sw'), contains('Afya'));
    expect(article.categoryFor('sw'), 'Afya');
  });
  test('incomplete Kiswahili translation falls back as a whole', () {
    final json = adminJson()..['body_sw'] = '';
    final article = BlogArticleDto.fromJson(json);
    expect(article.titleFor('sw'), 'Editorial guide');
    expect(article.bodyFor('sw'), json['body']);
  });
  test('draft and archived responses are rejected defensively', () {
    for (final status in [
      'DRAFT',
      'PENDING_REVIEW',
      'APPROVED',
      'SCHEDULED',
      'ARCHIVED'
    ]) {
      expect(() => BlogArticleDto.fromJson(adminJson()..['status'] = status),
          throwsFormatException);
    }
  });
  test('untrusted cover URL never enters the DTO media path', () {
    final article = BlogArticleDto.fromJson(adminJson()
      ..['cover_url'] = 'https://evil.example/track'
      ..['cover_image_id'] = '../../clinical');
    expect(article.hasCover, false);
  });
  test(
      'links reject active schemes, credential URLs, control characters and local paths',
      () {
    expect(safeArticleLink('https://example.org'), isNotNull);
    for (final url in [
      'javascript:alert(1)',
      'data:text/html,hi',
      'file:///secret',
      '//evil.example',
      'https://user:password@example.org',
      'https://example.org\n',
      'https:\\evil.example'
    ]) {
      expect(safeArticleLink(url), isNull, reason: url);
    }
  });
  test(
      'repository uses the existing feed/detail routes and bounded cover identity',
      () async {
    final api = BlogApi();
    final repo = BlogsRepository(api);
    api.response = [doctorJson(), adminJson()];
    expect((await repo.feed()).length, 2);
    api.response = adminJson();
    expect((await repo.detail(articleId)).isAdmin, true);
    await repo.cover(articleId);
    expect(api.paths,
        ['/articles', '/articles/$articleId', '/articles/$articleId/cover']);
    expect(() => repo.cover('../clinical'), throwsFormatException);
  });
  testWidgets('one Blogs tab displays both sources and opens Admin article',
      (tester) async {
    final repo = BlogFixture();
    await tester.pumpWidget(host(BlogsPage(repo: repo)));
    await tester.pumpAndSettle();
    expect(find.text('Doctor guide'), findsOneWidget);
    expect(find.text('Editorial guide'), findsOneWidget);
    expect(find.text('AfyaMkononi Editorial'), findsOneWidget);
    await tester.tap(find.text('Editorial guide'));
    await tester.pumpAndSettle();
    expect(repo.opened, [articleId]);
    expect(find.byType(BlogDetailPage), findsOneWidget);
    expect(find.text('Guidance', findRichText: true), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('Doctor detail keeps existing plain text and references',
      (tester) async {
    await tester.pumpWidget(host(
        BlogDetailPage(articleId: 'doctor-existing', repo: BlogFixture())));
    await tester.pumpAndSettle();
    expect(find.text('Doctor **plain** body'), findsOneWidget);
    expect(find.text('• Reference'), findsOneWidget);
    expect(find.byType(ArticleMarkdown), findsNothing);
  });
  testWidgets('Patient locale selects Kiswahili Admin detail', (tester) async {
    await tester.pumpWidget(host(
        BlogDetailPage(articleId: articleId, repo: BlogFixture()),
        language: 'sw'));
    await tester.pumpAndSettle();
    expect(find.text('Mwongozo'), findsOneWidget);
    expect(find.text('Soma kwa makini.', findRichText: true), findsOneWidget);
    expect(find.text('Editorial guide'), findsNothing);
  });
  testWidgets('Markdown cannot execute HTML or render embedded images',
      (tester) async {
    await tester.pumpWidget(host(const Scaffold(
        body: ArticleMarkdown(
            body:
                '## Safe\n\n<script>alert(1)</script>\n\n![track](https://tracking.example/x)\n\n![local](file:///secret)\n\n[bad](javascript:alert(1))'))));
    await tester.pumpAndSettle();
    expect(find.byType(Image), findsNothing);
    expect(find.text('Safe', findRichText: true), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('invalid cover bytes fail without breaking reading',
      (tester) async {
    await tester.pumpWidget(host(ArticleCover(
        articleId: articleId,
        mediaId: 'media',
        repo: BlogsRepository(BlogApi()))));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });
  testWidgets('Admin detail remains readable on narrow screens',
      (tester) async {
    tester.view.physicalSize = const Size(320, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
        host(BlogDetailPage(articleId: articleId, repo: BlogFixture())));
    await tester.pumpAndSettle();
    expect(find.text('Editorial guide'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
