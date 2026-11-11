import '../../../core/l10n/patient_display.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? _optionalText(dynamic value) => value?.toString();

/// Mirrors one row of `GET /patients/me/appointments`
/// (`services/backend/src/api/v1/patient_handlers.rs::list_own_appointments`).
class AppointmentSummaryDto {
  const AppointmentSummaryDto({
    required this.id,
    required this.appointmentNumber,
    required this.doctorId,
    required this.doctorDisplayName,
    required this.primarySpecialty,
    required this.modality,
    required this.status,
    required this.startTime,
    required this.endTime,
    this.hasPhoto = false,
    this.photoUpdatedAt,
    this.clinicalOutcome,
    this.completedAt,
    this.fee,
    this.paymentPolicy = 'REQUIRED',
    this.chatExpiresAt,
    this.chatLocked,
    this.consultationSessionStatus,
    this.consultationRecoveryState,
    this.emptyRoomDeadline,
    this.patientPresent,
    this.doctorPresent,
  });

  final String id,
      appointmentNumber,
      doctorId,
      doctorDisplayName,
      primarySpecialty,
      modality,
      status;
  final DateTime startTime, endTime;
  final DateTime? chatExpiresAt;
  final bool? chatLocked;
  final String? consultationSessionStatus, consultationRecoveryState;
  final DateTime? emptyRoomDeadline;
  final bool? patientPresent, doctorPresent;
  final bool hasPhoto;
  final DateTime? photoUpdatedAt;

  /// Mirrors `PublicDoctorDto.initials` (`directory_models.dart`) — kept as
  /// its own copy rather than a shared mixin since the two DTOs otherwise
  /// have no relationship and this is the only field they'd share.
  String get doctorInitials {
    final words = doctorDisplayName
        .replaceAll('Dr.', '')
        .trim()
        .split(RegExp(r'\s+'))
        .where((w) => w.isNotEmpty)
        .toList();
    if (words.isEmpty) return '?';
    if (words.length == 1) return words.first.substring(0, 1).toUpperCase();
    return (words.first.substring(0, 1) + words.last.substring(0, 1))
        .toUpperCase();
  }

  bool get hasActiveConsultation =>
      modality == 'VIDEO' &&
      status == 'IN_CONSULTATION' &&
      consultationRecoveryState != 'NEEDS_REVIEW' &&
      const ['INITIALIZED', 'ACTIVE'].contains(consultationSessionStatus);
  bool get isWaitingForReconnect =>
      hasActiveConsultation && consultationRecoveryState == 'GRACE_PERIOD';
  final String? clinicalOutcome, completedAt, fee;
  final String paymentPolicy;
  bool get paymentRequired => paymentPolicy != 'NOT_REQUIRED';

  factory AppointmentSummaryDto.fromJson(Json j) => AppointmentSummaryDto(
        id: _textOf(j['id']),
        appointmentNumber: _textOf(j['appointment_number']),
        doctorId: _textOf(j['doctor_id']),
        doctorDisplayName: patientDoctorName(_textOf(j['doctor_display_name'])),
        primarySpecialty: _textOf(j['primary_specialty']),
        hasPhoto: j['profile_picture_size_bytes'] != null,
        photoUpdatedAt: DateTime.tryParse(
            _textOf(j['profile_picture_updated_at'], '')),
        modality: _textOf(j['modality']),
        status: _textOf(j['status']),
        fee: _optionalText(j['consultation_fee']),
        paymentPolicy: _textOf(j['payment_policy'], 'REQUIRED'),
        chatExpiresAt: DateTime.tryParse(_textOf(j['chat_expires_at'])),
        chatLocked: j['chat_locked'] as bool?,
        consultationSessionStatus:
            _optionalText(j['consultation_session_status']),
        consultationRecoveryState:
            _optionalText(j['consultation_recovery_state']),
        emptyRoomDeadline:
            DateTime.tryParse(_textOf(j['empty_room_deadline']))?.toLocal(),
        patientPresent: j['patient_present'] as bool?,
        doctorPresent: j['doctor_present'] as bool?,
        startTime: DateTime.tryParse(_textOf(j['start_time']))?.toLocal() ??
            DateTime.fromMillisecondsSinceEpoch(0),
        endTime: DateTime.tryParse(_textOf(j['end_time']))?.toLocal() ??
            DateTime.fromMillisecondsSinceEpoch(0),
        clinicalOutcome: _optionalText(j['clinical_outcome']),
        completedAt: _optionalText(j['completed_at']),
      );
}
