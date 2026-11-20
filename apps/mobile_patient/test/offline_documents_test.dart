import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/cache/offline_documents.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';

class _MemoryStore implements TokenStore {
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
  late Directory dir;
  late _MemoryStore secure;
  late OfflineDocumentStore store;
  const id = '0d2c6b1e-0000-4000-8000-000000000001';
  final photo = TypedBytes(
      bytes: Uint8List.fromList(List.generate(4096, (i) => i % 251)),
      contentType: 'image/jpeg');

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('offline-docs-');
    secure = _MemoryStore();
    store = OfflineDocumentStore(secure, directory: () async => dir);
  });
  tearDown(() async {
    if (await dir.exists()) await dir.delete(recursive: true);
  });

  test('a saved document reads back exactly, with its type', () async {
    await store.put(id, photo);
    final back = await store.get(id);
    expect(back!.bytes, photo.bytes);
    expect(back.contentType, 'image/jpeg');
    expect(await store.savedIds(), {id});
  });

  test('the file on disk is encrypted, not the document', () async {
    await store.put(id, photo);
    final raw =
        await File('${dir.path}${Platform.pathSeparator}$id.bin').readAsBytes();
    // No 64-byte run of the plaintext appears in the stored file.
    final plain = photo.bytes.sublist(100, 164);
    var found = false;
    for (var i = 0; i + plain.length <= raw.length && !found; i++) {
      var match = true;
      for (var j = 0; j < plain.length; j++) {
        if (raw[i + j] != plain[j]) {
          match = false;
          break;
        }
      }
      found = match;
    }
    expect(found, isFalse);
  });

  test('a file copied under another document id is refused', () async {
    const other = '0d2c6b1e-0000-4000-8000-000000000002';
    await store.put(id, photo);
    final sep = Platform.pathSeparator;
    await File('${dir.path}$sep$id.bin').copy('${dir.path}$sep$other.bin');
    expect(await store.get(other), isNull);
  });

  test('sign-out removes the key and every saved document', () async {
    await store.put(id, photo);
    await store.clear();
    expect(secure.values, isEmpty);
    expect(await store.get(id), isNull);
    expect(await store.savedIds(), isEmpty);
  });

  test('ids that are not plain ids are never used as file names', () async {
    await store.put('../escape', photo);
    expect(await store.get('../escape'), isNull);
    expect(await store.savedIds(), isEmpty);
  });
}
