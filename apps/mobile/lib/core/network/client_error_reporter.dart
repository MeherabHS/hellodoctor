import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'api_client.dart';
import 'startup_incident_queue.dart';

// Native platforms read the installed bundle; other platforms can supply these
// explicit dart-defines. Defaults match the current checked-in pubspec.
const clientAppVersion =
    String.fromEnvironment('FLUTTER_BUILD_NAME', defaultValue: '1.0.0');
const clientBuildNumber =
    String.fromEnvironment('FLUTTER_BUILD_NUMBER', defaultValue: '1');

String sanitizedClientStack(StackTrace stack) {
  final frames = RegExp(
      r'(?:package:(?:hellodoctor_mobile|flutter)/[a-zA-Z0-9_./-]+\.dart:\d{1,7}:\d{1,7}|dart:[a-z_]+/[a-zA-Z0-9_./-]+\.dart:\d{1,7}:\d{1,7})');
  return frames
      .allMatches(stack
          .toString()
          .substring(0, stack.toString().length.clamp(0, 12000)))
      .take(24)
      .map((m) => m.group(0)!)
      .join('\n')
      .substringSafe(6000);
}

extension on String {
  String substringSafe(int max) => substring(0, length.clamp(0, max));
}

/// Startup failures use a bounded encrypted journal; regular diagnostics remain
/// best effort. No refresh/logout side effects or reporting of the
/// reporter's own failures. All callers may fire-and-forget this future safely.
class ClientErrorReporter {
  ClientErrorReporter(this.api,
      {required this.authenticated,
      this.platform,
      this.appVersion = clientAppVersion,
      this.buildNumber = clientBuildNumber}) {
    api.onUnexpectedError = (error, stack) {
      unawaited(capture(error, stack, operation: 'response_parse')
          .then<void>((_) {}));
    };
  }
  static ClientErrorReporter? current;
  static StartupIncidentQueue startupQueue = StartupIncidentQueue();
  static Future<void> captureStartup(Object error, StackTrace stack) async {
    if (error is ApiException ||
        error is DioException ||
        error is TimeoutException) {
      return;
    }
    final payload = clientIncidentPayload(error, stack,
        screen: 'Application', operation: 'startup', startup: true);
    try {
      final info = await const MethodChannel('hellodoctor/build_metadata')
          .invokeMapMethod<String, String>('get')
          .timeout(const Duration(seconds: 1));
      if (info != null) {
        payload['app_version'] = info['app_version'] ?? clientAppVersion;
        payload['build_number'] = info['build_number'] ?? clientBuildNumber;
      }
    } catch (_) {
      /* Compile-time metadata remains available before native boot. */
    }
    await startupQueue.enqueue(payload);
  }

  Future<void> flushStartup() => startupQueue.flush(
      authenticated: authenticated, send: api.sendClientIncident);
  static void observe(Object error, StackTrace stack,
      {String? screen, String operation = 'unhandled_async'}) {
    final reporter = current;
    if (reporter == null) {
      if (screen == null) unawaited(captureStartup(error, stack));
    } else {
      unawaited(reporter
          .capture(error, stack, screen: screen, operation: operation)
          .then<void>((_) {}));
    }
  }

  final ApiClient api;
  final bool Function() authenticated;
  final String? platform;
  String appVersion, buildNumber;
  String screen = 'Application';
  bool _sending = false;
  final Expando<bool> _reported = Expando<bool>();
  Future<void> loadBuildMetadata() async {
    try {
      final info = await const MethodChannel('hellodoctor/build_metadata')
          .invokeMapMethod<String, String>('get')
          .timeout(const Duration(seconds: 1));
      if (info != null) {
        appVersion = info['app_version'] ?? appVersion;
        buildNumber = info['build_number'] ?? buildNumber;
      }
    } catch (_) {
      // The reporter stays independent of application startup.
    }
  }

  Future<bool> capture(Object error, StackTrace stack,
      {String? screen, String operation = 'unhandled_async'}) async {
    var ownsSend = false;
    try {
      // All API responses and normal connectivity outcomes are handled by the
      // application/server already. This service captures unexpected client errors.
      if (_sending ||
          error is ApiException ||
          error is DioException ||
          error is TimeoutException) {
        return false;
      }
      if (!authenticated()) {
        await captureStartup(error, stack);
        return false;
      }
      if (error is! String && error is! num && error is! bool) {
        if (_reported[error] == true) return false;
        _reported[error] = true;
      }
      _sending = true;
      ownsSend = true;
      final payload = clientIncidentPayload(error, stack,
          screen: screen ?? this.screen,
          operation: operation,
          appVersion: appVersion,
          buildNumber: buildNumber,
          platform: platform);
      return await api.sendClientIncident(payload);
    } catch (_) {
      // Deliberately no logging of the original exception or report body.
      return false;
    } finally {
      if (ownsSend) _sending = false;
    }
  }
}

Map<String, dynamic> clientIncidentPayload(Object error, StackTrace stack,
    {required String screen,
    required String operation,
    bool startup = false,
    String appVersion = clientAppVersion,
    String buildNumber = clientBuildNumber,
    String? platform}) {
  final kind = error.runtimeType.toString();
  const kinds = {
    'TypeError',
    'StateError',
    'FormatException',
    'NoSuchMethodError',
    'RangeError',
    'AssertionError',
    'PlatformException',
    'Exception',
    'Error'
  };
  final safeKind = kinds.contains(kind) ? kind : 'Exception';
  final payload = <String, dynamic>{
    'client': 'DOCTOR_FLUTTER', 'error_type': safeKind,
    // Never call error.toString(): it can contain PHI or a raw request body.
    'message': 'Unexpected client exception ($safeKind).',
    'stack_trace': sanitizedClientStack(stack),
    'screen': safeClientScreen(screen),
    'operation': startup ? 'startup' : safeClientOperation(operation),
    'app_version': appVersion,
    'build_number': buildNumber,
    'platform': platform ?? (kIsWeb ? 'web' : defaultTargetPlatform.name),
    'occurred_at': DateTime.now().toUtc().toIso8601String(),
    if (startup) 'startup_reference': startupErrorReference(error),
  };
  return payload;
}

String startupErrorReference(Object error) {
  var hash = 0x811c9dc5;
  for (final unit in error.runtimeType.toString().codeUnits) {
    hash = ((hash ^ unit) * 0x01000193) & 0xffffffff;
  }
  return hash.toRadixString(16).padLeft(8, '0').toUpperCase();
}

String safeClientScreen(String value) => const {
      'Application',
      'Auth',
      'Queue',
      'Chat',
      'SmartRx',
      'Schedule',
      'Wallet',
      'Qna',
      'Telehealth',
      'Settings',
      'Notifications',
      'Patients'
    }.contains(value)
        ? value
        : 'Application';
String safeClientOperation(String value) => const {
      'render',
      'unhandled_async',
      'browser_error',
      'load',
      'send_message',
      'create_slot',
      'block_slot',
      'cancel_slot',
      'book_appointment',
      'login',
      'verify_login',
      'load_consultation',
      'upload_prescription',
      'complete_consultation',
      'call_telemetry',
      'mint_rtc_token',
      'load_document',
      'update',
      'response_parse',
      'push_init',
      'register_device',
      'revoke_device'
    }.contains(value)
        ? value
        : 'unhandled_async';

/// Shared by the actual main.dart hooks and tests. Zone errors are reported
/// directly, avoiding a second FlutterError.reportError callback.
class ClientErrorBoundaries {
  static void flutter(FlutterErrorDetails details) {
    FlutterError.presentError(details);
    ClientErrorReporter.observe(
        details.exception, details.stack ?? StackTrace.current,
        operation: 'render');
  }

  static void asynchronous(Object error, StackTrace stack) {
    ClientErrorReporter.observe(error, stack);
  }

  static bool platform(Object error, StackTrace stack) {
    asynchronous(error, stack);
    return true;
  }

  static void install() {
    FlutterError.onError = flutter;
    PlatformDispatcher.instance.onError = platform;
  }
}
