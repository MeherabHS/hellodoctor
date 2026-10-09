/// Returns the canonical E.164 form (+254XXXXXXXXX, exactly 9 national
/// digits) if [input] is a structurally valid Kenyan phone number, or null
/// if not. Mirrors
/// services/backend/src/domain/validation.rs::normalize_kenya_phone_number
/// exactly — keep both in sync if either changes.
///
/// Validated STRUCTURALLY (country code + length), never by requiring the
/// national number to start with a specific digit like 7 or 1 — Kenya's
/// numbering plan isn't a fixed prefix allowlist this app should hard-code,
/// and real reachability is proven by OTP delivery, not a prefix guess. See
/// docs/KENYAN_PHONE_NORMALIZATION_VERIFICATION.md.
String? normalizeKenyaPhone(String input) {
  final clean = input.trim().replaceAll(RegExp(r'[ \-()]'), '');
  String? national;
  if (clean.startsWith('+254')) {
    national = clean.substring(4);
  } else if (clean.startsWith('254')) {
    national = clean.substring(3);
  } else if (clean.startsWith('0')) {
    national = clean.substring(1);
  }
  if (national == null ||
      national.length != 9 ||
      !RegExp(r'^\d{9}$').hasMatch(national)) {
    return null;
  }
  return '+254$national';
}
