typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? _optionalText(dynamic value) => value?.toString();

/// Mirrors one row of the backend `notifications` table (see
/// `services/backend/src/domain/models.rs`'s `Notification` struct and
/// `services/backend/src/api/v1/notification_handlers.rs`) — the identical
/// contract `apps/mobile`'s `NotificationDto` already exercises for real.
/// The Rust struct renames its `notification_type` field to `type` only for
/// the *database column* (`#[sqlx(rename = "type")]`), with no matching
/// `#[serde(rename)]`, so the JSON key is `notification_type`, not `type`.
/// Deliberately never carries PHI: `title`/`body` are short backend
/// template strings, never symptoms, diagnosis, or prescription detail.
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

/// Mirrors `GET/PUT /notifications/preferences`
/// (`notification_handlers.rs`'s `NotificationPreferencesView`/
/// `UpdatePreferencesRequest`) — identical contract to `apps/mobile`'s
/// `NotificationPreferencesDto`. Quiet-hours times are plain "HH:MM" strings
/// on the wire — never raw `DateTime`/locale-dependent formatting.
class NotificationPreferencesDto {
  const NotificationPreferencesDto({
    required this.operationalEnabled,
    required this.engagementEnabled,
    required this.quietHoursEnabled,
    required this.quietHoursStartLocal,
    required this.quietHoursEndLocal,
    required this.greetingNotificationsEnabled,
    required this.religiousGreetingsEnabled,
    required this.preferredLanguage,
  });

  final bool operationalEnabled,
      engagementEnabled,
      quietHoursEnabled,
      greetingNotificationsEnabled,
      religiousGreetingsEnabled;
  final String quietHoursStartLocal, quietHoursEndLocal, preferredLanguage;

  factory NotificationPreferencesDto.fromJson(Json j) =>
      NotificationPreferencesDto(
        operationalEnabled: j['operational_enabled'] == true,
        engagementEnabled: j['engagement_enabled'] == true,
        quietHoursEnabled: j['quiet_hours_enabled'] == true,
        quietHoursStartLocal: _textOf(j['quiet_hours_start_local'], '22:00'),
        quietHoursEndLocal: _textOf(j['quiet_hours_end_local'], '07:00'),
        greetingNotificationsEnabled: j['greeting_notifications_enabled'] == true,
        religiousGreetingsEnabled: j['religious_greetings_enabled'] == true,
        preferredLanguage: _textOf(j['preferred_language'], 'en'),
      );

  NotificationPreferencesDto copyWith({
    bool? operationalEnabled,
    bool? engagementEnabled,
    bool? quietHoursEnabled,
    String? quietHoursStartLocal,
    String? quietHoursEndLocal,
    bool? greetingNotificationsEnabled,
    bool? religiousGreetingsEnabled,
    String? preferredLanguage,
  }) =>
      NotificationPreferencesDto(
        operationalEnabled: operationalEnabled ?? this.operationalEnabled,
        engagementEnabled: engagementEnabled ?? this.engagementEnabled,
        quietHoursEnabled: quietHoursEnabled ?? this.quietHoursEnabled,
        quietHoursStartLocal: quietHoursStartLocal ?? this.quietHoursStartLocal,
        quietHoursEndLocal: quietHoursEndLocal ?? this.quietHoursEndLocal,
        greetingNotificationsEnabled:
            greetingNotificationsEnabled ?? this.greetingNotificationsEnabled,
        religiousGreetingsEnabled:
            religiousGreetingsEnabled ?? this.religiousGreetingsEnabled,
        preferredLanguage: preferredLanguage ?? this.preferredLanguage,
      );
}
