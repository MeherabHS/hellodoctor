import 'dart:async';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';

import '../../features/notifications/notification_repository.dart';
import '../network/client_error_reporter.dart';

/// Patient FCM adapter. A push only causes an authenticated inbox refresh;
/// sensitive details and deep-link authorization remain server controlled.
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
    } catch (error, stack) {
      // The native Firebase files are deployment inputs (see
      // docs/FCM_SETUP.md). Missing them must not block authentication or
      // in-app notifications, but the failure must still be visible to
      // operators rather than silently invisible forever, so it goes through
      // the same sanitized client-diagnostics pipeline as any other
      // unexpected client error.
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'push_init');
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
    } catch (error, stack) {
      // FCM is optional until native project configuration and APNs setup are
      // supplied; the existing inbox remains the fallback delivery channel —
      // but, as above, the failure is still captured through sanitized
      // diagnostics instead of silently disappearing.
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'push_init');
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
