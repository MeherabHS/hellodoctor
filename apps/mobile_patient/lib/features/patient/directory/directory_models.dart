import '../../../core/l10n/patient_display.dart';
import '../../../core/utils/initials.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();

/// Mirrors `doctor_public_directory` as served by `GET /doctors` and
/// `GET /doctors/:id` (`services/backend/src/repository/doctor_repo.rs`'s
/// `PublicDoctorProfile`, plain `serde::Serialize` with no `rename_all` —
/// field names are already the Rust snake_case names verbatim). Every row
/// returned by these two endpoints already has
/// `professional_verification_status == 'VERIFIED'` (enforced in the SQL
/// itself), so this DTO does not re-derive a separate "is verified" flag.
class PublicDoctorDto {
  const PublicDoctorDto({
    required this.id,
    required this.fullName,
    required this.licenseNumber,
    required this.licenseAuthority,
    required this.licenseCountry,
    required this.primarySpecialty,
    required this.experienceYears,
    required this.currentHospital,
    required this.qualifications,
    required this.consultationFeeVideo,
    required this.consultationFeeChat,
    required this.isOnDuty,
    required this.hasPhoto,
    required this.photoUpdatedAt,
  });

  final String id,
      fullName,
      licenseNumber,
      licenseAuthority,
      licenseCountry,
      primarySpecialty,
      currentHospital,
      consultationFeeVideo,
      consultationFeeChat;
  final int experienceYears;
  final List<String> qualifications;
  final bool isOnDuty;

  /// Mirrors `profile_picture_size_bytes`/`profile_picture_updated_at` in
  /// `PublicDoctorProfile` (`services/backend/src/repository/doctor_repo.rs`)
  /// — a presence + cache-busting signal only; the actual bytes come from
  /// the separate authenticated `GET /doctors/:id/photo` endpoint.
  final bool hasPhoto;
  final DateTime? photoUpdatedAt;

  /// The patient app is payment-free (product decision, 2026-11-13): every
  /// doctor the server lists is approved and offers both video and chat,
  /// whatever fee is configured. Fees are never shown or used to decide
  /// availability; the server's directory decides who is listed, and booking
  /// refuses with a configuration error if the server ever requires payment.
  bool get supportsVideo => true;
  bool get supportsChat => true;
  bool videoBookable({required bool paymentRequired}) => true;

  String get displayName => patientDoctorName(fullName);

  /// Two-letter tile/avatar initials from the doctor's display name (e.g.
  /// "Dr. Sabrina Akter" → "SA") — the prototype's `.doc-tile-avatar` /
  /// `.profile-avatar` content, derived rather than hardcoded per doctor.
  String get initials => nameInitials(displayName);

  factory PublicDoctorDto.fromJson(Json j) => PublicDoctorDto(
        id: _textOf(j['id']),
        fullName: _textOf(j['full_name']),
        licenseNumber: _textOf(j['license_number']),
        licenseAuthority: _textOf(j['license_authority']),
        licenseCountry: _textOf(j['license_country']),
        primarySpecialty: _textOf(j['primary_specialty']),
        experienceYears: int.tryParse(_textOf(j['experience_years'], '0')) ?? 0,
        currentHospital: _textOf(j['current_hospital']),
        qualifications: (j['qualifications'] as List? ?? const [])
            .map((e) => e.toString())
            .toList(),
        consultationFeeVideo: _textOf(j['consultation_fee_video'], '0'),
        consultationFeeChat: _textOf(j['consultation_fee_chat'], '0'),
        isOnDuty: j['is_on_duty'] == true,
        hasPhoto: j['profile_picture_size_bytes'] != null,
        photoUpdatedAt:
            DateTime.tryParse(_textOf(j['profile_picture_updated_at'], '')),
      );
}
