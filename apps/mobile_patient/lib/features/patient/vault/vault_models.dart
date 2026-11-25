import '../../../core/l10n/patient_display.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? _optionalText(dynamic value) => value?.toString();

/// Mirrors one entry of `GET /patients/me/documents`
/// (`patient_handlers::list_own_documents`) — a merged, chronological view
/// over doctor-issued e-Prescriptions and patient-uploaded intake documents
/// (plan's gap #2, M7). `kind` distinguishes the two; prescription-only
/// fields (`rxNumber`, `doctorDisplayName`, `primarySpecialty`) are `null`
/// for an intake entry and vice versa for `pageNumber`'s absence — both
/// kinds share `id`/`appointmentId`/`createdAt`/`mimeType`.
class PatientDocumentDto {
  const PatientDocumentDto({
    required this.kind,
    required this.id,
    required this.appointmentId,
    required this.createdAt,
    this.mimeType,
    this.pageNumber,
    this.rxNumber,
    this.doctorDisplayName,
    this.primarySpecialty,
    this.appointmentNumber,
    this.modality,
    this.consultationStart,
    this.corrections = const [],
  });

  /// The doctor's correction notes on this prescription (a sent
  /// prescription cannot change; the doctor adds a note instead).
  final List<PrescriptionCorrection> corrections;
  final String kind, id, appointmentId;
  final DateTime createdAt;
  final String? mimeType, rxNumber, doctorDisplayName, primarySpecialty;
  final int? pageNumber;

  /// The consultation this document belongs to (both kinds), so Health
  /// Records can group documents by consultation.
  final String? appointmentNumber, modality;
  final DateTime? consultationStart;

  bool get isPrescription => kind == 'PRESCRIPTION';

  factory PatientDocumentDto.fromJson(Json j) => PatientDocumentDto(
        kind: _textOf(j['kind']),
        id: _textOf(j['id']),
        appointmentId: _textOf(j['appointment_id']),
        createdAt: DateTime.tryParse(_textOf(j['created_at']))?.toLocal() ??
            DateTime.fromMillisecondsSinceEpoch(0),
        mimeType: _optionalText(j['mime_type']),
        pageNumber: int.tryParse(_textOf(j['page_number'])),
        rxNumber: _optionalText(j['rx_number']),
        doctorDisplayName: j['doctor_display_name'] == null
            ? null
            : patientDoctorName(j['doctor_display_name'].toString()),
        primarySpecialty: _optionalText(j['primary_specialty']),
        appointmentNumber: _optionalText(j['appointment_number']),
        modality: _optionalText(j['modality']),
        consultationStart:
            DateTime.tryParse(_textOf(j['consultation_start']))?.toUtc(),
        corrections: [
          if (j['corrections'] is List)
            for (final c in j['corrections'] as List)
              if (c is Map && _textOf(c['note']).isNotEmpty)
                PrescriptionCorrection(
                    note: _textOf(c['note']),
                    createdAt:
                        DateTime.tryParse(_textOf(c['created_at']))?.toLocal()),
        ],
      );
}

/// One correction note the doctor added to a sent prescription.
class PrescriptionCorrection {
  const PrescriptionCorrection({required this.note, this.createdAt});
  final String note;
  final DateTime? createdAt;
}
