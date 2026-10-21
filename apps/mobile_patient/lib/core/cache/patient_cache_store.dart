import 'dart:convert';
import 'dart:async';
import 'package:crypto/crypto.dart';
import '../network/api_client.dart';

class CachedResource {
  const CachedResource({
    required this.data,
    required this.cachedAt,
    required this.scope,
    required this.schemaVersion,
    this.expiresAt,
  });
  final dynamic data;
  final DateTime cachedAt;
  final DateTime? expiresAt;
  final String scope;
  final int schemaVersion;

  bool get isExpired =>
      expiresAt != null && DateTime.now().toUtc().isAfter(expiresAt!);
}

/// A single encrypted, bounded structured cache. It deliberately uses the
/// existing secure storage rather than SharedPreferences, so cached clinical
/// data is protected at rest.
class PatientCacheStore {
  PatientCacheStore(this.store);
  final TokenStore store;
  static const schemaVersion = 1;
  static const _scopeKey = 'patient_cache_scope_v1';
  static const _prefix = 'patient_cache_v1_';
  static const keys = <String>[
    'doctors',
    'doctor_detail',
    'slots',
    'appointments',
    'appointment_detail',
    'documents',
    'chat_messages',
    'profile',
    'family_profiles',
    'notifications',
    'articles',
    'article_detail',
    'qna_feed',
  ];
  String? _scope;
  int _generation = 0;
  Future<void> _operations = Future<void>.value();

  Future<T> _serialize<T>(Future<T> Function() operation) {
    final result = Completer<T>();
    _operations = _operations.then((_) async {
      try {
        result.complete(await operation());
      } catch (error, stack) {
        result.completeError(error, stack);
      }
    });
    return result.future;
  }

  Future<void> initializeScope(String seed) {
    ++_generation;
    _scope = null;
    return _serialize(() => _initializeScope(seed));
  }

  Future<void> _initializeScope(String seed) async {
    final identity = _identitySeed(seed);
    final next = _fingerprint(identity);
    final previous = _scope ?? await store.read(_scopeKey);
    // An opaque refresh token cannot identify an account and rotates during
    // normal refresh. Keep the previously established JWT-subject scope until
    // an access token is available again.
    if (previous != null && identity == seed) {
      _scope = previous;
      return;
    }
    if (previous != null && previous != next) {
      // A different authenticated Patient must never inherit the prior
      // Patient's protected cache. Refresh-token rotation keeps the JWT
      // subject stable, so `_identitySeed` remains stable during one session.
      await _clearResources();
    }
    _scope = next;
    await store.write(_scopeKey, next);
  }

  String? get scope => _scope;

  Future<CachedResource?> read(String key) async {
    if (!_allowedKey(key) || _scope == null) return null;
    final generation = _generation;
    final scope = _scope;
    try {
      final raw = await store.read('$_prefix$key');
      if (generation != _generation || scope != _scope) return null;
      if (raw == null) return null;
      final decoded = jsonDecode(raw);
      if (decoded is! Map || decoded['schema_version'] != schemaVersion) {
        await store.delete('$_prefix$key');
        return null;
      }
      if (decoded['scope'] != _scope) return null;
      final cachedAt =
          DateTime.tryParse(decoded['cached_at']?.toString() ?? '');
      if (cachedAt == null) return null;
      return CachedResource(
        data: decoded['data'],
        cachedAt: cachedAt,
        expiresAt: DateTime.tryParse(decoded['expires_at']?.toString() ?? ''),
        scope: decoded['scope'].toString(),
        schemaVersion: schemaVersion,
      );
    } catch (_) {
      if (generation == _generation) await store.delete('$_prefix$key');
      return null;
    }
  }

  Future<void> write(String key, dynamic data, {Duration? ttl}) {
    final generation = _generation;
    final scope = _scope;
    return _serialize(() async {
      if (generation != _generation || scope != _scope) return;
      await _write(key, data, ttl: ttl);
    });
  }

  Future<void> _write(String key, dynamic data, {Duration? ttl}) async {
    if (!_allowedKey(key) || _scope == null) return;
    final now = DateTime.now().toUtc();
    await store.write(
      '$_prefix$key',
      jsonEncode({
        'schema_version': schemaVersion,
        'scope': _scope,
        'cached_at': now.toIso8601String(),
        'expires_at': ttl == null ? null : now.add(ttl).toIso8601String(),
        'data': data,
      }),
    );
  }

  Future<void> clear() {
    ++_generation;
    _scope = null;
    return _serialize(() async {
      await _clearResources();
      await store.delete(_scopeKey);
    });
  }

  static bool _allowedKey(String key) => keys.contains(key);

  Future<void> _clearResources() async {
    for (final key in keys) {
      await store.delete('$_prefix$key');
    }
  }

  static String _fingerprint(String value) =>
      sha256.convert(utf8.encode(value)).toString();

  static String _identitySeed(String value) {
    // Access tokens are JWTs and carry a stable subject. Refresh tokens are
    // opaque and rotate, so use their value only as a safe fallback.
    final parts = value.split('.');
    if (parts.length != 3) return value;
    try {
      final normalized = base64Url.normalize(parts[1]);
      final payload = jsonDecode(utf8.decode(base64Url.decode(normalized)));
      if (payload is Map) {
        for (final key in const ['sub', 'patient_id', 'user_id']) {
          final id = payload[key];
          if (id != null && id.toString().isNotEmpty) return id.toString();
        }
      }
    } catch (_) {
      // Treat malformed tokens as opaque; the backend remains authoritative.
    }
    return value;
  }
}
