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
    this.clinicalOutcome,
    this.completedAt,
    this.fee,
    this.chatExpiresAt,
    this.chatLocked,
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
  final String? clinicalOutcome, completedAt, fee;

  factory AppointmentSummaryDto.fromJson(Json j) => AppointmentSummaryDto(
        id: _textOf(j['id']),
        appointmentNumber: _textOf(j['appointment_number']),
        doctorId: _textOf(j['doctor_id']),
        doctorDisplayName: patientDoctorName(_textOf(j['doctor_display_name'])),
        primarySpecialty: _textOf(j['primary_specialty']),
        modality: _textOf(j['modality']),
        status: _textOf(j['status']),
        fee: _optionalText(j['consultation_fee']),
        chatExpiresAt: DateTime.tryParse(_textOf(j['chat_expires_at'])),
        chatLocked: j['chat_locked'] as bool?,
        startTime: DateTime.tryParse(_textOf(j['start_time']))?.toLocal() ??
            DateTime.fromMillisecondsSinceEpoch(0),
        endTime: DateTime.tryParse(_textOf(j['end_time']))?.toLocal() ??
            DateTime.fromMillisecondsSinceEpoch(0),
        clinicalOutcome: _optionalText(j['clinical_outcome']),
        completedAt: _optionalText(j['completed_at']),
      );
}
