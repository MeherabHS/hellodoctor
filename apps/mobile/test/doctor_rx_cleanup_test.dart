// Regression coverage for h16-doctor-rx-image-plaintext-residue: picked Rx
// images must not be left as unmanaged plaintext PHI after upload/removal,
// and the file_picker-sourced safety path must never raw-delete a path it
// didn't itself create (see cleanupPickedRxFile's doc comment).
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';

void main() {
  late Directory tempDir;

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('rx_cleanup_test_');
  });

  tearDown(() async {
    if (await tempDir.exists()) await tempDir.delete(recursive: true);
  });

  group('cleanupPickedRxFile', () {
    test('deletes an image_picker-sourced (owned cache) file', () async {
      final f = File('${tempDir.path}/picked.jpg');
      await f.writeAsBytes([1, 2, 3]);
      expect(await f.exists(), isTrue);

      await cleanupPickedRxFile(
        path: f.path,
        fromFilePicker: false,
        otherFilePickerPagesPending: false,
      );

      expect(
        await f.exists(),
        isFalse,
        reason:
            'image_picker-sourced temp files must be deleted once no longer needed',
      );
    });

    test(
        'never raw-deletes a file_picker-sourced path — only calls the '
        'plugin-safe clear function', () async {
      final f = File('${tempDir.path}/external_or_cached.pdf');
      await f.writeAsBytes([4, 5, 6]);

      var clearCalled = false;
      await cleanupPickedRxFile(
        path: f.path,
        fromFilePicker: true,
        otherFilePickerPagesPending: false,
        clearFilePickerTemp: () async {
          clearCalled = true;
        },
      );

      expect(
        clearCalled,
        isTrue,
        reason: 'the plugin-safe clearTemporaryFiles() path must run',
      );
      expect(
        await f.exists(),
        isTrue,
        reason: 'a file_picker-sourced path must NEVER be deleted directly by '
            'path — it could be a user-owned external original, not an '
            'app-managed cache copy',
      );
    });

    test(
        'defers the plugin-safe clear call while another file_picker page '
        'is still pending, so a concurrent upload is never starved', () async {
      final f = File('${tempDir.path}/pending_sibling.pdf');
      await f.writeAsBytes([7, 8, 9]);

      var clearCalled = false;
      await cleanupPickedRxFile(
        path: f.path,
        fromFilePicker: true,
        otherFilePickerPagesPending: true,
        clearFilePickerTemp: () async {
          clearCalled = true;
        },
      );

      expect(clearCalled, isFalse);
      expect(await f.exists(), isTrue);
    });

    test('a missing/already-deleted file does not throw', () async {
      final f = File('${tempDir.path}/does_not_exist.jpg');
      expect(await f.exists(), isFalse);

      await expectLater(
        cleanupPickedRxFile(
          path: f.path,
          fromFilePicker: false,
          otherFilePickerPagesPending: false,
        ),
        completes,
      );
    });

    test('a clearFilePickerTemp failure is swallowed, not rethrown', () async {
      await expectLater(
        cleanupPickedRxFile(
          path: '${tempDir.path}/unused.pdf',
          fromFilePicker: true,
          otherFilePickerPagesPending: false,
          clearFilePickerTemp: () async =>
              throw Exception('simulated plugin failure'),
        ),
        completes,
      );
    });
  });
}
