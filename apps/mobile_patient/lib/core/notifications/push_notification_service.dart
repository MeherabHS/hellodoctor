import 'local_notification_service.dart';
import 'notification_ids.dart';
import 'notification_taps.dart';
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
  bool _disposed = false;

  Future<void> initialize() async {
    if (_started || _disposed) return;
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
      if (_disposed) return;
      final messaging = FirebaseMessaging.instance;
      _foregroundSubscription = FirebaseMessaging.onMessage.listen((message) {
        unawaited(showForegroundPush(message));
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
      // FCM is optional until native project configuration and APNs setup are
      // supplied; the existing inbox remains the fallback delivery channel —
      // but, as above, the failure is still captured through sanitized
      // diagnostics instead of silently disappearing.
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'push_init');
    }
  }

  /// Shows a push that arrives while the app is open, with its own title and
  /// text, on its type's channel and sound (product owner, 2026-11-17: a
  /// notification must never be silent or wait for the inbox poll). Android
  /// does not display FCM notifications for a foreground app. Uses the id
  /// the inbox poll would use, so the notice never shows twice.
  static Future<void> showForegroundPush(RemoteMessage message) async {
    final notice = message.notification;
    final id = message.data['notification_id'];
    if (notice == null || id is! String || id.isEmpty) return;
    try {
      await LocalNotificationService.showNow(
        id: stableNotificationId(remoteEventKey(id)),
        channel: NotificationChannels.forType(
            '${message.data['notification_type'] ?? ''}'),
        title: notice.title ?? '',
        body: notice.body ?? '',
        payload: remoteEventKey(id),
      );
    } catch (_) {
      // Display is best-effort; the inbox still refreshes below.
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
