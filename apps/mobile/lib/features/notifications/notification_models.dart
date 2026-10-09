typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? _optionalText(dynamic value) => value?.toString();

/// Mirrors one row of the backend `notifications` table (see
/// `services/backend/src/domain/models.rs`'s `Notification` struct and
/// `services/backend/src/api/v1/notification_handlers.rs`). The Rust struct
/// renames its `notification_type` field to `type` only for the *database
/// column* (`#[sqlx(rename = "type")]`) — there is no `#[serde(rename)]`
/// alongside it, so the JSON key this client actually receives is
/// `notification_type`, not `type`. Deliberately never carries PHI: `title`/
/// `body` are short backend-constructed template strings (never symptoms,
/// diagnosis, prescription detail, or patient-identifying clinical content).
class NotificationDto {
  const NotificationDto({
    required this.id,
    required this.userId,
    required this.type,
    required this.title,
    required this.body,
    required this.createdAt,
    required this.deliveryStatus,
    this.relatedEntityType,
    this.relatedEntityId,
    this.scheduledFor,
    this.deliveredAt,
    this.readAt,
    this.metadata,
  });

  final String id, userId, type, title, body, deliveryStatus;
  final String? relatedEntityType, relatedEntityId;
  final DateTime createdAt;
  final DateTime? scheduledFor, deliveredAt, readAt;
  final Json? metadata;

  bool get isRead => readAt != null;

  factory NotificationDto.fromJson(Json j) => NotificationDto(
      id: _textOf(j['id']),
      userId: _textOf(j['user_id']),
      type: _textOf(j['notification_type'] ?? j['type']),
      title: _textOf(j['title']),
      body: _textOf(j['body']),
      deliveryStatus: _textOf(j['delivery_status']),
      relatedEntityType: _optionalText(j['related_entity_type']),
      relatedEntityId: _optionalText(j['related_entity_id']),
      createdAt: DateTime.tryParse(_textOf(j['created_at']))?.toUtc() ??
          DateTime.fromMillisecondsSinceEpoch(0, isUtc: true),
      scheduledFor: DateTime.tryParse(_textOf(j['scheduled_for'])),
      deliveredAt: DateTime.tryParse(_textOf(j['delivered_at'])),
      readAt: DateTime.tryParse(_textOf(j['read_at'])),
      metadata: j['metadata'] is Map
          ? Map<String, dynamic>.from(j['metadata'] as Map)
          : null);

  NotificationDto copyWith({DateTime? readAt}) => NotificationDto(
      id: id,
      userId: userId,
      type: type,
      title: title,
      body: body,
      deliveryStatus: deliveryStatus,
      relatedEntityType: relatedEntityType,
      relatedEntityId: relatedEntityId,
      createdAt: createdAt,
      scheduledFor: scheduledFor,
      deliveredAt: deliveredAt,
      readAt: readAt ?? this.readAt,
      metadata: metadata);
}
