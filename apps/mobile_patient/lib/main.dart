import 'dart:async';
import 'package:flutter/material.dart';
import 'app.dart';
import 'core/widgets/startup_error_page.dart';
import 'core/network/client_error_reporter.dart';

/// Any uncaught error during boot must never leave the device showing
/// nothing — for a real patient that is a serious failure, not just an
/// inconvenience, and a field tester sideloading an APK has no adb/logcat to
/// fall back on either. The zone guard below is a last-resort net in case
/// `runApp` itself fails before any widget tree mounts.
void main() {
  runZonedGuarded(() {
    WidgetsFlutterBinding.ensureInitialized();
    ClientErrorBoundaries.install();
    ErrorWidget.builder =
        (details) => StartupErrorPage(error: details.exception);
    runApp(const HelloDoctorPatientApp());
  }, (error, stack) {
    ClientErrorBoundaries.asynchronous(error, stack);
    if (ClientErrorReporter.current == null) {
      runApp(StartupErrorPage(error: error));
    }
  });
}
