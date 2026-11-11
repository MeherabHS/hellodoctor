import '../../../core/l10n/patient_display.dart';

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

  // Matches DoctorQuery: a positive configured fee enables that modality.
  bool get supportsVideo => _positiveFee(consultationFeeVideo);
  bool get supportsChat => _positiveFee(consultationFeeChat);

  /// Whether VIDEO can be booked. When the server's booking policy says no
  /// payment is required (`FINANCIAL_MODULE_MODE=deferred`) an unpriced doctor
  /// is still bookable — the deferred booking endpoint never charges the fee,
  /// so a zero fee must not hide the modality. In paid mode the configured
  /// fee remains the offering signal, exactly as before. CHAT stays
  /// fee-driven (its booking flow is payment-specific).
  bool videoBookable({required bool paymentRequired}) =>
      !paymentRequired || supportsVideo;
  static bool _positiveFee(String fee) {
    final value = double.tryParse(fee);
    return value != null && value.isFinite && value > 0;
  }

  String get displayName => patientDoctorName(fullName);

  /// Two-letter tile/avatar initials from the doctor's display name (e.g.
  /// "Dr. Sabrina Akter" → "SA") — the prototype's `.doc-tile-avatar` /
  /// `.profile-avatar` content, derived rather than hardcoded per doctor.
  String get initials {
    final words = displayName
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
