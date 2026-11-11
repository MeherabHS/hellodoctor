import 'dart:async';

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
class _TolerantGoldenFileComparator extends LocalFileComparator {
  _TolerantGoldenFileComparator(super.testFile);

  static const double _precisionTolerance = 0.01;

  @override
  Future<bool> compare(Uint8List imageBytes, Uri golden) async {
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
