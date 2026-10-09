import 'dart:async';
import 'package:flutter/material.dart';
import 'app.dart';
import 'core/widgets/startup_error_page.dart';
import 'core/network/client_error_reporter.dart';

/// Any uncaught error during boot must never leave the device showing
/// nothing — for a real patient or doctor that is a serious failure, not
/// just an inconvenience, and a field tester sideloading an APK has no
/// adb/logcat to fall back on either. `HelloDoctorApp` already catches and
/// displays its own initialization errors (see app.dart); the zone guard
/// below is a last-resort net in case `runApp` itself fails for some other
/// reason before any widget tree mounts — without it, that case would be
/// the one remaining way to end up with a truly blank screen.
void main() {
  runZonedGuarded(() {
    WidgetsFlutterBinding.ensureInitialized();
    ClientErrorBoundaries.install();
    ErrorWidget.builder =
        (details) => StartupErrorPage(error: details.exception);
    runApp(const HelloDoctorApp());
  }, (error, stack) {
    ClientErrorBoundaries.asynchronous(error, stack);
    if (ClientErrorReporter.current == null) {
      runApp(StartupErrorPage(error: error));
    }
  });
}
