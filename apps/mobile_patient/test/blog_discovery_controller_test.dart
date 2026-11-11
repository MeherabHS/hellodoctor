import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blog_discovery_controller.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_repository.dart';
import 'article_discovery_test.dart' show DiscoveryApi;

BlogArticleDto row(int i) => BlogArticleDto.fromJson({
      'id': '00000000-0000-4000-8000-${i.toString().padLeft(12, '0')}',
      'title': 'Guide $i',
      'summary': 'Reading',
      'body': 'Guidance',
      'status': 'PUBLISHED',
      'author_name': 'Doctor',
    });
final cursor = BlogCursor('2026-01-01T00:00:00Z', row(1).id);

class ControlledDiscovery extends BlogsRepository {
  ControlledDiscovery() : super(DiscoveryApi());
  Future<BlogPage> Function(int, String, String, BlogCursor?)? respond;
  final calls = <(String, String, BlogCursor?)>[];
  @override
  Future<BlogPage> discover(
      {String search = '',
      String language = 'en',
      BlogCursor? cursor,
      int limit = 20}) {
    calls.add((search, language, cursor));
    return respond!(calls.length, search, language, cursor);
  }
}

void main() {
  late ControlledDiscovery repo;
  late BlogDiscoveryController controller;
  setUp(() {
    repo = ControlledDiscovery();
    controller = BlogDiscoveryController(repo);
  });
  tearDown(() {
    controller.dispose();
    repo.api.dispose();
  });
  test('later pages deduplicate IDs and concurrent load-more calls coalesce',
      () async {
    final pending = Completer<BlogPage>();
    repo.respond = (call, _, __, ___) async =>
        call == 1 ? BlogPage([row(1)], cursor) : pending.future;
    await controller.refresh();
    final load = controller.loadMore();
    await controller.loadMore();
    expect(repo.calls.length, 2);
    expect(controller.loadingMore, isTrue);
    pending.complete(BlogPage([row(1), row(2)], null));
    await load;
    expect(controller.items.map((a) => a.id), [row(1).id, row(2).id]);
    expect(controller.next, isNull);
  });
  test('refresh clears the cursor and excludes an obsolete response', () async {
    final stale = Completer<BlogPage>();
    repo.respond = (call, _, __, ___) async =>
        call == 1 ? stale.future : BlogPage([row(2)], null);
    final first = controller.refresh();
    await controller.refresh();
    stale.complete(BlogPage([row(1)], cursor));
    await first;
    expect(controller.items.single.id, row(2).id);
    expect(controller.next, isNull);
  });
  test('failed next page retains items and its cursor for a successful retry',
      () async {
    repo.respond = (call, _, __, ___) async {
      if (call == 1) return BlogPage([row(1)], cursor);
      if (call == 2) throw StateError('Offline');
      return BlogPage([row(2)], null);
    };
    await controller.refresh();
    await controller.loadMore();
    expect(controller.pageFailed, isTrue);
    expect(controller.items.single.id, row(1).id);
    expect(controller.next, cursor);
    await controller.loadMore();
    expect(controller.pageFailed, isFalse);
    expect(controller.items.length, 2);
  });
  test('a non-advancing cursor fails visibly instead of looping', () async {
    repo.respond = (call, _, __, ___) async => BlogPage([row(1)], cursor);
    await controller.refresh();
    await controller.loadMore();
    expect(controller.pageFailed, isTrue);
    expect(controller.items.length, 1);
  });
  test('language refresh supersedes an in-flight English page', () async {
    final stale = Completer<BlogPage>();
    repo.respond = (call, _, language, __) async =>
        language == 'en' ? stale.future : BlogPage([row(2)], null);
    final first = controller.refresh();
    await controller.setLanguage('sw');
    stale.complete(BlogPage([row(1)], null));
    await first;
    expect(repo.calls.last.$2, 'sw');
    expect(controller.items.single.id, row(2).id);
  });
  testWidgets('new search fences old pagination during its debounce',
      (tester) async {
    repo.respond = (call, search, _, __) async =>
        BlogPage([row(search.isEmpty ? 1 : 2)], cursor);
    await controller.refresh();
    controller.search('new archive search');
    await controller.loadMore();
    expect(repo.calls.length, 1);
    await tester.pump(const Duration(milliseconds: 301));
    expect(repo.calls.last.$1, 'new archive search');
    expect(repo.calls.last.$3, isNull);
    expect(controller.items.single.id, row(2).id);
  });
  test('invalid cursor structures fail closed', () {
    for (final value in [
      {'before_time': 'invalid', 'before_id': row(1).id},
      {'before_time': cursor.time, 'before_id': '../private'}
    ]) {
      expect(() => BlogCursor.fromJson(value), throwsFormatException);
    }
  });
}
