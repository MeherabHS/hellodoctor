// Regression coverage for h17-patient-intake-phi-cache-residue: picked
// prescription/lab intake photos must not be left as unmanaged plaintext PHI
// in the app cache dir, and the file_picker-sourced safety path must never
// raw-delete a path it didn't itself create (see cleanupFilePickerTemp's doc
// comment).
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/pre_consult_intake_modal.dart';

void main() {
  late Directory tempDir;

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('intake_cleanup_test_');
  });

  tearDown(() async {
    if (await tempDir.exists()) await tempDir.delete(recursive: true);
  });

  group('deletePickedSourceFile (image_picker-sourced)', () {
    test('deletes the plugin\'s own cache-dir copy', () async {
      final f = File('${tempDir.path}/picked.jpg');
      await f.writeAsBytes([1, 2, 3]);
      expect(await f.exists(), isTrue);

      await deletePickedSourceFile(f.path);

      expect(
        await f.exists(),
        isFalse,
        reason: 'image_picker temp files must not persist indefinitely',
      );
    });

    test('a missing/already-deleted file does not throw', () async {
      final f = File('${tempDir.path}/does_not_exist.jpg');
      expect(await f.exists(), isFalse);
      await expectLater(deletePickedSourceFile(f.path), completes);
    });
  });

  group('cleanupFilePickerTemp (file_picker-sourced)', () {
    test(
        'never raw-deletes any path — only calls the plugin-safe clear '
        'function', () async {
      final external = File('${tempDir.path}/user_original.pdf');
      await external.writeAsBytes([4, 5, 6]);

      var clearCalled = false;
      await cleanupFilePickerTemp(
        clearFilePickerTemp: () async {
          clearCalled = true;
        },
      );

      expect(clearCalled, isTrue);
      expect(
        await external.exists(),
        isTrue,
        reason: 'file_picker-sourced results must never be deleted by raw '
            'path — only via the plugin\'s own clearTemporaryFiles(), '
            'which never touches a user-owned external original',
      );
    });

    test('a clearFilePickerTemp failure is swallowed, not rethrown', () async {
      await expectLater(
        cleanupFilePickerTemp(
          clearFilePickerTemp: () async =>
              throw Exception('simulated plugin failure'),
        ),
        completes,
      );
    });
  });
}
