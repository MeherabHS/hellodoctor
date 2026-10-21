import 'package:dio/dio.dart' show CancelToken;
import 'dart:convert';
import 'dart:typed_data';
import 'dart:math';
import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import 'chat_models.dart';
import '../home/home_models.dart';

/// Backs the 24-hour clinical chat: `GET /appointments/:id/conversation`
/// (plan's gap #3, closed alongside this milestone — resolves which
/// conversation belongs to an appointment, the piece that was missing) and
/// the pre-existing `GET/POST /chat/:id/messages`. No message caching: a
/// stale offline chat history risks a patient replying to an already-expired
/// 24h window without realizing it.
class ChatRepository {
  ChatRepository(this.api, {this.connectivity});
  final ApiClient api;
  final ConnectivityService? connectivity;
  final _pendingMessages = <String, ({String content, String key})>{};
  Future<void> acknowledge(
      String appointment, String? conversation, String event,
      {List<String> messages = const [],
      DateTime? clientOccurredAt,
      CancelToken? cancelToken}) async {
    requireOnline();
    await api.post('/appointments/$appointment/trace-ack',
        cancelToken: cancelToken,
        data: {
          'event': event,
          'conversation_id': conversation,
          'message_ids': messages,
          if (clientOccurredAt != null)
            'client_occurred_at': clientOccurredAt.toUtc().toIso8601String(),
        });
  }

  Future<List<ChatMessageDto>> newerMessages(
      String conversation, ChatMessageDto after) async {
    requireOnline();
    final raw = _listData(_data(await api.get('/chat/$conversation/messages',
        query: {
          'after_time': after.sentAt.toUtc().toIso8601String(),
          'after_id': after.id
        })));
    return raw
        .map(
            (e) => ChatMessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  Future<List<ChatMessageDto>> olderMessages(
      String conversation, ChatMessageDto before) async {
    requireOnline();
    final raw = _listData(_data(await api.get('/chat/$conversation/messages',
        query: {
          'before_time': before.sentAt.toUtc().toIso8601String(),
          'before_id': before.id
        })));
    return raw
        .map(
            (e) => ChatMessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  bool get isOffline => connectivity?.isOffline == true;

  void requireOnline() {
    if (isOffline) {
      throw const ApiException(
          null, 'Internet connection is required for this action.');
    }
  }

  dynamic _data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  List<dynamic> _listData(dynamic value) {
    if (value is List) return value;
    throw const FormatException('Unexpected response shape.');
  }

  /// Throws a 404 `ApiException` if the appointment has no conversation yet
  /// — real before `CONFIRMED` (the conversation is only created once
  /// payment is confirmed; see `AppointmentService::process_payment_webhook`)
  /// — callers should show this as "chat opens once your appointment is
  /// confirmed," not a generic error.
  Future<ChatConversationDto> conversationForAppointment(
      String appointmentId) async {
    try {
      requireOnline();
      final raw =
          _data(await api.get('/appointments/$appointmentId/conversation'));
      return ChatConversationDto.fromJson(
          Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Chat', operation: 'resolve_conversation');
      rethrow;
    }
  }

  Future<List<AppointmentSummaryDto>> consultationPage(
      {String? profileId, int offset = 0}) async {
    requireOnline();
    final raw =
        _listData(_data(await api.get('/patients/me/appointments', query: {
      'limit': 100,
      'offset': offset,
      if (profileId != null) 'profile_id': profileId,
    })));
    return raw
        .map((e) =>
            AppointmentSummaryDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  /// On-demand lookup for notification navigation; never a polling source.
  Future<List<AppointmentSummaryDto>> consultations({String? profileId}) async {
    final result = <AppointmentSummaryDto>[];
    for (var offset = 0;; offset += 100) {
      final page = await consultationPage(profileId: profileId, offset: offset);
      result.addAll(page);
      if (page.length < 100) return result;
    }
  }

  Future<Uint8List> clinicalImage(
      String conversationId, String messageId) async {
    requireOnline();
    final raw = _data(await api
        .get('/chat/$conversationId/messages/$messageId/attachment')) as Map;
    return base64Decode(raw['base64_content'] as String);
  }

  Future<List<ChatMessageDto>> messages(String conversationId) async {
    try {
      requireOnline();
      final raw =
          _listData(_data(await api.get('/chat/$conversationId/messages')));
      return raw
          .map((e) =>
              ChatMessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Chat', operation: 'load_messages');
      rethrow;
    }
  }

  /// `idempotent: true` — a retried double-submit of the same tap must not
  /// post the message twice.
  Future<ChatMessageDto> sendMessage(
      String conversationId, String content) async {
    try {
      requireOnline();
      if (_pendingMessages[conversationId]?.content != content) {
        final key = List.generate(
            16,
            (_) => Random.secure()
                .nextInt(256)
                .toRadixString(16)
                .padLeft(2, '0')).join();
        _pendingMessages[conversationId] = (content: content, key: key);
      }
      final raw = _data(await api.post('/chat/$conversationId/messages',
          idempotencyKey: _pendingMessages[conversationId]!.key,
          data: {'content': content}));
      _pendingMessages.remove(conversationId);
      return ChatMessageDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Chat', operation: 'send_message');
      rethrow;
    }
  }
}
