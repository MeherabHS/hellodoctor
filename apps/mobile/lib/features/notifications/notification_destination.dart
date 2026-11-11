import '../../core/notifications/notification_taps.dart';
import 'notification_repository.dart';

enum NotificationDestinationKind {
  inbox,
  appointment,
  chat,
  article,
  doctorArticle
}

class NotificationDestination {
  const NotificationDestination(this.kind, {this.id, this.data = const {}});
  final NotificationDestinationKind kind;
  final String? id;
  final Map<String, dynamic> data;

  /// Canonical, owner-scoped inbox lookup first; then an independently
  /// authorized destination read. No payload URL or cached ownership claim
  /// can authorize a clinical route. All reads remain non-mutating.
  static Future<NotificationDestination> load(
      NotificationRepository repo, String? id,
      {required bool doctor}) async {
    const inbox = NotificationDestination(NotificationDestinationKind.inbox);
    if (!notificationUuid(id)) return inbox;
    final notice = await repo.detail(id!);
    if (notice.id != id) throw const FormatException('Notification mismatch');
    final target = notice.relatedEntityId;
    if (!notificationUuid(target)) return inbox;
    Future<Map<String, dynamic>> read(String path) async {
      final response = await repo.api.get(path);
      final data = Map<String, dynamic>.from(response.data['data'] as Map);
      if (data['id'] != target) {
        throw const FormatException('Destination mismatch');
      }
      return data;
    }

    const appointments = {
      'BOOKING_CONFIRMED',
      'BOOKING_RESCHEDULED',
      'BOOKING_HOLD_CREATED',
      'BOOKING_HOLD_ONE_MINUTE',
      'BOOKING_HOLD_EXPIRED',
      'CONSULTATION_REMINDER_15M',
      'CONSULTATION_READY',
      'CONSULTATION_REJOIN',
      'CONSULTATION_COMPLETED',
      'CONSULTATION_INTERRUPTED',
      'CONSULTATION_NEEDS_REVIEW',
      'CONSULTATION_RECOVERY_RESOLVED',
      'PRESCRIPTION_AVAILABLE',
    };
    if (notice.relatedEntityType == 'APPOINTMENT' &&
        appointments.contains(notice.type)) {
      final data = await read('/appointments/$target');
      if (data['id'] != target) {
        throw const FormatException('Appointment mismatch');
      }
      return NotificationDestination(NotificationDestinationKind.appointment,
          id: target, data: data);
    }
    if (notice.type == 'NEW_CHAT_MESSAGE' &&
        notice.relatedEntityType == 'CHAT') {
      final appointment = notice.metadata?['appointment_id']?.toString();
      if (!notificationUuid(appointment)) {
        throw const FormatException('Missing appointment');
      }
      final response =
          await repo.api.get('/appointments/$appointment/conversation');
      final data = Map<String, dynamic>.from(response.data['data'] as Map);
      if (data['id'] != target || data['appointment_id'] != appointment) {
        throw const FormatException('Conversation mismatch');
      }
      return NotificationDestination(NotificationDestinationKind.chat,
          id: target, data: data);
    }
    if (!doctor &&
        notice.type == 'NEW_ARTICLE_PUBLISHED' &&
        notice.relatedEntityType == 'DOCTOR_ARTICLE') {
      return NotificationDestination(NotificationDestinationKind.article,
          id: target, data: await read('/articles/$target'));
    }
    if (doctor &&
        notice.type == 'ARTICLE_STATUS_CHANGED' &&
        notice.relatedEntityType == 'DOCTOR_ARTICLE') {
      return NotificationDestination(NotificationDestinationKind.doctorArticle,
          id: target, data: await read('/doctor/articles/$target'));
    }
    // Campaigns currently carry no approved article destination contract.
    return inbox;
  }
}
