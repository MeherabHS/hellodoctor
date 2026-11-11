import 'notification_taps.dart';
import 'dart:async';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';

import '../../features/notifications/notification_repository.dart';
import '../network/client_error_reporter.dart';

/// Thin FCM adapter for the Doctor app. FCM messages only trigger a refresh of
/// the existing authenticated inbox; the payload is never used to authorize
/// navigation or change appointment, consultation or financial state.
class PushNotificationService {
  PushNotificationService({
    required this.repository,
    required this.onNotificationReceived,
  });

  final NotificationRepository repository;
  final Future<void> Function() onNotificationReceived;
  StreamSubscription<String>? _tokenSubscription;
  StreamSubscription<RemoteMessage>? _foregroundSubscription;
  StreamSubscription<RemoteMessage>? _openedSubscription;
  bool _started = false;
  bool _disposed = false;

  Future<void> initialize() async {
    if (_started || _disposed) return;
    _started = true;
    try {
      await Firebase.initializeApp();
    } catch (error, stack) {
      // Missing google-services/GoogleService-Info configuration is a
      // deployment issue (see docs/FCM_SETUP.md) — push must never block
      // login or the app shell, but the failure must still be visible to
      // operators rather than silently invisible forever, so it goes through
      // the same sanitized client-diagnostics pipeline as any other
      // unexpected client error.
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'push_init');
    }

    try {
      if (_disposed) return;
      final messaging = FirebaseMessaging.instance;
      _foregroundSubscription = FirebaseMessaging.onMessage.listen((_) {
        unawaited(handleForegroundMessage());
      });
      _openedSubscription =
          FirebaseMessaging.onMessageOpenedApp.listen((message) {
        unawaited(handleOpenedMessage(message.data));
      });
      final initial = await messaging.getInitialMessage();
      if (initial != null) await handleOpenedMessage(initial.data);
      if (_disposed) return;
      final settings = await messaging.requestPermission(
        alert: true,
        badge: true,
        sound: true,
        provisional: true,
      );
      if (_disposed) return;
      if (settings.authorizationStatus == AuthorizationStatus.denied) return;
      final deviceId = await repository.deviceId();
      final token = await messaging.getToken();
      if (_disposed) return;
      if (token != null && token.isNotEmpty) {
        await repository.registerDevice(
          platform: NotificationRepository.currentPlatformLabel(),
          pushToken: token,
          deviceId: deviceId,
        );
      }
      if (_disposed) return;
      _tokenSubscription = messaging.onTokenRefresh.listen((nextToken) {
        if (nextToken.isEmpty) return;
        unawaited(_registerToken(nextToken, deviceId));
      });
    } catch (error, stack) {
      // Permissions, provider configuration and connectivity are all
      // best-effort. The in-app inbox remains available without FCM — but,
      // as above, the failure is still captured through sanitized
      // diagnostics instead of silently disappearing.
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'push_init');
    }
  }

  Future<void> handleForegroundMessage() async {
    if (!_disposed) await onNotificationReceived();
  }

  Future<void> handleOpenedMessage(Map<String, dynamic> data) async {
    if (_disposed) return;
    NotificationTaps.receive(data);
    await onNotificationReceived();
  }

  Future<void> _registerToken(String token, String deviceId) async {
    if (_disposed) return;
    try {
      await repository.registerDevice(
        platform: NotificationRepository.currentPlatformLabel(),
        pushToken: token,
        deviceId: deviceId,
      );
    } catch (_) {}
  }

  Future<void> dispose() async {
    _disposed = true;
    await _tokenSubscription?.cancel();
    await _foregroundSubscription?.cancel();
    await _openedSubscription?.cancel();
  }
}
