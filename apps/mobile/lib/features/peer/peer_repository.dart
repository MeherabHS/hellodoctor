import 'dart:convert';
import 'dart:typed_data';

import '../../core/network/api_client.dart';
import '../../core/video/join_eligibility.dart';

typedef Json = Map<String, dynamic>;

Json _map(Object? raw) => Map<String, dynamic>.from(raw as Map);
List<Json> _list(Object? raw) =>
    (raw is List ? raw : const []).whereType<Map>().map(_map).toList();
dynamic _data(dynamic response) => (response.data as Map)['data'];

String _text(Object? v) => v?.toString() ?? '';
DateTime? _time(Object? v) =>
    v == null ? null : DateTime.tryParse(v.toString())?.toLocal();

/// The signed-in Peer Navigator's own profile.
class PeerProfile {
  const PeerProfile(
      {required this.id,
      required this.fullName,
      required this.county,
      required this.approvalStatus,
      this.organization,
      this.bio,
      this.approvalNote});
  final String id, fullName, county, approvalStatus;
  final String? organization, bio, approvalNote;
  bool get approved => approvalStatus == 'APPROVED';
  factory PeerProfile.fromJson(Json j) => PeerProfile(
      id: _text(j['id']),
      fullName: _text(j['full_name']),
      county: _text(j['county']),
      approvalStatus: _text(j['approval_status']),
      organization: j['organization']?.toString(),
      bio: j['bio']?.toString(),
      approvalNote: j['approval_note']?.toString());
}

/// One support request as the peer sees it (no patient identity).
class PeerInboxItem {
  const PeerInboxItem(
      {required this.id,
      required this.patientName,
      required this.start,
      required this.end,
      required this.supportType,
      required this.topic,
      required this.status,
      this.ageRange,
      this.conversationId,
      this.callClosed = false});
  final String id, patientName, supportType, topic, status;

  /// Server-decided: the session's call window has passed or the call was
  /// hung up, so no call can be started for it.
  final bool callClosed;
  final String? ageRange, conversationId;
  final DateTime start, end;
  bool get isCall => supportType == 'CALL';
  factory PeerInboxItem.fromJson(Json j) => PeerInboxItem(
      id: _text(j['id']),
      patientName: _text(j['patient_display_name']),
      ageRange: j['patient_age_range']?.toString(),
      start: _time(j['start_time']) ?? DateTime.now(),
      end: _time(j['end_time']) ?? DateTime.now(),
      supportType: _text(j['support_type']),
      topic: _text(j['topic']),
      status: _text(j['status']),
      conversationId: j['conversation_id']?.toString(),
      callClosed: j['call_closed'] == true);
}

class PeerSlot {
  const PeerSlot(
      {required this.id,
      required this.start,
      required this.end,
      required this.status});
  final String id, status;
  final DateTime start, end;
  factory PeerSlot.fromJson(Json j) => PeerSlot(
      id: _text(j['id']),
      start: _time(j['start_time']) ?? DateTime.now(),
      end: _time(j['end_time']) ?? DateTime.now(),
      status: _text(j['status']));
}

class PeerChatSummary {
  const PeerChatSummary(
      {required this.id,
      required this.requestId,
      required this.name,
      required this.supportType,
      required this.requestStatus,
      required this.start,
      required this.unread,
      this.lastMessage,
      this.lastMessageAt,
      this.callClosed = false});
  final String id, requestId, name, supportType, requestStatus;

  /// See [PeerInboxItem.callClosed].
  final bool callClosed;
  final String? lastMessage;
  final DateTime start;
  final DateTime? lastMessageAt;
  final int unread;
  factory PeerChatSummary.fromJson(Json j) => PeerChatSummary(
      id: _text(j['id']),
      requestId: _text(j['request_id']),
      name: _text(j['counterpart_name']),
      supportType: _text(j['support_type']),
      requestStatus: _text(j['request_status']),
      start: _time(j['start_time']) ?? DateTime.now(),
      lastMessage: j['last_message']?.toString(),
      lastMessageAt: _time(j['last_message_at']),
      unread: int.tryParse(_text(j['unread_count'])) ?? 0,
      callClosed: j['call_closed'] == true);
}

class PeerChatMessage {
  const PeerChatMessage(
      {required this.id,
      required this.senderId,
      required this.content,
      required this.sentAt,
      required this.hasImage,
      this.isRead = false});
  final String id, senderId, content;
  final DateTime sentAt;
  final bool hasImage;

  /// The other person has read it (double tick).
  final bool isRead;
  factory PeerChatMessage.fromJson(Json j) => PeerChatMessage(
      id: _text(j['id']),
      senderId: _text(j['sender_id']),
      content: _text(j['content']),
      sentAt: _time(j['sent_at']) ?? DateTime.now(),
      hasImage: j['attachment'] != null,
      isRead: j['is_read'] == true);
}

/// The Care app's Peer Navigator API (backend Phase 1-3).
class PeerRepository {
  PeerRepository(this.api);
  final ApiClient api;

  Future<PeerProfile> profile() async =>
      PeerProfile.fromJson(_map(_data(await api.get('/peer/profile'))));

  Future<PeerProfile> updateProfile(
          {String? fullName,
          String? county,
          String? organization,
          String? bio}) async =>
      PeerProfile.fromJson(_map(_data(await api.patch('/peer/profile', data: {
        if (fullName != null) 'full_name': fullName,
        if (county != null) 'county': county,
        if (organization != null) 'organization': organization,
        if (bio != null) 'bio': bio,
      }))));

  /// Saves the peer's own profile photo (JPEG or PNG); the server
  /// compresses it and stores it encrypted.
  Future<void> uploadPhoto(Uint8List bytes, String mime) async =>
      api.post('/peer/profile/photo',
          data: {'mime_type': mime, 'base64_content': base64Encode(bytes)});

  Future<void> removePhoto() async => api.delete('/peer/profile/photo');

  Future<List<PeerInboxItem>> requests({String? status}) async =>
      _list(_data(await api.get('/peer/requests',
              query: {if (status != null) 'status': status})))
          .map(PeerInboxItem.fromJson)
          .toList();

  Future<PeerInboxItem> decide(String id, String action,
          {String? note}) async =>
      PeerInboxItem.fromJson(_map(_data(await api
          .post('/peer/requests/$id/$action', data: {
        if (note != null && note.trim().isNotEmpty) 'note': note.trim()
      }))));

  Future<List<PeerSlot>> slots(DateTime from, DateTime to) async =>
      _list(_data(await api.get('/peer/slots', query: {
        'from': from.toUtc().toIso8601String(),
        'to': to.toUtc().toIso8601String(),
      })))
          .map(PeerSlot.fromJson)
          .toList();

  Future<PeerSlot> addSlot(DateTime start) async =>
      PeerSlot.fromJson(_map(_data(await api.post('/peer/slots',
          data: {'start_time': start.toUtc().toIso8601String()},
          idempotent: true))));

  Future<void> removeSlot(String id) async => api.delete('/peer/slots/$id');

  Future<List<PeerChatSummary>> chats() async =>
      _list(_data(await api.get('/peer-chats')))
          .map(PeerChatSummary.fromJson)
          .toList();

  Future<List<PeerChatMessage>> messages(String chatId) async =>
      _list(_data(await api.get('/peer-chats/$chatId/messages')))
          .map(PeerChatMessage.fromJson)
          .toList();

  Future<PeerChatMessage> send(String chatId, String content) async =>
      PeerChatMessage.fromJson(_map(_data(await api.post(
          '/peer-chats/$chatId/messages',
          data: {'content': content},
          idempotent: true))));

  /// One JPEG or PNG; the server compresses and stores it encrypted.
  Future<PeerChatMessage> sendImage(
          String chatId, Uint8List bytes, String mime, String key) async =>
      PeerChatMessage.fromJson(_map(_data(await api.post(
          '/peer-chats/$chatId/attachments',
          idempotencyKey: key,
          data: {'mime_type': mime, 'base64_content': base64Encode(bytes)}))));

  /// Image bytes, held in memory only (never a disk cache).
  Future<Uint8List> image(String chatId, String messageId) async {
    final raw = _map(_data(
        await api.get('/peer-chats/$chatId/messages/$messageId/attachment')));
    return base64Decode(raw['base64_content'] as String);
  }

  Future<void> markRead(String chatId) async =>
      api.post('/peer-chats/$chatId/read');

  /// Same JSON as the doctor join-eligibility endpoint, from `/call`.
  Future<JoinEligibility?> joinEligibility(String requestId) async {
    final raw = _map(_data(await api.get('/peer-requests/$requestId/call')));
    return JoinEligibility.tryParse(raw['join']);
  }

  Future<Json> rtcToken(String requestId) async =>
      _map(_data(await api.post('/peer-requests/$requestId/rtc-token')));

  /// The three current Peer documents shown at sign-up.
  Future<List<Json>> peerDocuments() async =>
      _list(_data(await api.get('/legal/documents')))
          .where((d) => _text(d['document_type']).startsWith('PEER_'))
          .toList();

  Future<void> register(
      {required String fullName,
      required String email,
      required String phone,
      required String password,
      required String county,
      String? organization,
      required List<Json> acceptedDocuments}) async {
    await api.post('/auth/peer/register', data: {
      'full_name': fullName,
      'email': email,
      'phone_number': phone,
      'password': password,
      'county': county,
      if (organization != null && organization.trim().isNotEmpty)
        'organization': organization.trim(),
      'accepted_documents': [
        for (final d in acceptedDocuments)
          {'type': d['document_type'], 'version': d['version']}
      ],
    });
  }
}
