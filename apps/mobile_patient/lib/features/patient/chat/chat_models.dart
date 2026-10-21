import '../../../core/l10n/kenya_time.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();

/// Mirrors `GET /appointments/:id/conversation`
/// (`chat_repo::ChatConversation`, plain snake-case JSON, no `rename_all`) —
/// every `CONFIRMED` appointment gets one of these regardless of modality
/// (confirmed directly in `AppointmentService::process_payment_webhook`: the
/// 24-hour chat is bundled with a Video booking too, not only a Chat one).
class ChatConversationDto {
  const ChatConversationDto({
    required this.id,
    required this.appointmentId,
    required this.expiresAt,
    required this.isLocked,
  });
  final String id, appointmentId;
  final DateTime expiresAt;
  final bool isLocked;

  /// A conversation can be unexpired by its own `expires_at` yet the server
  /// hasn't lazily flipped `is_locked` yet (it's only set on the next failed
  /// send — see `ChatService::send_message`). Checking both client-side
  /// gives an immediate, honest "this is closed" state instead of waiting
  /// for a send to fail first.
  bool get isExpired => isLocked || !DateTime.now().toUtc().isBefore(expiresAt);

  factory ChatConversationDto.fromJson(Json j) => ChatConversationDto(
        id: _textOf(j['id']),
        appointmentId: _textOf(j['appointment_id']),
        expiresAt: DateTime.tryParse(_textOf(j['expires_at']))?.toUtc() ??
            DateTime.fromMillisecondsSinceEpoch(0, isUtc: true),
        isLocked: j['is_locked'] == true,
      );
}

/// Mirrors one row of `GET /chat/:id/messages` (`chat_repo::ChatMessage`).
/// `senderId` is a `users.id`, not a `patient_profiles.id` — comparing it
/// against the signed-in account's own `userId` is how the UI decides which
/// side of the thread a bubble renders on (`ChatMessage.sender_id` is FK'd to
/// `users` directly, confirmed in `chat_handlers.rs`'s own comment).
class ChatMessageDto {
  const ChatMessageDto({
    required this.id,
    required this.senderId,
    required this.content,
    required this.sentAt,
    this.attachmentId,
    this.attachmentSize,
  });
  final String id, senderId, content;
  final String? attachmentId;
  final int? attachmentSize;
  final DateTime sentAt;

  factory ChatMessageDto.fromJson(Json j) => ChatMessageDto(
        id: _textOf(j['id']),
        senderId: _textOf(j['sender_id']),
        attachmentId: j['attachment_id']?.toString(),
        attachmentSize: (j['attachment'] as Map?)?['size_bytes'] as int?,
        content: _textOf(j['content']),
        sentAt: kenyaTime(DateTime.tryParse(_textOf(j['sent_at'])) ??
            DateTime.fromMillisecondsSinceEpoch(0, isUtc: true)),
      );
}
