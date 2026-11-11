import 'dart:async';
import 'package:flutter/foundation.dart';
import 'blogs_models.dart';
import 'blogs_repository.dart';

/// Screen-local discovery. Generations prevent old search/refresh/page responses
/// from replacing newer results; failures retain a retryable cursor.
class BlogDiscoveryController extends ChangeNotifier {
  BlogDiscoveryController(this.repository);
  final BlogsRepository repository;
  List<BlogArticleDto> items = [];
  BlogCursor? next;
  String query = '', language = 'en';
  bool loading = true, loadingMore = false, failed = false, pageFailed = false;
  int _generation = 0;
  bool _disposed = false;
  Timer? _debounce;
  void _emit() {
    if (!_disposed) notifyListeners();
  }

  void search(String value) {
    query = value;
    _debounce?.cancel();
    _generation++; // Invalidate responses immediately, including during debounce.
    next = null;
    loadingMore = false;
    loading = true;
    _emit();
    _debounce = Timer(const Duration(milliseconds: 300), refresh);
  }

  Future<void> setLanguage(String value) async {
    language = value;
    await refresh();
  }

  Future<void> refresh() async {
    _debounce?.cancel();
    final generation = ++_generation;
    loading = true;
    loadingMore = false;
    failed = false;
    pageFailed = false;
    next = null;
    items = [];
    _emit();
    try {
      final page = await repository.discover(search: query, language: language);
      if (_disposed || generation != _generation) return;
      items = {for (final a in page.items) a.id: a}.values.toList();
      next = page.next;
    } catch (_) {
      if (!_disposed && generation == _generation) failed = true;
    } finally {
      if (!_disposed && generation == _generation) {
        loading = false;
        _emit();
      }
    }
  }

  Future<void> loadMore() async {
    if (loading || loadingMore || next == null || _disposed) return;
    final generation = _generation, cursor = next!;
    loadingMore = true;
    pageFailed = false;
    _emit();
    try {
      final page = await repository.discover(
          search: query, language: language, cursor: cursor);
      if (_disposed || generation != _generation) return;
      if (page.next?.id == cursor.id && page.next?.time == cursor.time) {
        throw const FormatException('Article cursor did not advance.');
      }
      items = {
        for (final a in [...items, ...page.items]) a.id: a
      }.values.toList();
      next = page.next;
    } catch (_) {
      if (!_disposed && generation == _generation) pageFailed = true;
    } finally {
      if (!_disposed && generation == _generation) {
        loadingMore = false;
        _emit();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    _debounce?.cancel();
    super.dispose();
  }
}
