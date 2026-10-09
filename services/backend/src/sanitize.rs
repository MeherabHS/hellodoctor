//! Server-side sanitization for free-text fields. Client-side validation (Flutter,
//! admin-web) is a UX convenience, not a security boundary — every free-text field
//! that ends up in the database goes through this first.

use crate::error::AppError;

/// Trims the input, rejects NUL bytes and non-whitespace control characters, and
/// enforces a hard max length — returning a clear 422 rather than silently truncating
/// or letting Postgres reject it with a raw constraint-violation error.
pub fn sanitize_text(input: &str, max_len: usize) -> Result<String, AppError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "This field cannot be empty.".into(),
        ));
    }

    if trimmed.chars().count() > max_len {
        return Err(AppError::UnprocessableEntity(format!(
            "This field must be {max_len} characters or fewer."
        )));
    }

    if trimmed
        .chars()
        .any(|c| c == '\0' || (c.is_control() && c != '\n' && c != '\r' && c != '\t'))
    {
        return Err(AppError::UnprocessableEntity(
            "This field contains characters that are not allowed.".into(),
        ));
    }

    Ok(trimmed.to_string())
}
