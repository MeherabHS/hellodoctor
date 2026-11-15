import 'dart:convert';
import 'dart:typed_data';

import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import '../booking/booking_models.dart';
import '../booking/join_eligibility.dart';

typedef Json = Map<String, dynamic>;

Json _map(Object? raw) => Map<String, dynamic>.from(raw as Map);
List<Json> _list(Object? raw) =>
    (raw is List ? raw : const []).whereType<Map>().map(_map).toList();
String _text(Object? v) => v?.toString() ?? '';
DateTime _time(Object? v) =>
    DateTime.tryParse(_text(v))?.toLocal() ??
    DateTime.fromMillisecondsSinceEpoch(0);

/// An approved Peer Navigator a patient can ask for support.
class PeerNavigatorDto {
  const PeerNavigatorDto(
      {required this.id,
      required this.fullName,
      required this.county,
      this.organization,
      this.bio,
      this.nextAvailable});
  final String id, fullName, county;
  final String? organization, bio;
  final DateTime? nextAvailable;
  factory PeerNavigatorDto.fromJson(Json j) => PeerNavigatorDto(
      id: _text(j['id']),
      fullName: _text(j['full_name']),
      county: _text(j['county']),
      organization: j['organization']?.toString(),
      bio: j['bio']?.toString(),
      nextAvailable:
          j['next_available'] == null ? null : _time(j['next_available']));
}

class PeerOpenSlotDto {
  const PeerOpenSlotDto(
      {required this.id, required this.start, required this.end});
  final String id;
  final DateTime start, end;
  factory PeerOpenSlotDto.fromJson(Json j) => PeerOpenSlotDto(
      id: _text(j['id']),
      start: _time(j['start_time']),
      end: _time(j['end_time']));
}

/// One of the patient's own support requests.
class PatientPeerRequestDto {
  const PatientPeerRequestDto(
      {required this.id,
      required this.peerId,
      required this.peerName,
      required this.start,
      required this.end,
      required this.supportType,
      required this.topic,
      required this.status,
      this.declineNote,
      this.conversationId});
  final String id, peerId, peerName, supportType, topic, status;
  final String? declineNote, conversationId;
  final DateTime start, end;
  bool get isCall => supportType == 'CALL';
  bool get isPending => status == 'PENDING';
  factory PatientPeerRequestDto.fromJson(Json j) => PatientPeerRequestDto(
      id: _text(j['id']),
      peerId: _text(j['peer_id']),
      peerName: _text(j['peer_name']),
      start: _time(j['start_time']),
      end: _time(j['end_time']),
      supportType: _text(j['support_type']),
      topic: _text(j['topic']),
      status: _text(j['status']),
      declineNote: j['decline_note']?.toString(),
      conversationId: j['conversation_id']?.toString());
}

class PeerChatSummaryDto {
  const PeerChatSummaryDto(
      {required this.id,
      required this.requestId,
      required this.peerName,
      required this.supportType,
      required this.requestStatus,
      required this.start,
      required this.unread,
      this.lastMessage,
      this.lastMessageAt});
  final String id, requestId, peerName, supportType, requestStatus;
  final String? lastMessage;
  final DateTime start;
  final DateTime? lastMessageAt;
  final int unread;
  factory PeerChatSummaryDto.fromJson(Json j) => PeerChatSummaryDto(
      id: _text(j['id']),
      requestId: _text(j['request_id']),
      peerName: _text(j['counterpart_name']),
      supportType: _text(j['support_type']),
      requestStatus: _text(j['request_status']),
      start: _time(j['start_time']),
      lastMessage: j['last_message']?.toString(),
      lastMessageAt:
          j['last_message_at'] == null ? null : _time(j['last_message_at']),
      unread: int.tryParse(_text(j['unread_count'])) ?? 0);
}

class PeerChatMessageDto {
  const PeerChatMessageDto(
      {required this.id,
      required this.senderId,
      required this.content,
      required this.sentAt,
      required this.hasImage,
      this.isRead = false});
  final String id, senderId, content;
  final DateTime sentAt;
  final bool hasImage;

  /// The other party has read it (shown as ticks on the patient's own).
  final bool isRead;
  factory PeerChatMessageDto.fromJson(Json j) => PeerChatMessageDto(
      id: _text(j['id']),
      senderId: _text(j['sender_id']),
      content: _text(j['content']),
      sentAt: _time(j['sent_at']),
      hasImage: j['attachment'] != null,
      isRead: j['is_read'] == true);
}

/// The Patient app's Peer Navigator API (backend Phase 1-3).
class PatientPeerRepository {
  PatientPeerRepository(this.api, {this.connectivity});
  final ApiClient api;
  final ConnectivityService? connectivity;

  dynamic _data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  void _online() {
    if (connectivity?.isOffline == true) {
      throw const ApiException(
          null, 'Internet connection is required for this action.');
    }
  }

  Future<T> _run<T>(String op, Future<T> Function() body) async {
    try {
      _online();
      return await body();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'PeerNavigator', operation: op);
      rethrow;
    }
  }

  Future<List<PeerNavigatorDto>> peers() => _run(
      'peers',
      () async => _list(_data(await api.get('/peers')))
          .map(PeerNavigatorDto.fromJson)
          .toList());

  Future<List<PeerOpenSlotDto>> openSlots(String peerId) => _run(
      'peer_slots',
      () async => _list(_data(await api.get('/peers/$peerId/slots')))
          .map(PeerOpenSlotDto.fromJson)
          .toList());

  Future<PatientPeerRequestDto> request(
          {required String slotId,
          required String supportType,
          required String topic}) =>
      _run(
          'peer_request',
          () async => PatientPeerRequestDto.fromJson(_map(_data(
                  await api.post('/peer-requests', idempotent: true, data: {
                'slot_id': slotId,
                'support_type': supportType,
                'topic': topic.trim(),
              })))));

  Future<List<PatientPeerRequestDto>> myRequests() => _run(
      'peer_requests',
      () async => _list(_data(await api.get('/peer-requests')))
          .map(PatientPeerRequestDto.fromJson)
          .toList());

  Future<PatientPeerRequestDto> cancel(String requestId) => _run(
      'peer_request_cancel',
      () async => PatientPeerRequestDto.fromJson(
          _map(_data(await api.post('/peer-requests/$requestId/cancel')))));

  Future<List<PeerChatSummaryDto>> chats() => _run(
      'peer_chats',
      () async => _list(_data(await api.get('/peer-chats')))
          .map(PeerChatSummaryDto.fromJson)
          .toList());

  Future<List<PeerChatMessageDto>> messages(String chatId) => _run(
      'peer_messages',
      () async => _list(_data(await api.get('/peer-chats/$chatId/messages')))
          .map(PeerChatMessageDto.fromJson)
          .toList());

  Future<PeerChatMessageDto> send(String chatId, String content) => _run(
      'peer_send',
      () async => PeerChatMessageDto.fromJson(_map(_data(await api.post(
          '/peer-chats/$chatId/messages',
          idempotent: true,
          data: {'content': content})))));

  /// One JPEG or PNG; the server compresses and stores it encrypted.
  Future<PeerChatMessageDto> sendImage(
          String chatId, Uint8List bytes, String mime, String key) =>
      _run(
          'peer_send_image',
          () async => PeerChatMessageDto.fromJson(_map(_data(await api.post(
                  '/peer-chats/$chatId/attachments',
                  idempotencyKey: key,
                  data: {
                    'mime_type': mime,
                    'base64_content': base64Encode(bytes)
                  })))));

  /// Image bytes, held in memory only (never a disk cache).
  Future<Uint8List> image(String chatId, String messageId) => _run(
      'peer_image',
      () async => base64Decode(_map(_data(await api
              .get('/peer-chats/$chatId/messages/$messageId/attachment')))[
          'base64_content'] as String));

  Future<void> markRead(String chatId) async {
    await api.post('/peer-chats/$chatId/read');
  }

  /// When the call room opens, from `GET /peer-requests/:id/call` (the same
  /// join-window JSON as a doctor consultation).
  Future<DateTime?> callOpensAt(String requestId) async {
    final raw = _map(_data(await api.get('/peer-requests/$requestId/call')));
    return JoinEligibility.tryParse(raw['join'])?.joinOpenAt;
  }

  Future<RtcSessionDto> rtcToken(String requestId) async =>
      RtcSessionDto.fromJson(
          _map(_data(await api.post('/peer-requests/$requestId/rtc-token'))));
}
