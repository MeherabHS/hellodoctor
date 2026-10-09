import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/articles_page.dart';
import 'package:hellodoctor_mobile/features/doctor/articles_repository.dart';

class _Store implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

Map<String, dynamic> article({String status = 'DRAFT', int version = 1}) => {
      'id': '10000001-0000-0000-0000-000000000001',
      'title': 'Educational title',
      'summary': 'Summary',
      'body': 'Educational article',
      'references_list': <String>[],
      'specialty': 'General medicine',
      'status': status,
      'version': version
    };
void main() {
  test('direct publish statuses do not introduce a review state', () {
    final draft = DoctorArticle.fromJson(article());
    expect(draft.publishable, isTrue);
    final published =
        DoctorArticle.fromJson(article(status: 'PUBLISHED', version: 2));
    expect(published.publishable, isFalse);
    expect(published.editable, isTrue);
    expect(
        DoctorArticle.fromJson(article(status: 'ARCHIVED')).editable, isFalse);
    expect(() => DoctorArticle.fromJson(article(status: 'PENDING_REVIEW')),
        throwsA(isA<ApiException>()));
  });
  test(
      'repository sends server version and excludes author/status mass assignment',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(
          requestOptions: request, data: {'data': article(version: 4)}));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = ArticlesRepository(api);
    final saved = await repo.save(
        article: DoctorArticle.fromJson(article(version: 3)),
        creationKey: articleCreationKey(),
        content: {
          'title': 'Title',
          'summary': 'Summary',
          'body': 'Article',
          'references_list': []
        });
    await repo.transition(saved, publish: true);
    expect(requests[0].method, 'PATCH');
    expect(requests[0].data['version'], 3);
    expect(requests[0].data.containsKey('author_id'), isFalse);
    expect(requests[0].data.containsKey('status'), isFalse);
    expect(requests[1].path, endsWith('/publish'));
    expect(requests[1].data, {'version': 4});
    api.dispose();
  });
  testWidgets(
      'Doctor writes a structured article and publishes without Admin review',
      (tester) async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final mutations = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      if (request.method == 'GET') {
        handler.resolve(Response(requestOptions: request, data: {'data': []}));
        return;
      }
      mutations.add(request);
      final value = article(
          status: request.path.endsWith('/publish') ? 'PUBLISHED' : 'DRAFT',
          version: request.path.endsWith('/publish') ? 2 : 1);
      handler.resolve(Response(requestOptions: request, data: {'data': value}));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    await tester.pumpWidget(MaterialApp(home: DoctorArticlesPage(api: api)));
    await tester.pumpAndSettle();
    expect(
        find.text('No articles yet. Create your first draft.'), findsOneWidget);
    await tester.tap(find.text('New article'));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byType(TextFormField).at(0), 'Educational title');
    await tester.enterText(find.byType(TextFormField).at(1), 'Summary');
    await tester.enterText(
        find.byType(TextFormField).at(2), 'Educational article');
    tester.testTextInput.hide();
    await tester.scrollUntilVisible(find.text('Publish'), 300,
        scrollable: find
            .descendant(
                of: find.byType(ListView).last,
                matching: find.byType(Scrollable))
            .first);
    await tester.tap(find.text('Publish'));
    await tester.pumpAndSettle();
    expect(mutations.length, 2);
    expect(mutations[0].path, '/doctor/articles');
    expect(mutations[1].path, endsWith('/publish'));
    expect(mutations[1].data, {'version': 1});
    expect(find.text('Published to the patient feed.'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    api.dispose();
  });
}
