/// Emergency contact configuration for the patient Home screen's
/// "Emergency Contact" banner (`prototype/index.html`'s
/// `#patientEmergencyHelplineStrip`).
///
/// The backend has no configuration endpoint for this (confirmed by direct
/// backend research — no env var, config field, or route serves an emergency
/// number anywhere in `services/backend/src`). The prototype hardcodes a
/// Bangladesh number (`tel:16263`) that does not apply to this Kenya
/// deployment, so this value must be a real, product/legal-confirmed Kenya
/// contact before shipping — `999` is Kenya's standard national emergency
/// number, used here as a reasonable default pending that confirmation, not
/// as a verified HelloDoctor-specific helpline.
///
/// CLAUDE.md requires this to be configurable per country for future
/// expansion beyond Kenya; keeping it as a single named constant (rather than
/// scattering the literal across widgets) is the seam that allows that later.
abstract final class EmergencyContact {
  static const String kenyaNumber = '999';
  static const String kenyaTelUri = 'tel:$kenyaNumber';
}
