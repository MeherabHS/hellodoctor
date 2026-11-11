import 'dart:async';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blog_detail_page.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class DiscoveryTokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

class DiscoveryApi extends ApiClient {
  DiscoveryApi()
      : super(
            dio: Dio(BaseOptions(baseUrl: 'https://example.test/api/v1')),
            tokenStore: DiscoveryTokens());
  bool fail = false;
  Future<void>? pause;
  final requests = <Map<String, dynamic>>[];
  final rows = List.generate(
      27,
      (i) => <String, dynamic>{
            'id':
                '00000000-0000-4000-8000-${(i + 1).toString().padLeft(12, '0')}',
            'title': i == 26 ? 'Older health guide' : 'Guide $i',
            'summary': 'Useful educational reading',
            'body': 'Educational content',
            'author_name': 'Dr. Synthetic',
            'specialty': 'General practice',
            'status': 'PUBLISHED',
            'article_type': i.isEven ? 'DOCTOR' : 'ADMIN',
            'published_at': DateTime.utc(2026, 10, 27)
                .subtract(Duration(minutes: i))
                .toIso8601String(),
          });
  @override
  Future<Response<dynamic>> get(String path,
      {Map<String, dynamic>? query}) async {
    if (pause != null) await pause;
    if (fail) throw const ApiException(503, 'Synthetic unavailable');
    if (path.startsWith('/articles/')) {
      final id = path.split('/').last;
      return Response(requestOptions: RequestOptions(path: path), data: {
        'success': true,
        'data': rows.firstWhere((r) => r['id'] == id)
      });
    }
    final q = {...?query};
    requests.add(q);
    final search = (q['search'] ?? '').toString().toLowerCase();
    final matching = rows
        .where((r) => BlogArticleDto.fromJson(r)
            .titleFor(q['language']?.toString() ?? 'en')
            .toLowerCase()
            .contains(search))
        .toList();
    var offset = (q['offset'] as int?) ?? 0;
    if (q['before_id'] != null) {
      offset = matching.indexWhere((r) => r['id'] == q['before_id']) + 1;
    }
    final limit = (q['limit'] as int?) ?? 20;
    final page = matching.skip(offset).take(limit).toList();
    final more = offset + page.length < matching.length;
    return Response(requestOptions: RequestOptions(path: path), data: {
      'success': true,
      'data': page,
      'next_cursor': more
          ? {
              'before_time': page.last['published_at'],
              'before_id': page.last['id']
            }
          : null,
    });
  }
}

Widget discoveryHost(DiscoveryApi api, {String language = 'en'}) => MaterialApp(
      locale: Locale(language),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: BlogsPage(repo: BlogsRepository(api)),
    );

void main() {
  testWidgets('older articles are discoverable beyond the first twenty',
      (tester) async {
    final api = DiscoveryApi();
    addTearDown(api.dispose);
    await tester.pumpWidget(discoveryHost(api));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
        find.byKey(const ValueKey('blogs-load-more')), 400,
        maxScrolls: 20,
        scrollable: find.descendant(
            of: find.byType(ListView), matching: find.byType(Scrollable)));
    await tester.tap(find.byKey(const ValueKey('blogs-load-more')));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text('Older health guide'), 400,
        maxScrolls: 20,
        scrollable: find.descendant(
            of: find.byType(ListView), matching: find.byType(Scrollable)));
    expect(find.text('Older health guide'), findsOneWidget);
    expect(api.requests.length, 2);
    expect(api.requests.last['before_id'], isNotNull);
  });
  testWidgets(
      'search queries the authorized archive rather than the loaded page',
      (tester) async {
    final api = DiscoveryApi();
    addTearDown(api.dispose);
    await tester.pumpWidget(discoveryHost(api));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Older health');
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pumpAndSettle();
    expect(api.requests.last['search'], 'Older health');
    expect(find.text('Older health guide'), findsOneWidget);
  });
  testWidgets(
      'Kiswahili archive search opens the existing bilingual Admin detail',
      (tester) async {
    final api = DiscoveryApi();
    addTearDown(api.dispose);
    api.rows[25].addAll({
      'title': 'Editorial guide',
      'title_sw': 'Mwongozo wa afya',
      'body_sw': 'Elimu kwa kila mtu.',
      'content_format': 'markdown'
    });
    await tester.pumpWidget(discoveryHost(api, language: 'sw'));
    await tester.pumpAndSettle();
    expect(find.text('Makala za afya'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'Mwongozo');
    await tester.pump(const Duration(milliseconds: 301));
    await tester.pumpAndSettle();
    expect(api.requests.last['language'], 'sw');
    await tester.tap(find.text('Mwongozo wa afya'));
    await tester.pumpAndSettle();
    expect(find.byType(BlogDetailPage), findsOneWidget);
    expect(find.text('Elimu kwa kila mtu.'), findsOneWidget);
  });
  testWidgets('Kiswahili load failures expose a localized retry that recovers',
      (tester) async {
    final api = DiscoveryApi()..fail = true;
    addTearDown(api.dispose);
    await tester.pumpWidget(discoveryHost(api, language: 'sw'));
    await tester.pumpAndSettle();
    final l10n = AppLocalizations.of(tester.element(find.byType(BlogsPage)));
    expect(find.text(l10n.blogsLoadError), findsOneWidget);
    expect(find.text(l10n.retryButton), findsOneWidget);
    api.fail = false;
    await tester.tap(find.text(l10n.retryButton));
    await tester.pumpAndSettle();
    expect(find.text('Guide 0'), findsOneWidget);
  });
  testWidgets(
      'refresh and a language change restart discovery without an old cursor',
      (tester) async {
    final api = DiscoveryApi();
    addTearDown(api.dispose);
    await tester.pumpWidget(discoveryHost(api));
    await tester.pumpAndSettle();
    await tester.drag(find.byType(ListView), const Offset(0, 450));
    await tester.pumpAndSettle();
    expect(api.requests.length, 2);
    expect(api.requests.last['before_id'], isNull);
    await tester.pumpWidget(discoveryHost(api, language: 'sw'));
    await tester.pumpAndSettle();
    expect(api.requests.last['language'], 'sw');
    expect(api.requests.last['before_id'], isNull);
  });
  testWidgets('leaving Blogs fences a late discovery response', (tester) async {
    final pending = Completer<void>(), api = DiscoveryApi();
    api.pause = pending.future;
    addTearDown(api.dispose);
    await tester.pumpWidget(discoveryHost(api));
    await tester.pump();
    await tester.pumpWidget(const SizedBox());
    pending.complete();
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });
}
