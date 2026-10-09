import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/cache/doctor_cache_store.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';

class MemoryStore implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async => values[key] = value;
  @override
  Future<void> delete(String key) async => values.remove(key);
  @override
  Future<void> clear() async => values.clear();
}

void main() {
  String jwtFor(String subject) =>
      'e30.${base64UrlEncode(utf8.encode(jsonEncode({'sub': subject})))}.sig';

  test('concurrent statement writes remain indexed and logout removes all',
      () async {
    final store = MemoryStore();
    final cache = DoctorCacheStore(store);
    await cache.initializeScope(jwtFor('doctor-a'));
    await Future.wait(List.generate(
        12,
        (month) => cache.write(
            'statement_2026-${(month + 1).toString().padLeft(2, '0')}',
            {'payable': '$month'})));
    final pendingWrite =
        cache.write('statement_2025-12', {'payable': 'private'});
    final clearing = cache.clear();
    await Future.wait([pendingWrite, clearing]);
    expect(store.values.keys.where((key) => key.startsWith('doctor_cache_')),
        isEmpty);
    expect(store.values.containsKey('doctor_statement_cache_keys_v1'), isFalse);
    await cache.initializeScope(jwtFor('doctor-b'));
    expect(await cache.read('statement_2025-12'), isNull);
  });

  test('monthly statements and payout setup cache are protected and cleared',
      () async {
    final store = MemoryStore();
    final cache = DoctorCacheStore(store);
    await cache.initializeScope(jwtFor('doctor-a'));
    await cache
        .write('statement_2026-09', {'payable': '600', 'currency': 'KES'});
    await cache.write('payout_method', {'masked_destination': '•••• 1111'});
    expect((await cache.read('statement_2026-09'))?.data['payable'], '600');
    expect(await cache.read('payout_method'), isNotNull);
    await cache.initializeScope(jwtFor('doctor-b'));
    expect(await cache.read('statement_2026-09'), isNull);
    expect(await store.read('doctor_cache_v1_statement_2026-09'), isNull);
    expect(await cache.read('payout_method'), isNull);
    await cache.write('statement_2026-08', {'payable': '80'});
    await cache.clear();
    expect(await store.read('doctor_cache_v1_statement_2026-08'), isNull);
  });

  test('statement cache is bounded and rejects arbitrary keys', () async {
    final store = MemoryStore();
    final cache = DoctorCacheStore(store);
    await cache.initializeScope(jwtFor('doctor-a'));
    for (var month = 0; month < 25; month++) {
      final date = DateTime(2024, month + 1);
      final key =
          'statement_${date.year}-${date.month.toString().padLeft(2, '0')}';
      await cache.write(key, {'payable': '$month'});
    }
    expect(await cache.read('statement_2024-01'), isNull);
    expect(await cache.read('statement_2026-01'), isNotNull);
    await cache.write('statement_2026-13', {'payable': 'invalid'});
    expect(await cache.read('statement_2026-13'), isNull);
  });

  test('cache is scoped, versioned and records freshness metadata', () async {
    final store = MemoryStore();
    final cache = DoctorCacheStore(store);
    await cache.initializeScope('doctor-a-refresh');
    await cache.write(
        'queue',
        [
          {'id': 'a1'}
        ],
        ttl: const Duration(minutes: 5));
    final saved = await cache.read('queue');
    expect(saved?.data, isA<List>());
    expect(saved?.scope, isNotEmpty);
    expect(saved?.cachedAt, isNotNull);
    expect(saved?.isExpired, isFalse);
    await cache.clear();
    expect(await cache.read('queue'), isNull);
    expect(await store.read('doctor_cache_scope_v1'), isNull);
  });

  test('corrupt and incompatible cache entries are discarded safely', () async {
    final store = MemoryStore();
    final cache = DoctorCacheStore(store);
    await cache.initializeScope('doctor-a');
    await store.write('doctor_cache_v1_queue', '{bad json');
    expect(await cache.read('queue'), isNull);
    await store.write('doctor_cache_v1_queue',
        '{"schema_version":999,"scope":"x","cached_at":"2026-01-01T00:00:00Z","data":[]}');
    expect(await cache.read('queue'), isNull);
  });

  test('expired entries remain readable as stale data and expose expiry',
      () async {
    final store = MemoryStore();
    final cache = DoctorCacheStore(store);
    await cache.initializeScope('doctor-a');
    await cache.write('queue', [], ttl: const Duration(seconds: -1));
    expect((await cache.read('queue'))?.isExpired, isTrue);
  });

  test('a different authenticated scope cannot read prior doctor data',
      () async {
    final store = MemoryStore();
    final first = DoctorCacheStore(store);
    await first.initializeScope(jwtFor('doctor-a'));
    await first.write('wallet', {'final_net': '80'});
    final second = DoctorCacheStore(store);
    await second.initializeScope(jwtFor('doctor-b'));
    expect(await second.read('wallet'), isNull);
  });
}
