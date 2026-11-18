import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/utils/upload_image_shrink.dart';

void main() {
  group('shrinkTargetShortEdge', () {
    test('images within 2000 px are left alone', () {
      expect(shrinkTargetShortEdge(2000, 1500), isNull);
      expect(shrinkTargetShortEdge(800, 600), isNull);
    });

    test('a 12 MP landscape photo shrinks to a 2000 px long edge', () {
      // 4000x3000 -> short edge 1500, so the result is 2000x1500.
      expect(shrinkTargetShortEdge(4000, 3000), 1500);
    });

    test('portrait and landscape give the same target (EXIF-rotation safe)',
        () {
      expect(
          shrinkTargetShortEdge(3000, 4000), shrinkTargetShortEdge(4000, 3000));
    });

    test('very wide images still get a positive short edge', () {
      expect(shrinkTargetShortEdge(9000, 3), 1);
      expect(shrinkTargetShortEdge(0, 3000), isNull);
    });
  });
}
