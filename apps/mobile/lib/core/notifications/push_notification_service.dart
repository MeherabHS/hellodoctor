import 'dart:async';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';

import '../../features/notifications/notification_repository.dart';

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

  Future<void> initialize() async {
    if (_started) return;
    _started = true;
    try {
      await Firebase.initializeApp();
    } catch (_) {
      // Missing google-services/GoogleService-Info configuration is a
      // deployment issue. Push must never block login or the app shell.
    }

    try {
      final messaging = FirebaseMessaging.instance;
      final settings = await messaging.requestPermission(
        alert: true,
        badge: true,
        sound: true,
        provisional: true,
      );
      if (settings.authorizationStatus == AuthorizationStatus.denied) return;
      final deviceId = await repository.deviceId();
      final token = await messaging.getToken();
      if (token != null && token.isNotEmpty) {
        await repository.registerDevice(
          platform: NotificationRepository.currentPlatformLabel(),
          pushToken: token,
          deviceId: deviceId,
        );
      }
      _tokenSubscription = messaging.onTokenRefresh.listen((nextToken) {
        if (nextToken.isEmpty) return;
        unawaited(_registerToken(nextToken, deviceId));
      });
      _foregroundSubscription = FirebaseMessaging.onMessage.listen((_) {
        unawaited(onNotificationReceived());
      });
      _openedSubscription =
          FirebaseMessaging.onMessageOpenedApp.listen((_) {
        unawaited(onNotificationReceived());
      });
      final initial = await messaging.getInitialMessage();
      if (initial != null) unawaited(onNotificationReceived());
    } catch (_) {
      // Permissions, provider configuration and connectivity are all
      // best-effort. The in-app inbox remains available without FCM.
    }
  }

  Future<void> _registerToken(String token, String deviceId) async {
    try {
      await repository.registerDevice(
        platform: NotificationRepository.currentPlatformLabel(),
        pushToken: token,
        deviceId: deviceId,
      );
    } catch (_) {}
  }

  Future<void> dispose() async {
    await _tokenSubscription?.cancel();
    await _foregroundSubscription?.cancel();
    await _openedSubscription?.cancel();
  }
}
