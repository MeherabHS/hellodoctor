import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import '../theme/tokens.dart';
import '../network/client_error_reporter.dart' show startupErrorReference;

/// Shown instead of a blank screen whenever the app fails to start.
///
/// A release build (what a real patient runs) never shows the raw
/// exception — a stack trace means nothing to them and only looks broken.
/// They get a plain, reassuring message with a retry action instead. A
/// debug/profile build (internal pilot/field-test APKs) shows the exact
/// technical reason, since the person holding that phone has no adb/logcat
/// access and is usually the person who can act on it.
class StartupErrorPage extends StatelessWidget {
  const StartupErrorPage({super.key, required this.error, this.onRetry});
  final Object error;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: PatientTokens.theme(),
      home: Scaffold(
          backgroundColor: PatientTokens.bg,
          body: SafeArea(
              child: Center(
                  child: SingleChildScrollView(
                      padding: const EdgeInsets.all(28),
                      child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Icon(Icons.error_outline_rounded,
                                color: PatientTokens.danger, size: 42),
                            const SizedBox(height: 14),
                            const Text('AfyaMkononi could not start',
                                style: TextStyle(
                                    fontSize: 20,
                                    fontWeight: FontWeight.w800,
                                    color: PatientTokens.text)),
                            const SizedBox(height: 10),
                            const Text(
                                kReleaseMode
                                    ? 'Something went wrong while starting the '
                                        'app. Please check your internet '
                                        'connection and try again.'
                                    : 'Show this screen to whoever built the '
                                        'app — the message below is the exact '
                                        'reason.',
                                style: TextStyle(color: PatientTokens.textSub)),
                            if (!kReleaseMode) ...[
                              const SizedBox(height: 18),
                              Container(
                                  width: double.infinity,
                                  padding: const EdgeInsets.all(14),
                                  decoration: BoxDecoration(
                                      color: PatientTokens.surface,
                                      borderRadius: BorderRadius.circular(
                                          PatientTokens.cardRadius),
                                      border: Border.all(
                                          color: PatientTokens.border)),
                                  child: Text(error.toString(),
                                      style: const TextStyle(
                                          fontFamily: 'monospace',
                                          fontSize: 12,
                                          color: PatientTokens.text))),
                            ] else ...[
                              const SizedBox(height: 10),
                              Text('Reference: ${startupErrorReference(error)}',
                                  style: const TextStyle(
                                      fontSize: 11,
                                      color: PatientTokens.textSub)),
                            ],
                            if (onRetry != null) ...[
                              const SizedBox(height: 22),
                              SizedBox(
                                  width: double.infinity,
                                  child: FilledButton(
                                      onPressed: onRetry,
                                      style: FilledButton.styleFrom(
                                          backgroundColor:
                                              PatientTokens.primary,
                                          padding: const EdgeInsets.symmetric(
                                              vertical: 14)),
                                      child: const Text('Try again'))),
                            ]
                          ]))))));
}
