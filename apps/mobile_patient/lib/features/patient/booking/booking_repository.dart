import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';
import '../../../core/network/connectivity_service.dart';
import '../../../core/validation/upload_validation.dart';
import 'booking_models.dart';
import 'join_eligibility.dart';

/// Backs `GET /doctors/:id/slots`, `POST /appointments/book`, and
/// `POST /appointments/:id/prescriptions` (patient intake upload) — all
/// three fully real today (plan §5/§5b). Deliberately has no cache reads:
/// slot availability and booking are both write-adjacent, time-sensitive
/// operations where a stale offline read would actively mislead a patient
/// into thinking a slot is still open.
class BookingRepository {
  BookingRepository(this.api, {this.connectivity});
  final ApiClient api;
  final ConnectivityService? connectivity;

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

  Future<List<DoctorSlotDto>> slots(String doctorId) async {
    try {
      requireOnline();
      final raw = _listData(_data(await api.get('/doctors/$doctorId/slots')));
      return raw
          .map((e) =>
              DoctorSlotDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList()
        ..sort((a, b) => a.startTime.compareTo(b.startTime));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Booking', operation: 'load_slots');
      rethrow;
    }
  }

  /// [modality]: `'VIDEO'` or `'CHAT'`. [profileId]: `null` books for the
  /// caller's own self profile; a dependent's id scopes the appointment to
  /// that dependent instead (§5b — `AuthContext::acting_patient_profile_id`).
  /// [paymentMethodId] must be a previously-saved `patient_payment_methods`
  /// row's id (see [paymentMethods]/[addMpesaPaymentMethod]) — the backend
  /// (`appointment_handlers::book_appointment`) resolves the real gateway
  /// from it server-side and re-validates ownership, so this app never needs
  /// to send a bare `gateway` string or re-collect payment details at
  /// booking time once a method is saved. `null` is only valid when
  /// [bookingPolicy] reported `paymentRequired == false` — the backend
  /// itself is the only authority on that, and rejects a `null` method when
  /// payment actually is required.
  Future<BookingResultDto> book({
    required String doctorId,
    required String slotId,
    required String modality,
    required String? paymentMethodId,
    String? chiefComplaint,
    String? profileId,
  }) async {
    try {
      requireOnline();
      final raw =
          _data(await api.post('/appointments/book', idempotent: true, data: {
        'doctor_id': doctorId,
        'slot_id': slotId,
        'modality': modality,
        if (paymentMethodId != null) 'payment_method_id': paymentMethodId,
        if (chiefComplaint != null && chiefComplaint.trim().isNotEmpty)
          'chief_complaint': chiefComplaint.trim(),
        if (profileId != null) 'profile_id': profileId,
      }));
      return BookingResultDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Booking', operation: 'book_appointment');
      rethrow;
    }
  }

  /// `GET /config/booking-policy` — the single server-controlled source of
  /// truth for whether a booking about to be made requires payment. Checked
  /// before the Fee/Payment-method UI is shown at all, never inferred
  /// client-side.
  Future<BookingPolicyDto> bookingPolicy() async {
    try {
      requireOnline();
      final raw = _data(await api.get('/config/booking-policy'));
      return BookingPolicyDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Booking', operation: 'load_booking_policy');
      rethrow;
    }
  }

  /// `GET /patients/me/payment-methods` — the patient's own saved payment
  /// instruments (scoped to [profileId] — `null` means the caller's own self
  /// profile, a dependent's id means that dependent's own saved methods;
  /// both are visible to whichever account guardians them, per
  /// `fn_patient_profile_owned_by_caller`).
  Future<List<PatientPaymentMethodDto>> paymentMethods(
      {String? profileId}) async {
    try {
      requireOnline();
      final raw = _listData(_data(await api.get('/patients/me/payment-methods',
          query: profileId != null ? {'profile_id': profileId} : null)));
      return raw
          .map((e) => PatientPaymentMethodDto.fromJson(
              Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Booking', operation: 'load_payment_methods');
      rethrow;
    }
  }

  /// `POST /patients/me/payment-methods` — saves a new M-Pesa number. The
  /// backend hard-stops (409) if the platform has no active M-Pesa provider
  /// configured yet (`payment_provider_verification::verify_patient_payment_method`);
  /// this call surfaces that honestly rather than pretending the method was
  /// verified.
  Future<PatientPaymentMethodDto> addMpesaPaymentMethod(String phoneNumber,
      {String? profileId}) async {
    try {
      requireOnline();
      final raw = _data(await api
          .post('/patients/me/payment-methods', idempotent: true, data: {
        'gateway': 'MPESA',
        'account_identifier': phoneNumber,
        if (profileId != null) 'profile_id': profileId,
      }));
      return PatientPaymentMethodDto.fromJson(
          Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Booking', operation: 'add_payment_method');
      rethrow;
    }
  }

  /// Resume an owned booking's existing attempt. No client-created checkout URL.
  Future<Uri> initiatePayment(String appointmentId) async {
    requireOnline();
    final raw = _data(await api.post('/appointments/$appointmentId/payment',
        idempotent: true));
    final uri = Uri.tryParse((raw as Map)['checkout_url']?.toString() ?? '');
    if (uri == null ||
        uri.scheme != 'https' ||
        uri.host.isEmpty ||
        uri.host.endsWith('.invalid')) {
      throw const ApiException(409,
          'Payment service is currently unavailable. Please try again later.');
    }
    return uri;
  }

  /// `POST /appointments/:id/prescriptions` — `idempotent: true` since a
  /// retried double-submit of the same file batch must not create duplicate
  /// documents. [files] must already satisfy [isValidIntakeImageCount] and
  /// use only [kAllowedIntakeMimeTypes]; size is the server's sole concern
  /// (see `core/validation/upload_validation.dart`).
  Future<int> uploadIntake(
      String appointmentId, List<Map<String, String>> files) async {
    try {
      requireOnline();
      final raw = _data(await api.post(
          '/appointments/$appointmentId/prescriptions',
          idempotent: true,
          data: {'files': files}));
      final map = Map<String, dynamic>.from(raw as Map);
      return int.tryParse(map['uploaded_count']?.toString() ?? '') ??
          files.length;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Booking', operation: 'upload_intake');
      rethrow;
    }
  }

  /// `GET /appointments/:id` — used by the Waiting Room to poll for a status
  /// change while a patient waits (e.g. cancelled elsewhere). No caching:
  /// a stale read here could mislead a patient into waiting for a
  /// consultation that no longer exists.
  Future<AppointmentDetailDto> appointment(String appointmentId) async {
    try {
      requireOnline();
      final raw = _data(await api.get('/appointments/$appointmentId'));
      return AppointmentDetailDto.fromJson(
          Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'WaitingRoom', operation: 'poll_appointment');
      rethrow;
    }
  }

  /// `POST /consultations/:id/telemetry/events` — real call-lifecycle
  /// telemetry (plan: "patient joined / disconnection / consultation start /
  /// finished"), the same event vocabulary and endpoint
  /// `ConsultationService::record_telemetry_events` already defines
  /// (`PATIENT_JOINED`, `PATIENT_DISCONNECTED`, `CALL_ENDED`, …) — this feeds
  /// the grievance system's own telemetry-based adjudication
  /// (`docs/STATE_MACHINES.md`'s `GrievanceState`: "Telemetry proves
  /// failure"). Requires a `consultation_sessions` row to already exist for
  /// this appointment, which only [mintRtcToken] creates — callers must mint
  /// before submitting events. Best-effort by design: a telemetry failure
  /// must never interrupt or fail the call experience itself, so every call
  /// site treats this as fire-and-forget, never awaited into a user-visible
  /// error path.
  Future<void> submitTelemetryEvents(
      String appointmentId, List<Map<String, dynamic>> events) async {
    try {
      requireOnline();
      await api.post('/consultations/$appointmentId/telemetry/events',
          data: {'events': events});
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Telehealth', operation: 'call_telemetry');
      // Intentionally swallowed, not rethrown — see doc comment above.
    }
  }

  /// `GET /consultations/:id/join-eligibility` — the server's authoritative,
  /// read-only answer to "may I enter this room now?" (same policy and
  /// authorization as the token mint; issues no token, creates no session).
  Future<JoinEligibility> joinEligibility(String appointmentId) async {
    try {
      requireOnline();
      final raw = _data(
          await api.get('/consultations/$appointmentId/join-eligibility'));
      final parsed = JoinEligibility.tryParse(raw);
      if (parsed == null) {
        throw const FormatException('Unexpected response shape.');
      }
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'WaitingRoom', operation: 'join_eligibility');
      rethrow;
    }
  }

  /// `POST /consultations/:id/rtc-token` — mirrors `apps/mobile`'s
  /// `DoctorRepository.rtcToken` exactly (same endpoint, same response
  /// shape). Only mints a session; never renders fake media (plan §8, real
  /// Agora join deferred).
  Future<RtcSessionDto> mintRtcToken(String appointmentId) async {
    try {
      requireOnline();
      final raw =
          _data(await api.post('/consultations/$appointmentId/rtc-token'));
      return RtcSessionDto.fromJson(Map<String, dynamic>.from(raw as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Telehealth', operation: 'mint_rtc_token');
      rethrow;
    }
  }
}
