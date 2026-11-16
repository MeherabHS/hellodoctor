import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

/// Directory-level test configuration (auto-discovered by `flutter test`,
/// applied to every test file under this directory — see
/// `flutter_test`'s own `goldens.dart` doc comment for this exact pattern).
///
/// Golden rendering has a small, consistent cross-environment component
/// (font hinting/anti-aliasing) even between two Linux machines running the
/// same Flutter version — this repo already documented a 0.47%-5.94% drift
/// between a local Windows capture and real Linux CI (commit
/// `fb7ebf6`, "recapture Doctor golden images on the real Linux CI
/// environment"), and a further 0.07% drift was observed between that
/// capture environment and this repo's actual permanent CI workflow
/// (`application-checks.yml`), which had never run these goldens before.
/// Tolerating up to 1% (an order of magnitude above both observed drifts)
/// absorbs that noise while remaining far tighter than any genuine visual
/// regression in this codebase has ever measured (a real layout/content
/// difference shows as a double-digit percentage, not single digits).
///
/// The golden PNGs are captured on Linux CI (commit `2034a10`), which is the
/// reference renderer. Windows and macOS anti-alias text differently (1-6%,
/// above the 1% tolerance), so comparing pixels there fails with no real
/// difference. On those hosts the screen is still built, laid out and
/// rendered (overflows and exceptions still fail the test); only the pixel
/// comparison is left to Linux CI, and recapturing is refused so a non-Linux
/// capture can never replace the reference images again (as `c564ded` did).
class _TolerantGoldenFileComparator extends LocalFileComparator {
  _TolerantGoldenFileComparator(super.testFile);

  static const double _precisionTolerance = 0.01;

  /// Linux matches the CI capture environment; anything else does not.
  static bool get _isReferenceHost => Platform.isLinux;

  @override
  Future<void> update(Uri golden, Uint8List imageBytes) async {
    if (!_isReferenceHost) {
      throw FlutterError(
          'Refusing to update "$golden" on ${Platform.operatingSystem}: golden '
          'images are captured on Linux CI only (see flutter_test_config.dart).');
    }
    await super.update(golden, imageBytes);
  }

  @override
  Future<bool> compare(Uint8List imageBytes, Uri golden) async {
    if (!_isReferenceHost) {
      debugPrint('Golden "$golden" rendered but not pixel-compared on '
          '${Platform.operatingSystem}; Linux CI compares it.');
      return true;
    }
    final ComparisonResult result = await GoldenFileComparator.compareLists(
      imageBytes,
      await getGoldenBytes(golden),
    );

    if (result.passed || result.diffPercent <= _precisionTolerance) {
      result.dispose();
      return true;
    }

    final String error = await generateFailureOutput(result, golden, basedir);
    result.dispose();
    throw FlutterError(error);
  }
}

Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  goldenFileComparator =
      _TolerantGoldenFileComparator(Uri.parse('test/flutter_test_config.dart'));
  await testMain();
}
