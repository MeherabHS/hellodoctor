import 'dart:async';
import 'dart:convert';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'api_client.dart';

/// Device-level startup diagnostics contain no account or clinical data. A
/// separate native storage namespace keeps logout from erasing undelivered boot
/// failures. Only already-sanitized envelopes may be enqueued by the reporter.
class StartupIncidentQueue {
  StartupIncidentQueue({TokenStore? store, DateTime Function()? now})
      : _store = store ??
            SecureTokenStore(const FlutterSecureStorage(
                aOptions: AndroidOptions(
                    sharedPreferencesName: 'hellodoctor_boot_diagnostics'),
                iOptions:
                    IOSOptions(accountName: 'hellodoctor.boot.diagnostics'),
                mOptions:
                    MacOsOptions(accountName: 'hellodoctor.boot.diagnostics'))),
        _now = now ?? DateTime.now;

  static const storageKey = 'startup_incidents_v1';
  static const maxEntries = 8;
  static const maxAge = Duration(days: 7);
  final TokenStore _store;
  final DateTime Function() _now;
  Future<void> _operations = Future<void>.value();

  Future<void> _serialize(Future<void> Function() work) {
    final next = _operations.then((_) => work()).catchError((Object _) {});
    _operations = next;
    return next;
  }

  Future<List<Map<String, dynamic>>> _read() async {
    final raw =
        await _store.read(storageKey).timeout(const Duration(seconds: 2));
    if (raw == null || raw.length > 64000) return [];
    try {
      final items = jsonDecode(raw) as List;
      return items
          .whereType<Map>()
          .map((e) => Map<String, dynamic>.from(e))
          .where((e) {
            final occurred =
                DateTime.tryParse(e['occurred_at']?.toString() ?? '');
            return occurred != null &&
                !occurred.isAfter(_now().toUtc()) &&
                _now().toUtc().difference(occurred) <= maxAge;
          })
          .take(maxEntries)
          .toList();
    } catch (_) {
      return [];
    }
  }

  Future<void> _write(List<Map<String, dynamic>> entries) async {
    if (entries.isEmpty) {
      await _store.delete(storageKey).timeout(const Duration(seconds: 2));
    } else {
      await _store
          .write(storageKey, jsonEncode(entries))
          .timeout(const Duration(seconds: 2));
    }
  }

  Future<void> enqueue(Map<String, dynamic> payload) => _serialize(() async {
        final entries = await _read();
        // Repeated retry taps should not fill the bounded journal with one error.
        if (!entries.any((e) =>
            e['startup_reference'] == payload['startup_reference'] &&
            e['stack_trace'] == payload['stack_trace'] &&
            e['app_version'] == payload['app_version'] &&
            e['build_number'] == payload['build_number'])) {
          if (entries.length == maxEntries) entries.removeAt(0);
          entries.add(payload);
        }
        await _write(entries);
      });

  Future<void> flush(
          {required bool Function() authenticated,
          required Future<bool> Function(Map<String, dynamic>) send}) =>
      _serialize(() async {
        final entries = await _read();
        while (entries.isNotEmpty && authenticated()) {
          if (!await send(entries.first)) break;
          entries.removeAt(0);
          await _write(entries);
        }
        await _write(entries);
      });
}
