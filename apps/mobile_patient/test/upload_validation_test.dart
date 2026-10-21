import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/validation/upload_validation.dart';

void main() {
  group('isValidIntakeImageCount', () {
    test('accepts the documented 1-20 range', () {
      expect(isValidIntakeImageCount(1), isTrue);
      expect(isValidIntakeImageCount(20), isTrue);
      expect(isValidIntakeImageCount(10), isTrue);
    });

    test('rejects zero and anything above 20', () {
      expect(isValidIntakeImageCount(0), isFalse);
      expect(isValidIntakeImageCount(21), isFalse);
    });
  });

  group('excessIntakeImageCount', () {
    test('is zero within bounds', () {
      expect(excessIntakeImageCount(20), 0);
      expect(excessIntakeImageCount(5), 0);
    });

    test('reports exactly how many must be dropped', () {
      expect(excessIntakeImageCount(23), 3);
    });
  });

  group('encodeIntakeFile', () {
    test('base64-encodes the bytes with the mime type alongside', () {
      final bytes = Uint8List.fromList([1, 2, 3, 4]);
      final encoded = encodeIntakeFile(mimeType: 'image/jpeg', bytes: bytes);
      expect(encoded['mime_type'], 'image/jpeg');
      expect(base64Decode(encoded['base64_content']!), bytes);
    });

    test('accepts a PDF, matching the backend\'s real accepted mime types', () {
      final encoded = encodeIntakeFile(
          mimeType: 'application/pdf', bytes: Uint8List.fromList([1]));
      expect(encoded['mime_type'], 'application/pdf');
    });

    test('rejects a mime type outside the allowed set', () {
      expect(
          () => encodeIntakeFile(
              mimeType: 'image/gif', bytes: Uint8List.fromList([1])),
          throwsArgumentError);
    });
  });
}
