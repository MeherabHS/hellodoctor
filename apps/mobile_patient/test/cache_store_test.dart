import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/cache/patient_cache_store.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';

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

  test('cache is scoped, versioned and records freshness metadata', () async {
    final store = MemoryStore();
    final cache = PatientCacheStore(store);
    await cache.initializeScope(jwtFor('patient-a'));
    await cache.write(
        'appointments',
        [
          {'id': 'a1'}
        ],
        ttl: const Duration(minutes: 5));
    final saved = await cache.read('appointments');
    expect(saved?.data, isA<List>());
    expect(saved?.scope, isNotEmpty);
    expect(saved?.cachedAt, isNotNull);
    expect(saved?.isExpired, isFalse);
    await cache.clear();
    expect(await cache.read('appointments'), isNull);
    expect(await store.read('patient_cache_scope_v1'), isNull);
  });

  test('corrupt and incompatible cache entries are discarded safely', () async {
    final store = MemoryStore();
    final cache = PatientCacheStore(store);
    await cache.initializeScope(jwtFor('patient-a'));
    await store.write('patient_cache_v1_doctors', '{bad json');
    expect(await cache.read('doctors'), isNull);
    await store.write('patient_cache_v1_doctors',
        '{"schema_version":999,"scope":"x","cached_at":"2026-01-01T00:00:00Z","data":[]}');
    expect(await cache.read('doctors'), isNull);
  });

  test('expired entries remain readable as stale data and expose expiry',
      () async {
    final store = MemoryStore();
    final cache = PatientCacheStore(store);
    await cache.initializeScope(jwtFor('patient-a'));
    await cache.write('doctors', [], ttl: const Duration(seconds: -1));
    expect((await cache.read('doctors'))?.isExpired, isTrue);
  });

  test('a different authenticated scope cannot read prior patient data',
      () async {
    final store = MemoryStore();
    final first = PatientCacheStore(store);
    await first.initializeScope(jwtFor('patient-a'));
    await first.write('profile', {'full_name': 'Rafiq Ahmed'});
    final second = PatientCacheStore(store);
    await second.initializeScope(jwtFor('patient-b'));
    expect(await second.read('profile'), isNull);
  });

  test('an arbitrary, non-allowlisted key is silently ignored', () async {
    final store = MemoryStore();
    final cache = PatientCacheStore(store);
    await cache.initializeScope(jwtFor('patient-a'));
    await cache.write('not_a_real_key', {'x': 1});
    expect(await cache.read('not_a_real_key'), isNull);
    expect(
        store.values.containsKey('patient_cache_v1_not_a_real_key'), isFalse);
  });

  test('logout clears every scoped resource key', () async {
    final store = MemoryStore();
    final cache = PatientCacheStore(store);
    await cache.initializeScope(jwtFor('patient-a'));
    for (final key in PatientCacheStore.keys) {
      await cache.write(key, {'v': key});
    }
    await cache.clear();
    expect(store.values.keys.where((k) => k.startsWith('patient_cache_v1_')),
        isEmpty);
  });
}
