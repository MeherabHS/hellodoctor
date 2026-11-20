import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';
import 'package:path_provider/path_provider.dart';

import '../network/api_client.dart';

/// Health Records available offline (product owner, 2026-11-20).
///
/// CLAUDE.md §1.7: clinical documents are never put in a generic disk cache.
/// Each saved document is AES-256-GCM encrypted with a per-install random key
/// held in the existing secure storage, bound to its document id (so files
/// cannot be swapped), and written to the app's private support directory.
/// Everything — files and key — is wiped at sign-out and session expiry
/// ([OfflineDocuments.clear]). Total size is capped; the oldest saved
/// documents are evicted first.
class OfflineDocuments {
  OfflineDocuments._();

  static OfflineDocumentStore? _store;

  /// Configured once at app start with the app's secure storage.
  static void configure(TokenStore secure) =>
      _store = OfflineDocumentStore(secure);

  static OfflineDocumentStore? get store => _store;

  /// Sign-out / session expiry: removes the key (which makes every saved
  /// copy unreadable at once) and then the files. Callers do not wait for
  /// it: file deletion must never hold up signing out.
  static Future<void> clear() async => _store?.clear();
}

class OfflineDocumentStore {
  OfflineDocumentStore(this.secure, {Future<Directory> Function()? directory})
      : _directory = directory ?? _defaultDirectory;

  final TokenStore secure;
  final Future<Directory> Function() _directory;

  static const _keyName = 'offline_documents_key_v1';

  /// About 100 prescription photos at the 600 KB ceiling, or a mix with PDFs.
  static const maxTotalBytes = 60 * 1024 * 1024;
  static final _algorithm = AesGcm.with256bits();
  static final _safeId = RegExp(r'^[A-Za-z0-9-]{1,64}$');

  static Future<Directory> _defaultDirectory() async =>
      Directory('${(await getApplicationSupportDirectory()).path}'
          '${Platform.pathSeparator}offline_documents');

  Future<SecretKey> _key({bool create = true}) async {
    final saved = await secure.read(_keyName);
    if (saved != null) return SecretKey(base64Decode(saved));
    if (!create) throw StateError('No offline document key.');
    final key = await _algorithm.newSecretKey();
    await secure.write(_keyName, base64Encode(await key.extractBytes()));
    return key;
  }

  Future<File?> _file(String documentId) async {
    if (!_safeId.hasMatch(documentId)) return null;
    final dir = await _directory();
    return File('${dir.path}${Platform.pathSeparator}$documentId.bin');
  }

  /// Saves [bytes] for [documentId], replacing any earlier copy.
  Future<void> put(String documentId, TypedBytes document) async {
    final file = await _file(documentId);
    if (file == null) return;
    final box = await _algorithm.encrypt(document.bytes,
        secretKey: await _key(), aad: utf8.encode(documentId));
    final type = utf8.encode(document.contentType ?? '');
    final out = BytesBuilder(copy: false)
      ..addByte(1) // format version
      ..addByte(type.length)
      ..add(type)
      ..add(box.nonce)
      ..add(box.mac.bytes)
      ..add(box.cipherText);
    await file.parent.create(recursive: true);
    await file.writeAsBytes(out.takeBytes(), flush: true);
    await _evict();
  }

  /// The saved document, or null when it was never saved or cannot be read.
  Future<TypedBytes?> get(String documentId) async {
    final file = await _file(documentId);
    if (file == null || !await file.exists()) return null;
    try {
      final raw = await file.readAsBytes();
      if (raw.isEmpty || raw[0] != 1) return null;
      final typeLength = raw[1];
      var at = 2;
      final type = utf8.decode(raw.sublist(at, at + typeLength));
      at += typeLength;
      final nonce = raw.sublist(at, at + 12);
      at += 12;
      final mac = Mac(raw.sublist(at, at + 16));
      at += 16;
      final clear = await _algorithm.decrypt(
          SecretBox(raw.sublist(at), nonce: nonce, mac: mac),
          secretKey: await _key(create: false),
          aad: utf8.encode(documentId));
      // Touch so eviction keeps recently opened documents.
      await file.setLastModified(DateTime.now());
      return TypedBytes(
          bytes: Uint8List.fromList(clear),
          contentType: type.isEmpty ? null : type);
    } catch (_) {
      // Tampered, truncated or from a wiped key: drop it.
      try {
        await file.delete();
      } catch (_) {}
      return null;
    }
  }

  /// Ids of every saved document.
  Future<Set<String>> savedIds() async {
    final dir = await _directory();
    if (!await dir.exists()) return {};
    return {
      await for (final entry in dir.list())
        if (entry is File && entry.path.endsWith('.bin'))
          entry.uri.pathSegments.last.replaceAll('.bin', ''),
    };
  }

  Future<void> _evict() async {
    final dir = await _directory();
    if (!await dir.exists()) return;
    final files = [
      await for (final entry in dir.list())
        if (entry is File) entry,
    ];
    final stats = {for (final f in files) f: await f.stat()};
    var total = stats.values.fold<int>(0, (sum, s) => sum + s.size);
    final oldestFirst = files
      ..sort((a, b) => stats[a]!.modified.compareTo(stats[b]!.modified));
    for (final file in oldestFirst) {
      if (total <= maxTotalBytes) break;
      total -= stats[file]!.size;
      try {
        await file.delete();
      } catch (_) {}
    }
  }

  Future<void> clear() async {
    // Key first: without it no saved copy can be decrypted, even before the
    // files themselves are gone.
    try {
      await secure.delete(_keyName);
    } catch (_) {}
    try {
      final dir = await _directory();
      if (await dir.exists()) await dir.delete(recursive: true);
    } catch (_) {}
  }
}
