use crate::error::AppError;
use chrono::{DateTime, Utc};

pub fn validate_phone_number(phone: &str) -> Result<(), AppError> {
    let clean = phone.trim().replace([' ', '-'], "");
    // Bangladesh format: +8801[3-9]XXXXXXXX (14 chars) or 01[3-9]XXXXXXXX (11 chars)
    // Kenya format: +254[17]XXXXXXXX (13 chars) or 0[17]XXXXXXXX (10 chars)
    let is_bd = clean.starts_with("+8801") && clean.len() == 14
        || clean.starts_with("01") && clean.len() == 11;
    let is_ke = clean.starts_with("+254") && (clean.len() == 13 || clean.len() == 12)
        || (clean.starts_with("07") || clean.starts_with("01")) && clean.len() == 10;

    if is_bd || is_ke {
        Ok(())
    } else {
        Err(AppError::Validation(
            "Invalid mobile phone number format for target regions (Bangladesh +880 or Kenya +254).".into(),
        ))
    }
}

pub fn validate_license_number(license: &str) -> Result<(), AppError> {
    let clean = license.trim();
    if clean.is_empty() || clean.len() < 3 {
        Err(AppError::Validation(
            "Medical license number is required and must be at least 3 characters.".into(),
        ))
    } else {
        Ok(())
    }
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
