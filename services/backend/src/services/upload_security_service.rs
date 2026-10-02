use crate::error::AppError;
use image::ImageFormat;
use std::io::Cursor;

pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024; // 10 MB

pub struct UploadSecurityService;

impl UploadSecurityService {
    /// Validates size + magic bytes, and for image types, fully decodes and
    /// re-encodes the image (stripping EXIF/other metadata) so what gets stored
    /// is guaranteed to be a genuine, sanitized image rather than a spoofed payload.
    /// Returns the sanitized bytes to persist.
    pub fn validate_and_sanitize(bytes: &[u8], mime_type: &str) -> Result<Vec<u8>, AppError> {
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(AppError::PayloadTooLarge("Uploaded file exceeds maximum limit of 10 MB.".into()));
        }

        if bytes.len() < 4 {
            return Err(AppError::UnprocessableEntity("Uploaded file is empty or corrupt.".into()));
        }

        let is_jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
        let is_png = bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]);
        let is_pdf = bytes.starts_with(&[0x25, 0x50, 0x44, 0x46]);

        if is_pdf {
            return Ok(bytes.to_vec());
        }

        if !is_jpeg && !is_png {
            return Err(AppError::UnprocessableEntity(
                "Invalid file content. Must be a valid JPEG, PNG, or PDF.".into(),
            ));
        }

        let format = if is_jpeg { ImageFormat::Jpeg } else { ImageFormat::Png };
        let decoded = image::load_from_memory_with_format(bytes, format)
            .map_err(|_| AppError::UnprocessableEntity("File claims to be an image but could not be decoded.".into()))?;

        let mut sanitized = Cursor::new(Vec::new());
        decoded
            .write_to(&mut sanitized, format)
            .map_err(|e| AppError::Internal(format!("Failed to re-encode sanitized image: {e}")))?;

        let _ = mime_type;
        Ok(sanitized.into_inner())
    }
}
