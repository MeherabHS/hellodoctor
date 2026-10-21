typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? _optionalText(dynamic value) => value?.toString();

/// Mirrors `GET/PATCH /patients/me`, `GET /patients/me/profiles`, and
/// `POST /patients/me/profiles`'s responses
/// (`services/backend/src/api/v1/patient_handlers.rs`) — all three serialize
/// the same backend `PatientProfile` shape, self or dependent alike.
/// `weightKg` stays a `String?`, not a parsed `double` — the backend's
/// `BigDecimal` NUMERIC serialization can carry trailing zero-padding (e.g.
/// "62.5000"), so this DTO keeps the exact server string and lets the
/// editing UI parse/format it once, rather than risk a lossy double
/// round-trip through every read.
class PatientProfileDto {
  const PatientProfileDto({
    required this.id,
    required this.userId,
    required this.fullName,
    required this.dateOfBirth,
    required this.gender,
    this.bloodGroup,
    this.weightKg,
    this.emergencyContactPhone,
    this.emergencyContactRelation,
    this.guardianPatientId,
    this.relationship,
  });

  final String id, userId, fullName, dateOfBirth, gender;
  final String? bloodGroup,
      weightKg,
      emergencyContactPhone,
      emergencyContactRelation,
      guardianPatientId,
      relationship;

  /// True while the profile still matches the exact OTP-login placeholder
  /// (`auth_service::verify_otp_and_login` seeds `full_name: "Patient User"`,
  /// `date_of_birth: "1995-01-01"`, `gender: "OTHER"`) — the signal
  /// `profile_completion_page.dart` uses to decide whether to route a
  /// freshly signed-in patient into profile completion.
  bool get isPlaceholder =>
      fullName == 'Patient User' &&
      dateOfBirth == '1995-01-01' &&
      gender == 'OTHER';

  /// A dependent profile always has `user_id = NULL` and a non-null
  /// `guardian_patient_id` (`patient_profiles_self_xor_dependent`); a self
  /// profile is the reverse. Checking `guardianPatientId` (present on every
  /// row returned by `GET /patients/me/profiles`) distinguishes the two
  /// without needing a separate "is this the caller's own id" comparison.
  bool get isSelf => guardianPatientId == null;

  factory PatientProfileDto.fromJson(Json j) => PatientProfileDto(
        id: _textOf(j['id']),
        userId: _textOf(j['user_id']),
        fullName: _textOf(j['full_name']),
        dateOfBirth: _textOf(j['date_of_birth']),
        gender: _textOf(j['gender']),
        bloodGroup: _optionalText(j['blood_group']),
        weightKg: _optionalText(j['weight_kg']),
        emergencyContactPhone: _optionalText(j['emergency_contact_phone']),
        emergencyContactRelation:
            _optionalText(j['emergency_contact_relation']),
        guardianPatientId: _optionalText(j['guardian_patient_id']),
        relationship: _optionalText(j['relationship']),
      );
}
