import '../../../core/l10n/kenya_time.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();

/// Mirrors one row of `GET /doctors/:id/slots`
/// (`doctor_repo::list_slots_for_doctor`) — the endpoint only ever returns
/// rows with `status = 'AVAILABLE'` belonging to a verified doctor, so this
/// DTO carries no status field to branch on; a slot either appears here (and
/// is bookable) or it doesn't.
class DoctorSlotDto {
  const DoctorSlotDto({
    required this.id,
    required this.doctorId,
    required this.startTime,
    required this.endTime,
  });
  final String id, doctorId;
  final DateTime startTime, endTime;

  factory DoctorSlotDto.fromJson(Json j) => DoctorSlotDto(
        id: _textOf(j['id']),
        doctorId: _textOf(j['doctor_id']),
        startTime: kenyaTime(DateTime.tryParse(_textOf(j['start_time'])) ??
            DateTime.fromMillisecondsSinceEpoch(0, isUtc: true)),
        endTime: kenyaTime(DateTime.tryParse(_textOf(j['end_time'])) ??
            DateTime.fromMillisecondsSinceEpoch(0, isUtc: true)),
      );
}

/// Result of `POST /appointments/book`
/// (`appointment_handlers.rs::book_appointment`'s JSON body). `status` is
/// always `PENDING_PAYMENT` on a fresh booking — the backend only ever
/// confirms payment via a provider webhook
/// (`services/backend/src/api/v1/webhook_handlers.rs`), which this phase
/// does not implement a real M-Pesa integration for (plan §8, out of scope).
/// The UI must show this real pending state honestly, not fake a
/// "Confirmed" result.
class BookingResultDto {
  const BookingResultDto({
    required this.appointmentId,
    required this.appointmentNumber,
    required this.status,
    required this.amount,
    this.holdExpiresAt,
    this.serverTime,
  });
  final String appointmentId, appointmentNumber, status, amount;
  final DateTime? holdExpiresAt, serverTime;

  factory BookingResultDto.fromJson(Json j) => BookingResultDto(
        appointmentId: _textOf(j['appointment_id']),
        appointmentNumber: _textOf(j['appointment_number']),
        status: _textOf(j['status']),
        amount: _textOf(j['amount'], '0'),
        holdExpiresAt: DateTime.tryParse(_textOf(j['hold_expires_at']))?.toUtc(),
        serverTime: DateTime.tryParse(_textOf(j['server_time']))?.toUtc(),
      );
}

/// Mirrors `GET /appointments/:id` (`appointment_repo::find_by_id`'s minimal
/// `Appointment` struct) — used by the Waiting Room to poll for a status
/// change (e.g. the appointment completing or expiring while the patient
/// waits), not to re-derive doctor/time display data, which the caller
/// already has from wherever it navigated here (`AppointmentSummaryDto`,
/// booking's own result, etc.) — this endpoint has no join to doctor/slot
/// tables.
class AppointmentDetailDto {
  const AppointmentDetailDto({
    required this.id,
    required this.status,
    this.clinicalOutcome,
    this.completedAt,
    this.paymentStatus,
    this.holdExpiresAt,
    this.serverTime,
  });
  final String? paymentStatus;
  final DateTime? holdExpiresAt, serverTime;
  bool get isReady => const [
        'CONFIRMED',
        'WAITING',
        'IN_CONSULTATION',
        'COMPLETED'
      ].contains(status);
  final String id, status;
  final String? clinicalOutcome, completedAt;

  factory AppointmentDetailDto.fromJson(Json j) => AppointmentDetailDto(
        id: _textOf(j['id']),
        status: _textOf(j['status']),
        paymentStatus: j['payment_status']?.toString(),
        holdExpiresAt: DateTime.tryParse(_textOf(j['hold_expires_at']))?.toUtc(),
        serverTime: DateTime.tryParse(_textOf(j['server_time']))?.toUtc(),
        clinicalOutcome: j['clinical_outcome']?.toString(),
        completedAt: j['completed_at']?.toString(),
      );
}

/// Mirrors one row of `GET /patients/me/payment-methods`
/// (`patient_handlers::payment_method_json`) — a previously-saved payment
/// instrument the patient can pick at checkout instead of entering details
/// fresh every time (`services/backend/src/services/payment_provider_verification.rs`
/// requires the platform to have a real provider configured before any such
/// method can be saved at all, so every row here is already provider-checked).
class PatientPaymentMethodDto {
  const PatientPaymentMethodDto({
    required this.id,
    required this.gateway,
    required this.maskedDestination,
    required this.isPrimary,
  });
  final String id, gateway, maskedDestination;
  final bool isPrimary;

  factory PatientPaymentMethodDto.fromJson(Json j) => PatientPaymentMethodDto(
        id: _textOf(j['id']),
        gateway: _textOf(j['gateway']),
        maskedDestination: _textOf(j['masked_destination']),
        isPrimary: j['is_primary'] == true,
      );
}

/// Mirrors `POST /consultations/:id/rtc-token`'s response
/// (`consultation_handlers::mint_agora_token`) — the same contract Doctor's
/// `AgoraSession`/`agora_page.dart` already exercises for real.
/// Mirrors `POST /consultations/:id/rtc-token`'s full response —
/// `token`/`uid` are required for a real Agora join (not just `channelName`/
/// `appId`, which is all the old mint-then-placeholder flow ever needed).
class RtcSessionDto {
  const RtcSessionDto({
    required this.channelName,
    required this.appId,
    required this.token,
    required this.uid,
    required this.expiresInSeconds,
  });
  final String channelName, appId, token;
  final int uid, expiresInSeconds;

  factory RtcSessionDto.fromJson(Json j) {
    final uidValue = j['uid'];
    final expiresValue = j['expires_in'];
    return RtcSessionDto(
      channelName: _textOf(j['channel_name']),
      appId: _textOf(j['app_id']),
      token: _textOf(j['token']),
      uid: uidValue is int ? uidValue : int.tryParse(_textOf(uidValue)) ?? 0,
      expiresInSeconds: expiresValue is int
          ? expiresValue
          : int.tryParse(_textOf(expiresValue)) ?? 3600,
    );
  }
}

/// Client-submittable event vocabulary — mirrors
/// `ConsultationService::TELEMETRY_EVENT_TYPES` exactly (the DB `CHECK`
/// constraint on `consultation_telemetry_events.event_type`). Only the
/// subset this app can honestly observe at the current mint-then-placeholder
/// fidelity is used (see `video/agora_placeholder_page.dart`); the
/// reconnect/network-quality entries need a real Agora SDK session to
/// measure and are deliberately left unsent rather than faked.
abstract final class TelemetryEventType {
  static const callInitiated = 'CALL_INITIATED';
  static const patientJoined = 'PATIENT_JOINED';
  static const patientDisconnected = 'PATIENT_DISCONNECTED';
  static const patientReconnected = 'PATIENT_RECONNECTED';
  static const callEnded = 'CALL_ENDED';
}

/// One entry for `POST /consultations/:id/telemetry/events`'s `events` array
/// (`consultation_handlers::TelemetryEventInput`). `occurred_at` is always
/// the real client-observed UTC instant — this is what lets a "consultation
/// delayed" determination be made later (comparing this against the
/// appointment's scheduled slot time) without this app inventing a separate
/// "delay" field the backend has no concept of.
Map<String, dynamic> telemetryEvent(String eventType, DateTime occurredAt) => {
      'event_type': eventType,
      'occurred_at': occurredAt.toUtc().toIso8601String(),
    };
