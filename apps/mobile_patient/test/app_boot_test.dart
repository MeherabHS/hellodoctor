import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/app.dart';

void main() {
  // No `ENVIRONMENT` dart-define is supplied by a plain `flutter test` run,
  // which is exactly the condition `validateMobileDeployment`
  // (`core/network/api_client.dart`) is designed to reject outright rather
  // than silently default — see that function's doc comment: an
  // unconfigured backend address must never white-screen a real device, and
  // must never silently fall back to an insecure default either. So the
  // real, correct behavior under these test conditions is the
  // `StartupErrorPage` safety net, not the signed-out phone-entry screen —
  // this test pins down that the net actually catches it, matching
  // CLAUDE.md's "never leave a blank screen" startup-safety requirement.
  testWidgets(
      'an unconfigured ENVIRONMENT is safely caught by the startup error '
      'page rather than a blank screen or an uncaught exception',
      (tester) async {
    // Without an explicit unmount, `_AppState.dispose()` (which cancels
    // `ConnectivityService`'s stream subscription) never runs before the
    // test ends, and the test binding flags the still-pending timer as a
    // leak — the same defensive pattern used throughout `apps/mobile`'s own
    // widget tests.
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(const HelloDoctorPatientApp());
    await tester.pumpAndSettle();

    expect(find.text('AfyaMkononi could not start'), findsOneWidget);
    expect(tester.takeException(), isNull);

    // `captureStartup` chains a best-effort 1s-timeout MethodChannel probe
    // for native build metadata into `StartupIncidentQueue.enqueue`, whose
    // own secure-storage read/write each carry a 2s timeout — none of this
    // touches the widget tree, so `pumpAndSettle` has nothing to keep
    // pumping for and returns before any of it resolves. Flush the whole
    // chain explicitly (1s + 2s + 2s, padded) so no Timer outlives the test.
    await tester.pump(const Duration(seconds: 6));
  });
}
