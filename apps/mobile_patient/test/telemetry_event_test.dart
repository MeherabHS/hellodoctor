import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';

void main() {
  group('telemetryEvent', () {
    test(
        'builds the exact shape ConsultationService::NewTelemetryEvent expects',
        () {
      final occurredAt = DateTime.utc(2026, 10, 10, 8, 30);
      final event =
          telemetryEvent(TelemetryEventType.patientJoined, occurredAt);
      expect(event, {
        'event_type': 'PATIENT_JOINED',
        'occurred_at': '2026-10-10T08:30:00.000Z',
      });
    });

    test('always normalizes occurred_at to UTC regardless of input', () {
      final local = DateTime(2026, 10, 10, 8, 30).toLocal();
      final event = telemetryEvent(TelemetryEventType.callEnded, local);
      expect(event['occurred_at'], endsWith('Z'));
    });
  });

  group('TelemetryEventType', () {
    test('values match the backend CHECK constraint vocabulary exactly', () {
      // services/backend/src/services/consultation_service.rs::TELEMETRY_EVENT_TYPES
      const backendVocabulary = {
        'CALL_INITIATED',
        'PATIENT_JOINED',
        'DOCTOR_JOINED',
        'PATIENT_RECONNECTING',
        'PATIENT_RECONNECTED',
        'PATIENT_DISCONNECTED',
        'DOCTOR_RECONNECTING',
        'DOCTOR_RECONNECTED',
        'DOCTOR_DISCONNECTED',
        'NETWORK_DEGRADED',
        'NETWORK_RESTORED',
        'CALL_ENDED',
      };
      const usedByThisApp = {
        TelemetryEventType.callInitiated,
        TelemetryEventType.patientJoined,
        TelemetryEventType.patientDisconnected,
        TelemetryEventType.patientReconnected,
        TelemetryEventType.callEnded,
      };
      for (final value in usedByThisApp) {
        expect(backendVocabulary, contains(value));
      }
    });
  });
}
