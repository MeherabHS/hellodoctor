import 'dart:convert';
import 'dart:typed_data';

/// Pre-consultation intake upload contract — mirrors
/// `services/backend/src/services/prescription_service.rs`'s
/// `PATIENT_MAX_IMAGES_PER_CONSULTATION = 20` exactly, scoped per-appointment
/// (per-consultation), never lifetime/account-wide, never summed across
/// consultations, per CLAUDE.md's business rule.
const int kPatientMaxImagesPerConsultation = 20;
const int kPatientMinImagesPerConsultation = 1;

/// Deliberately does NOT replicate the backend's 40KB-per-image compression
/// math here. `services/backend/src/services/upload_security_service.rs`
/// already guarantees every accepted image independently fits
/// `MAX_PRESCRIPTION_IMAGE_BYTES = 40 * 1024` bytes server-side, and never
/// rejects an upload for being hard to compress — only for genuinely invalid
/// content (wrong magic bytes, dangerous PDF markers). Client-side logic only
/// needs to produce accurate "1 to 20 photos" copy and a count/mime
/// pre-check; the server is the sole source of truth on size.
const Set<String> kAllowedIntakeMimeTypes = {
  'image/jpeg',
  'image/png',
  'application/pdf',
};

/// Whether [count] selected files satisfy the per-consultation count bound.
bool isValidIntakeImageCount(int count) =>
    count >= kPatientMinImagesPerConsultation &&
    count <= kPatientMaxImagesPerConsultation;

/// How many of [count] selected files must be dropped (via
/// `removePatientPrescriptionImage`-equivalent UI, mirroring the prototype's
/// `#preConsultIntakeModal`) to fit the cap. Zero if already within bounds.
int excessIntakeImageCount(int count) =>
    count > kPatientMaxImagesPerConsultation
        ? count - kPatientMaxImagesPerConsultation
        : 0;

/// Builds one `{mime_type, base64_content}` entry for
/// `POST /appointments/:id/prescriptions`'s JSON body (NOT multipart — see
/// `services/backend/src/api/v1/prescription_handlers.rs`'s
/// `PatientIntakeUploadRequest`/`FileData`). Throws [ArgumentError] for a mime
/// type outside [kAllowedIntakeMimeTypes] rather than silently sending a
/// value the backend's magic-byte check would reject anyway.
Map<String, String> encodeIntakeFile({
  required String mimeType,
  required Uint8List bytes,
}) {
  if (!kAllowedIntakeMimeTypes.contains(mimeType)) {
    throw ArgumentError.value(
        mimeType, 'mimeType', 'Not an allowed intake image mime type.');
  }
  return {
    'mime_type': mimeType,
    'base64_content': base64Encode(bytes),
  };
}
