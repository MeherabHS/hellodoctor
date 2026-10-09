use crate::error::AppError;
use chrono::{DateTime, Utc};

/// Active deployment accepts Kenyan phone numbers only — validated
/// STRUCTURALLY (country code `+254` plus exactly 9 national digits), never
/// by inferring a carrier from the leading national digit. A prior version
/// of this function required the national number to start with `7` or `1`
/// (an attempt to recognize "mobile" prefixes); that was a product mistake,
/// not a security control — Kenya's numbering plan isn't a fixed allowlist
/// this codebase should hard-code, and real reachability is proven by OTP
/// delivery/verification, not by a prefix guess. See
/// docs/KENYAN_PHONE_NORMALIZATION_VERIFICATION.md.
pub fn validate_phone_number(phone: &str) -> Result<(), AppError> {
    normalize_kenya_phone_number(phone).map(|_| ())
}

pub fn validate_kenya_phone_number(phone: &str) -> Result<(), AppError> {
    normalize_kenya_phone_number(phone).map(|_| ())
}

/// Accepts local (`0` + 9 digits), international (`+254` + 9 digits), or
/// bare international (`254` + 9 digits) forms, tolerating spaces/hyphens/
/// parentheses as presentation formatting. Rejects purely on country code
/// and national-number length (exactly 9 digits) — never on which digit the
/// national number starts with. Always returns the canonical `+254XXXXXXXXX`
/// form (13 characters) on success.
pub fn normalize_kenya_phone_number(phone: &str) -> Result<String, AppError> {
    let clean = phone.trim().replace([' ', '-', '(', ')'], "");
    let invalid = || {
        AppError::FieldValidation {
        field: "phone_number",
        code: "INVALID_KENYAN_PHONE",
        message: "Enter a Kenyan phone number with 9 digits after +254, e.g. +254 712 345 678 or 0712 345 678."
            .into(),
    }
    };
    let national = clean
        .strip_prefix("+254")
        .or_else(|| clean.strip_prefix("254"))
        .or_else(|| clean.strip_prefix('0'))
        .ok_or_else(invalid)?;
    if national.len() != 9 || !national.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid());
    }
    Ok(format!("+254{national}"))
}

pub fn validate_email(email: &str) -> Result<(), AppError> {
    let clean = email.trim();
    let invalid = || AppError::Validation("Enter a valid email address.".into());
    if clean.is_empty() || clean.chars().count() > 255 {
        return Err(invalid());
    }
    let mut parts = clean.split('@');
    let (local, domain) = match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => (local, domain),
        _ => return Err(invalid()),
    };
    if local.is_empty() || domain.len() < 3 {
        return Err(invalid());
    }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return Err(invalid());
    }
    // Require a genuine TLD shape: the label after the last `.` must be
    // alphabetic-only and at least 2 characters. Without this, a degenerate
    // domain like "a@b.c" (a single-character, non-alphabetic-checked "TLD")
    // previously passed — "proper domain names" per the user's own request.
    let tld = domain.rsplit('.').next().unwrap_or("");
    if tld.chars().count() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(invalid());
    }
    Ok(())
}

/// Minimum length for any doctor-set password. `auth_handlers::doctor_register`
/// previously had its own separate, stricter, hardcoded 12-128 character
/// check that ran *before* this function — meaning the 12-char minimum
/// silently won over this 8-char policy (12 > 8 always binds first),
/// directly contradicting the explicitly requested "8 characters with a
/// proper combination" policy. That redundant check was removed; this
/// function is now the single source of truth for Doctor password strength.
const MIN_PASSWORD_LENGTH: usize = 8;

/// Broad punctuation/symbol class for the password-strength special-character
/// requirement below — deliberately wide (not a narrow whitelist) so it
/// doesn't arbitrarily reject a password someone naturally chose.
const SPECIAL_CHARS: &str = "!@#$%^&*()_+-=[]{};:,.<>/?\\|~`'\"";

/// Doctor registration/password-setting strength check: at least
/// `MIN_PASSWORD_LENGTH` characters, with at least one uppercase letter, one
/// lowercase letter, one digit, and one special character. Deliberately has
/// no maximum-length cap — capping a password's maximum length is a known
/// anti-pattern (it limits, not strengthens, passphrase-style passwords),
/// and nothing else in this codebase does it either. Scoped to Doctor
/// self-service registration only — patient login is OTP-based (no password
/// at all) and admin accounts are seeded/managed, not self-service.
pub fn validate_password_strength(password: &str) -> Result<(), AppError> {
    let mut missing = Vec::new();
    if password.chars().count() < MIN_PASSWORD_LENGTH {
        missing.push(format!("at least {MIN_PASSWORD_LENGTH} characters"));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        missing.push("an uppercase letter".to_string());
    }
    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        missing.push("a lowercase letter".to_string());
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        missing.push("a digit".to_string());
    }
    if !password.chars().any(|c| SPECIAL_CHARS.contains(c)) {
        missing.push("a special character".to_string());
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Password must contain {}.",
            missing.join(", ")
        )))
    }
}

/// Shared free-text validator: trims, strips control characters other than
/// `\n`/`\r`/`\t` (defense against garbage/binary paste — not an XSS
/// concern; neither the Flutter app nor the admin-web/React UI renders raw
/// HTML anywhere, confirmed before this was written, so there is no
/// stored-XSS rendering vector here), and rejects anything over `max_len`
/// *characters* (not bytes, so multi-byte UTF-8 content isn't penalized).
/// Returns the cleaned value so callers can store the trimmed/sanitized
/// version rather than the raw input.
pub fn validate_text_field(
    value: &str,
    field_name: &str,
    max_len: usize,
) -> Result<String, AppError> {
    let cleaned: String = value
        .trim()
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .collect();
    if cleaned.chars().count() > max_len {
        return Err(AppError::Validation(format!(
            "{field_name} must not exceed {max_len} characters."
        )));
    }
    Ok(cleaned)
}

pub fn validate_license_number(license: &str) -> Result<(), AppError> {
    let clean = license.trim();
    if clean.is_empty() || clean.len() < 3 {
        Err(AppError::Validation(
            "KMPDC registration number is required and must be at least 3 characters.".into(),
        ))
    } else {
        Ok(())
    }
}

/// Pure syntax checks on the `accepted_documents` array a Doctor submits at
/// registration: known type strings, no duplicate type, non-empty version.
/// Whether the submitted set actually matches what's *currently* required is
/// a DB-backed question and stays inline in `auth_handlers::doctor_register`,
/// matching how that handler already checks duplicate license/phone/email
/// directly rather than inside this file.
pub fn validate_accepted_documents(submitted: &[(String, String)]) -> Result<(), AppError> {
    const KNOWN_TYPES: [&str; 5] = [
        "DOCTOR_TERMS",
        "PRIVACY_NOTICE",
        "PROFESSIONAL_VERIFICATION_DECLARATION",
        "TELEMEDICINE_PRACTITIONER_POLICY",
        "MARKETING_COMMUNICATIONS",
    ];
    let invalid = || {
        AppError::Validation(
            "Each accepted document requires a known document type and a non-empty version.".into(),
        )
    };
    if submitted.is_empty() {
        return Err(invalid());
    }
    for (doc_type, version) in submitted {
        if !KNOWN_TYPES.contains(&doc_type.as_str()) {
            return Err(invalid());
        }
        if version.trim().is_empty() || version.chars().count() > 20 {
            return Err(invalid());
        }
    }
    let mut seen = std::collections::HashSet::new();
    if !submitted.iter().all(|(doc_type, _)| seen.insert(doc_type)) {
        return Err(AppError::Validation(
            "Each document type may only be accepted once.".into(),
        ));
    }
    Ok(())
}

pub fn validate_slot_times(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), AppError> {
    if end <= start {
        return Err(AppError::Validation(
            "Slot end time must be strictly after slot start time.".into(),
        ));
    }
    if start < Utc::now() {
        return Err(AppError::Validation(
            "Slot start time cannot be in the past.".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod kenya_phone_tests {
    use super::*;
    #[test]
    fn kenya_numbers_are_normalized_and_reject_non_digits_or_wrong_country() {
        assert_eq!(
            normalize_kenya_phone_number("0711 884-920").unwrap(),
            "+254711884920"
        );
        assert_eq!(
            normalize_kenya_phone_number("+254111884920").unwrap(),
            "+254111884920"
        );
        // "+254211884920": national number "211884920" is a structurally
        // valid 9-digit Kenyan number starting with "2" — the prefix-agnostic
        // rule accepts it. This used to be in the rejection list under the
        // old (removed) 7/1-prefix restriction; that was the bug.
        assert_eq!(
            normalize_kenya_phone_number("+254211884920").unwrap(),
            "+254211884920"
        );
        for invalid in [
            "+8801711884920", // wrong country code
            "+25471188492",   // only 8 national digits
            "+25471188492a",  // non-digit in national number
            "07abcdefgh",     // non-digit local number
            "é254711884920",  // no recognized country/trunk prefix at all
        ] {
            assert!(normalize_kenya_phone_number(invalid).is_err(), "{invalid}");
        }
    }
}

#[cfg(test)]
mod phone_digit_tests {
    use super::*;

    #[test]
    fn rejects_right_shape_wrong_content_phone_numbers() {
        // Right prefix and length, but not actually all-digit — previously
        // slipped through since only the shape (prefix + length) was checked.
        assert!(validate_phone_number("+8801abcdefgh").is_err());
        assert!(validate_phone_number("0170000000a").is_err());
        assert!(validate_phone_number("+25471234567a").is_err());
    }

    #[test]
    fn accepts_kenya_numbers_and_rejects_legacy_country_numbers() {
        // Foreign-country inputs must not become Kenya accounts.
        assert!(validate_phone_number("+8801712345678").is_err());
        // "01712345678": stripping the leading local "0" leaves 10 national
        // digits, not 9 — rejected on length, not because "1" isn't 7/1-ish.
        assert!(validate_phone_number("01712345678").is_err());
        assert!(validate_phone_number("+254712345678").is_ok());
        assert!(validate_phone_number("0712345678").is_ok());
    }

    #[test]
    fn strips_parentheses_as_well_as_spaces_and_hyphens() {
        assert_eq!(
            normalize_kenya_phone_number("+254 (712) 345 678").unwrap(),
            "+254712345678"
        );
        assert_eq!(
            normalize_kenya_phone_number("0712-(345)-678").unwrap(),
            "+254712345678"
        );
    }

    #[test]
    fn rejects_by_length_not_by_leading_digit() {
        // +25420341371: the real bug report. National part "20341371" is
        // only 8 digits — rejected for LENGTH, never because it starts "20"
        // rather than "7"/"1". The prefix restriction that used to also
        // reject this is gone; length is the only remaining reason.
        assert!(normalize_kenya_phone_number("+25420341371").is_err());
        // The structurally-complete 9-digit version of the same leading
        // digits must now PASS — this is the prefix-agnostic proof.
        assert_eq!(
            normalize_kenya_phone_number("+254203413710").unwrap(),
            "+254203413710"
        );
        assert_eq!(
            normalize_kenya_phone_number("0203413710").unwrap(),
            "+254203413710"
        );
        // Bare "254" + 9 digits (no leading "+") is explicitly supported —
        // see docs/KENYAN_PHONE_NORMALIZATION_VERIFICATION.md.
        assert_eq!(
            normalize_kenya_phone_number("254712345678").unwrap(),
            "+254712345678"
        );
        // Other wrong-length shapes.
        assert!(normalize_kenya_phone_number("+2542034137100").is_err()); // 10 digits
        assert!(normalize_kenya_phone_number("071234567").is_err()); // 8 digits
        assert!(normalize_kenya_phone_number("07123456789").is_err()); // 10 digits
        assert!(normalize_kenya_phone_number("+254").is_err());
        assert!(normalize_kenya_phone_number("").is_err());
    }

    #[test]
    fn invalid_number_returns_field_validation_with_stable_code() {
        match normalize_kenya_phone_number("+25420341371") {
            Err(AppError::FieldValidation { field, code, .. }) => {
                assert_eq!(field, "phone_number");
                assert_eq!(code, "INVALID_KENYAN_PHONE");
            }
            other => panic!("expected AppError::FieldValidation, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod email_tld_tests {
    use super::*;

    #[test]
    fn rejects_degenerate_domains() {
        assert!(validate_email("a@b.c").is_err(), "single-char TLD");
        assert!(validate_email("a@b.c1").is_err(), "non-alphabetic TLD");
        assert!(validate_email("a@b.").is_err(), "empty TLD");
    }

    #[test]
    fn accepts_realistic_addresses() {
        for addr in [
            "admin@hellodoctor.health",
            "sabrina.akter@hellodoctor.health",
            "doctor@example.co.ke",
            "patient@example.com",
        ] {
            assert!(validate_email(addr).is_ok(), "{addr}");
        }
    }
}

#[cfg(test)]
mod password_strength_tests {
    use super::*;

    #[test]
    fn rejects_too_short() {
        assert!(validate_password_strength("Ab1!def").is_err()); // 7 chars
    }

    #[test]
    fn rejects_missing_uppercase() {
        assert!(validate_password_strength("abcdefg1!").is_err());
    }

    #[test]
    fn rejects_missing_lowercase() {
        assert!(validate_password_strength("ABCDEFG1!").is_err());
    }

    #[test]
    fn rejects_missing_digit() {
        assert!(validate_password_strength("Abcdefgh!").is_err());
    }

    #[test]
    fn rejects_missing_special_character() {
        assert!(validate_password_strength("Abcdefg1").is_err());
    }

    #[test]
    fn accepts_valid_password() {
        assert!(validate_password_strength("Abcdefg1!").is_ok());
    }

    #[test]
    fn does_not_cap_maximum_length() {
        let long_but_valid = format!("Ab1!{}", "x".repeat(200));
        assert!(validate_password_strength(&long_but_valid).is_ok());
    }
}

#[cfg(test)]
mod text_field_tests {
    use super::*;

    #[test]
    fn trims_and_accepts_within_limit() {
        assert_eq!(
            validate_text_field("  hello  ", "field", 10).unwrap(),
            "hello"
        );
    }

    #[test]
    fn rejects_over_limit_by_character_count_not_bytes() {
        // "é" is 2 bytes in UTF-8 but 1 character — the limit must be
        // character-based so multi-byte content isn't unfairly penalized.
        let value = "é".repeat(10);
        assert!(validate_text_field(&value, "field", 10).is_ok());
        assert!(validate_text_field(&value, "field", 9).is_err());
    }

    #[test]
    fn strips_control_characters_but_keeps_newlines_tabs() {
        let value = "line one\nline two\ttabbed\u{0007}bell";
        let cleaned = validate_text_field(value, "field", 100).unwrap();
        assert!(cleaned.contains('\n'));
        assert!(cleaned.contains('\t'));
        assert!(!cleaned.contains('\u{0007}'));
    }
}
