// Debug-only design preview for the patient's notification inbox.
//
//   flutter run -t lib/dev/notifications_preview_main.dart
//
// Sample notifications answered in-process; nothing reaches a server.
// Refuses to run in release.

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../core/network/api_client.dart';
import '../core/theme/tokens.dart';
import '../features/notifications/notification_center_page.dart';
import '../features/notifications/notification_cubit.dart';
import '../features/notifications/notification_models.dart';
import '../features/notifications/notification_repository.dart';
import '../l10n/app_localizations.dart';

class _NoTokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

String _ago(Duration d) => DateTime.now().toUtc().subtract(d).toIso8601String();

NotificationDto _notice(
        String id, String type, String title, String body, Duration ago,
        {bool read = false}) =>
    NotificationDto.fromJson({
      'id': id,
      'user_id': 'me',
      'notification_type': type,
      'title': title,
      'body': body,
      'delivery_status': 'DELIVERED',
      'created_at': _ago(ago),
      if (read) 'read_at': _ago(ago),
    });

final _notices = [
  _notice(
      'n1',
      'CONSULTATION_REMINDER_15M',
      'Consultation in 15 minutes',
      'Your video consultation starts soon. Open the waiting room. (sample)',
      const Duration(minutes: 4)),
  _notice('n2', 'NEW_CHAT_MESSAGE', 'New message from your doctor',
      'Open the chat to read it. (sample)', const Duration(minutes: 38)),
  _notice(
      'n3',
      'PRESCRIPTION_AVAILABLE',
      'Prescription ready',
      'Your doctor has sent a prescription to Health Records. (sample)',
      const Duration(hours: 3),
      read: true),
  _notice('n4', 'BOOKING_CONFIRMED', 'Booking confirmed',
      'Your chat consultation is booked. (sample)', const Duration(days: 1),
      read: true),
  _notice(
      'n5',
      'PEER_REQUEST_ACCEPTED',
      'Peer support accepted',
      'Your Peer Navigator accepted your request. (sample)',
      const Duration(days: 3),
      read: true),
];

class _PreviewNotifications extends NotificationRepository {
  _PreviewNotifications()
      : super(
            ApiClient(
                dio: Dio(BaseOptions(baseUrl: 'http://preview.invalid/api/v1')),
                tokenStore: _NoTokens()),
            deviceIdStore: _NoTokens());
  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async =>
      _notices;
  @override
  Future<List<NotificationDto>> newerThan(NotificationDto cursor) async =>
      const [];
  @override
  Future<List<NotificationDto>> olderThan(NotificationDto cursor) async =>
      const [];
  @override
  Future<int> unreadCount() async => _notices.where((n) => !n.isRead).length;
  @override
  Future<void> markRead(String id) async {}
  @override
  Future<int> markAllRead() async => 0;
}

void main() {
  if (kReleaseMode) {
    runApp(const MaterialApp(
        home: Scaffold(
            body: Center(
                child: Text('The preview is not available in release.')))));
    return;
  }
  runApp(MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: PatientTokens.theme(),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Banner(
          message: 'PREVIEW DATA',
          location: BannerLocation.bottomStart,
          child: NotificationCenterPage(
              cubit: NotificationCubit(_PreviewNotifications())))));
}
