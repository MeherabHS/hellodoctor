use crate::error::AppError;
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Maximum long-edge dimension for a stored prescription/intake image.
/// New requirement (not previously implemented — every image was stored at
/// its original, uncompressed resolution and quality): a modern phone camera
/// photo of an A4/A5 prescription page is commonly 3000-4000px on the long
/// edge and several megabytes; none of that extra resolution is needed to
/// read handwritten or printed prescription text, which stays fully legible
/// well below this cap. Chosen generously (not minimally) specifically to
/// protect legibility — this is a size/bandwidth optimization, not an
/// aggressive compression target.
const MAX_IMAGE_DIMENSION: u32 = 2000;

/// Per-image compression ceiling — product rule, not a shared batch budget:
/// EVERY accepted prescription/Rx image has its OWN independent maximum
/// normalized size, never divided by how many images are in the same
/// submission. A prior version of this constant (`TOTAL_BATCH_BUDGET_BYTES`,
/// 400 KB) was a BATCH-wide budget split across however many images were
/// uploaded together (e.g. a 5-image submission got 80 KB each) — that
/// "total ÷ count" shape was a product mistake, not a safety requirement; see
/// docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md. 40 KiB
/// (40 * 1024 = 40,960 bytes), matching this codebase's existing
/// decimal-multiplier-of-1024 convention for byte constants (see
/// `MAX_UPLOAD_BYTES` above).
pub const MAX_PRESCRIPTION_IMAGE_BYTES: usize = 40 * 1024;

/// Final design decision: the per-image budget is a GUARANTEE, not a
/// best-effort target — "the idea is system will auto compress whatever is
/// uploaded into [budget]... no rejection or nothing." An upload is never
/// rejected for being hard to compress, and the stored result must never
/// exceed the budget. Two earlier, tighter attempts at a "legibility floor"
/// (quality 40 / 800px) were explicitly removed because they could let
/// compression stop early while still over budget — the only floors that
/// remain are the bottom-of-the-barrel technical minimums below, which exist
/// solely to avoid asking the encoder for a degenerate/invalid result, not to
/// protect image quality. In the ordinary case (real prescription photos,
/// which compress far better than this file's deliberately-adversarial
/// worst-case test fixture) these technical floors are never reached at all
/// — see the empirical sizes in this file's tests.
const ABSOLUTE_MIN_JPEG_QUALITY: u8 = 1;

/// Absolute floor for the long-edge dimension this service will downscale an
/// image to while guaranteeing a byte budget — small enough that encoding is
/// trivially tiny regardless of content (an 8x8 image is at most a few
/// hundred bytes even as uncompressed PNG), so the budget is always
/// reachable before this floor would ever bind in practice. Not "1" purely
/// to stay comfortably clear of any degenerate single-pixel edge case in the
/// resize/encode pipeline.
const ABSOLUTE_MIN_IMAGE_DIMENSION: u32 = 8;

/// Byte-level markers for PDF active-content/interactivity features that have
/// no legitimate place in a scanned medical document photo/report. This is a
/// structural check, not a substitute for real antivirus/malware scanning —
/// no such integration (ClamAV or otherwise) exists in this codebase yet;
/// this is the honest, currently-implemented layer, not a claim of a fuller
/// one. A simple byte-subsequence scan can in principle false-positive on a
/// coincidental match inside compressed stream data, but for these markers to
/// be *functional* they must appear as real, uncompressed dictionary keys in
/// the PDF's object/xref structure — an acceptable tradeoff for the kind of
/// document this endpoint actually receives (scanned lab reports/photos),
/// not a general-purpose PDF security product.
const PDF_DANGEROUS_MARKERS: &[&[u8]] = &[
    b"/JavaScript",
    b"/JS",
    b"/OpenAction",
    b"/AA",
    b"/Launch",
    b"/EmbeddedFile",
    b"/RichMedia",
    b"/SubmitForm",
    b"/ImportData",
];

pub struct UploadSecurityService;

fn decoder_semaphore() -> std::sync::Arc<tokio::sync::Semaphore> {
    static DECODERS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    DECODERS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
}

struct SanitizeWork;
impl SanitizeWork {
    fn start() -> Self {
        metrics::gauge!("upload_sanitize_active_workers").increment(1.0);
        metrics::counter!("upload_sanitize_operations_total").increment(1);
        Self
    }
}
impl Drop for SanitizeWork {
    fn drop(&mut self) {
        metrics::gauge!("upload_sanitize_active_workers").decrement(1.0);
    }
}

impl UploadSecurityService {
    /// Every image is compressed toward the same fixed per-image budget
    /// (`MAX_PRESCRIPTION_IMAGE_BYTES`) regardless of how many images are
    /// being submitted together in the same logical request — the patient
    /// pre-consultation intake upload (now up to 20 images) and the doctor's
    /// one-photo-per-call Rx upload both call this the same way.
    pub async fn sanitize_async(bytes: Vec<u8>, mime: String) -> Result<Vec<u8>, AppError> {
        // Reject cheap size/MIME/magic failures before reserving a decoder or
        // scheduling blocking work. Full decoding remains inside the bulkhead.
        Self::validated_format(&bytes, &mime)?;
        let permit = decoder_semaphore().try_acquire_owned().map_err(|_| {
            AppError::RateLimitExceeded(
                "Too many uploads in progress. Please try again shortly.".into(),
            )
        })?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _work = SanitizeWork::start();
            Self::validate_and_sanitize(&bytes, &mime)
        })
        .await
        .map_err(|_| AppError::internal("Upload validation failed.".into()))?
    }
    /// Validates size + magic bytes, and for image types, fully decodes and
    /// re-encodes the image (stripping EXIF/other metadata) so what gets stored
    /// is guaranteed to be a genuine, sanitized image rather than a spoofed payload.
    /// Returns the sanitized bytes to persist.
    ///
    /// A file that fails validation is rejected outright (never stored) — the
    /// `security_status` column every call site persists alongside this is
    /// therefore always `'CLEAN'` for any row that exists today, by
    /// construction. That column exists for a *future* async re-scan path,
    /// not because this function can itself produce anything else yet.
    pub fn validate_and_sanitize(bytes: &[u8], mime_type: &str) -> Result<Vec<u8>, AppError> {
        let Some(format) = Self::validated_format(bytes, mime_type)? else {
            Self::validate_pdf_structure(bytes)?;
            return Ok(bytes.to_vec());
        };
        let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        reader.limits(limits);
        let decoded = reader.decode().map_err(|_| {
            AppError::UnprocessableEntity(
                "File claims to be an image but could not be decoded.".into(),
            )
        })?;

        let decoded = Self::downscale_if_needed(decoded, MAX_IMAGE_DIMENSION);
        let sanitized = Self::compress_to_target(decoded, format, MAX_PRESCRIPTION_IMAGE_BYTES)?;

        if sanitized.len() > MAX_UPLOAD_BYTES {
            return Err(AppError::PayloadTooLarge(
                "Sanitized image exceeds maximum limit of 10 MB.".into(),
            ));
        }
        Ok(sanitized)
    }

    fn validated_format(bytes: &[u8], mime_type: &str) -> Result<Option<ImageFormat>, AppError> {
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(AppError::PayloadTooLarge(
                "Uploaded file exceeds maximum limit of 10 MB.".into(),
            ));
        }

        if bytes.len() < 4 {
            return Err(AppError::UnprocessableEntity(
                "Uploaded file is empty or corrupt.".into(),
            ));
        }

        let is_jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
        let is_png = bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]);
        let is_pdf = bytes.starts_with(&[0x25, 0x50, 0x44, 0x46]);
        let expected_mime = if is_pdf {
            "application/pdf"
        } else if is_png {
            "image/png"
        } else {
            "image/jpeg"
        };
        if mime_type != expected_mime {
            return Err(AppError::UnprocessableEntity(
                "File MIME type does not match its content.".into(),
            ));
        }

        if is_pdf {
            return Ok(None);
        }

        if !is_jpeg && !is_png {
            return Err(AppError::UnprocessableEntity(
                "Invalid file content. Must be a valid JPEG, PNG, or PDF.".into(),
            ));
        }

        Ok(Some(if is_jpeg {
            ImageFormat::Jpeg
        } else {
            ImageFormat::Png
        }))
    }

    /// Downscales an image whose long edge exceeds `max_dim`, preserving
    /// aspect ratio; a no-op for images already within bounds (the common
    /// case for anything that isn't a raw phone-camera photo — e.g. the tiny
    /// fixture images this file's own tests use).
    fn downscale_if_needed(img: DynamicImage, max_dim: u32) -> DynamicImage {
        if img.width() <= max_dim && img.height() <= max_dim {
            return img;
        }
        img.resize(max_dim, max_dim, image::imageops::FilterType::Lanczos3)
    }

    /// Compresses `img` toward `target_bytes`, combining two levers in order:
    /// (1) for JPEG, lowering encoder quality down to
    /// `ABSOLUTE_MIN_JPEG_QUALITY`; (2) once quality (or PNG's lack of a
    /// quality knob) is exhausted, progressively downscaling the image
    /// further and retrying, down to `ABSOLUTE_MIN_IMAGE_DIMENSION`. This is
    /// a hard guarantee, not a best-effort target — the upload must never be
    /// rejected for being hard to compress, and the result must never exceed
    /// `target_bytes` for any input the two technical floors can still act
    /// on (which is effectively all realistic input: an image at the
    /// absolute dimension floor encodes to at most a few hundred bytes
    /// regardless of content, so the budget is always reachable before
    /// either floor actually binds). The floors exist only to avoid asking
    /// the encoder for a degenerate/invalid result (quality 0, a near-zero
    /// resize), not to protect image quality — a pathological input is
    /// allowed to come out looking rough in the extreme worst case; it must
    /// never come out over budget.
    fn compress_to_target(
        mut img: DynamicImage,
        format: ImageFormat,
        target_bytes: usize,
    ) -> Result<Vec<u8>, AppError> {
        loop {
            let encoded = match format {
                ImageFormat::Jpeg => {
                    Self::encode_jpeg_within_target(&img, target_bytes, ABSOLUTE_MIN_JPEG_QUALITY)?
                }
                ImageFormat::Png => Self::encode_png_compressed(&img)?,
                _ => unreachable!("only Jpeg/Png images reach this branch, checked above"),
            };

            let long_edge = img.width().max(img.height());
            if encoded.len() <= target_bytes || long_edge <= ABSOLUTE_MIN_IMAGE_DIMENSION {
                return Ok(encoded);
            }

            // Step the long edge down by half per round (a geometric, not
            // linear, reduction converges in a handful of iterations even
            // from MAX_IMAGE_DIMENSION all the way down to the absolute
            // floor) — each round re-runs the full JPEG quality loop, so
            // round count directly drives latency; a faster `Triangle`
            // resample is used here too, since by this point quality has
            // already been sacrificed to chase the byte target and resample
            // fidelity is no longer the bottleneck.
            let next_long_edge = ((long_edge as f32) * 0.5) as u32;
            let next_long_edge = next_long_edge.max(ABSOLUTE_MIN_IMAGE_DIMENSION);
            img = img.resize(
                next_long_edge,
                next_long_edge,
                image::imageops::FilterType::Triangle,
            );
        }
    }

    /// Encodes as JPEG at a single resolution, iteratively lowering quality
    /// until the result fits `target_bytes` or `min_quality` (the absolute
    /// technical floor) is reached — whichever comes first. Starts at
    /// quality 80 (comparable to the unconditional default this whole scheme
    /// replaced) so an already-small image/target typically succeeds on the
    /// first pass without any extra quality loss.
    fn encode_jpeg_within_target(
        img: &DynamicImage,
        target_bytes: usize,
        min_quality: u8,
    ) -> Result<Vec<u8>, AppError> {
        let rgb = img.to_rgb8();
        let mut quality: u8 = 80;
        loop {
            let mut buf = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality)
                .encode_image(&rgb)
                .map_err(|e| {
                    AppError::internal(format!("Failed to re-encode sanitized image: {e}"))
                })?;
            if buf.len() <= target_bytes || quality <= min_quality {
                return Ok(buf);
            }
            quality = quality.saturating_sub(15).max(min_quality);
        }
    }

    /// Encodes as PNG at maximum compression, at whatever resolution `img`
    /// currently is. PNG is lossless, so there is no quality knob to trade
    /// against size the way JPEG has — resolution (driven by the caller,
    /// `compress_to_target`) is the only size lever for PNG. Deliberately
    /// does not convert format (PNG stays PNG) — the mime/content-type
    /// recorded alongside the stored bytes elsewhere in the pipeline
    /// (`prescription_service.rs`) is set from the original upload's
    /// declared type, so silently changing the encoded format here would
    /// desynchronize the two.
    fn encode_png_compressed(img: &DynamicImage) -> Result<Vec<u8>, AppError> {
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut buf,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        img.write_with_encoder(encoder)
            .map_err(|e| AppError::internal(format!("Failed to re-encode sanitized image: {e}")))?;
        Ok(buf)
    }

    /// Rejects encrypted PDFs (opaque to this check, so treated as untrusted
    /// rather than passed through blindly) and PDFs containing active-content
    /// dictionary keys. Checks the real file structure, not the client-
    /// supplied `Content-Type` header.
    ///
    /// Matching runs against the name-escape-*normalized* bytes
    /// (`normalize_pdf_name_escapes`), not the raw file bytes: PDF name
    /// objects permit `#xx` hex-character escaping (ISO 32000-1 §7.3.5), and
    /// a marker written as `/J#61vaScript` or `/En#63rypt` is spec-valid and
    /// decodes identically to the unescaped form in any conforming parser,
    /// but previously never matched this literal-byte scan
    /// (h03-pdf-dangerous-marker-hex-escape-bypass). This remains a
    /// structural heuristic, not a substitute for real antivirus/malware
    /// scanning — no such integration exists in this codebase.
    fn validate_pdf_structure(bytes: &[u8]) -> Result<(), AppError> {
        let normalized = normalize_pdf_name_escapes(bytes);
        if contains_subsequence(&normalized, b"/Encrypt") {
            return Err(AppError::UnprocessableEntity(
                "Encrypted PDFs are not accepted.".into(),
            ));
        }
        for marker in PDF_DANGEROUS_MARKERS {
            if contains_subsequence(&normalized, marker) {
                return Err(AppError::UnprocessableEntity(
                    "This PDF contains active content (scripts, actions, or embedded files) and cannot be accepted.".into(),
                ));
            }
        }
        Ok(())
    }
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Decodes PDF name-object `#xx` hex-character escapes (ISO 32000-1 §7.3.5)
/// within name tokens only (a token starting at `/` and running until a PDF
/// delimiter/whitespace byte) — not arbitrary `#xx` sequences anywhere in the
/// file, so this stays bounded to actual name-token syntax rather than
/// blindly decoding file-wide. Output is never longer than the input (every
/// decode step replaces 3 input bytes with 1 output byte; everything else is
/// copied 1:1), so this performs a single bounded linear pass with no
/// unbounded allocation. A malformed escape (a `#` not followed by two hex
/// digits, including truncation at end of input) fails safe: the `#` is kept
/// as a literal byte, so detection can only ever widen, never be evaded by a
/// deliberately-broken escape.
fn normalize_pdf_name_escapes(bytes: &[u8]) -> Vec<u8> {
    fn is_name_delimiter(b: u8) -> bool {
        matches!(
            b,
            b' ' | b'\t'
                | b'\r'
                | b'\n'
                | 0x0c
                | 0x00
                | b'('
                | b')'
                | b'<'
                | b'>'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'/'
                | b'%'
        )
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        out.push(b);
        i += 1;
        if b != b'/' {
            continue;
        }
        // Inside a name token now: decode #xx escapes until a delimiter.
        while i < bytes.len() && !is_name_delimiter(bytes[i]) {
            if bytes[i] == b'#' {
                let decoded = bytes
                    .get(i + 1..i + 3)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|s| u8::from_str_radix(s, 16).ok());
                match decoded {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(b'#');
                        i += 1;
                    }
                }
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod async_capacity_tests {
    use super::*;

    static SANITIZER_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn tiny_png() -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image::RgbImage::new(2, 2))
            .write_to(&mut output, ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[tokio::test]
    async fn four_decoder_permits_reject_excess_without_waiting_and_recover() {
        let _serial = SANITIZER_TEST_LOCK.lock().await;
        let decoders = decoder_semaphore();
        let busy = decoders.try_acquire_many(4).unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            UploadSecurityService::sanitize_async(tiny_png(), "image/png".into()),
        )
        .await
        .expect("excess decoders must fail fast instead of queuing");
        assert!(matches!(result, Err(AppError::RateLimitExceeded(_))));
        assert_eq!(decoders.available_permits(), 0);
        drop(busy);

        let sanitized = UploadSecurityService::sanitize_async(tiny_png(), "image/png".into())
            .await
            .expect("released decoder capacity must be usable");
        let image = image::load_from_memory_with_format(&sanitized, ImageFormat::Png).unwrap();
        assert_eq!((image.width(), image.height()), (2, 2));
        assert!(sanitized.len() <= MAX_PRESCRIPTION_IMAGE_BYTES);
        assert_eq!(decoders.available_permits(), 4);
    }

    #[tokio::test]
    async fn invalid_mime_and_magic_reject_before_decoder_reservation() {
        let _serial = SANITIZER_TEST_LOCK.lock().await;
        let decoders = decoder_semaphore();
        let _busy = decoders.try_acquire_many(4).unwrap();
        for (bytes, mime) in [
            (tiny_png(), "application/pdf"),
            (b"not-an-image".to_vec(), "image/jpeg"),
            (vec![0, 1], "image/png"),
        ] {
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                UploadSecurityService::sanitize_async(bytes, mime.into()),
            )
            .await
            .expect("cheap invalid input must not queue for decoder capacity");
            assert!(matches!(result, Err(AppError::UnprocessableEntity(_))));
            assert_eq!(decoders.available_permits(), 0);
        }
    }

    #[test]
    fn cancelled_sanitizer_keeps_permit_until_blocking_work_finishes() {
        let _serial = SANITIZER_TEST_LOCK.blocking_lock();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            struct ReleaseWorker(Option<std::sync::mpsc::Sender<()>>);
            impl Drop for ReleaseWorker {
                fn drop(&mut self) {
                    if let Some(sender) = self.0.take() {
                        let _ = sender.send(());
                    }
                }
            }

            let deadline = std::time::Duration::from_secs(30);
            let decoders = decoder_semaphore();
            let held = decoders.try_acquire_many(3).unwrap();
            let (release, wait) = std::sync::mpsc::channel();
            let mut release = ReleaseWorker(Some(release));
            let (started, started_rx) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                wait.recv().unwrap();
            });
            tokio::time::timeout(deadline, started_rx)
                .await
                .expect("controlled blocking worker must start")
                .unwrap();

            let (polled, polled_rx) = tokio::sync::oneshot::channel();
            let caller = tokio::spawn(async move {
                let mut work = Box::pin(UploadSecurityService::sanitize_async(
                    tiny_png(),
                    "image/png".into(),
                ));
                let mut polled = Some(polled);
                std::future::poll_fn(|context| {
                    let result = std::future::Future::poll(work.as_mut(), context);
                    if let Some(sender) = polled.take() {
                        let _ = sender.send(result.is_pending());
                    }
                    result
                })
                .await
            });
            assert!(tokio::time::timeout(deadline, polled_rx)
                .await
                .expect("real sanitizer must reserve capacity")
                .unwrap());
            assert_eq!(decoders.available_permits(), 0);
            caller.abort();
            assert!(matches!(caller.await, Err(error) if error.is_cancelled()));
            assert_eq!(
                decoders.available_permits(),
                0,
                "caller cancellation must not release queued decoder capacity"
            );
            assert!(matches!(
                UploadSecurityService::sanitize_async(tiny_png(), "image/png".into()).await,
                Err(AppError::RateLimitExceeded(_))
            ));

            release.0.take().unwrap().send(()).unwrap();
            tokio::time::timeout(deadline, blocker)
                .await
                .expect("controlled worker must finish")
                .unwrap();
            let recovered = tokio::time::timeout(deadline, decoders.acquire())
                .await
                .expect("real decoder must finish after caller cancellation")
                .unwrap();
            assert_eq!(decoders.available_permits(), 0);
            drop(recovered);
            drop(held);
            assert_eq!(decoders.available_permits(), 4);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_pdf(body_extra: &[u8]) -> Vec<u8> {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        pdf.extend_from_slice(body_extra);
        pdf.extend_from_slice(b"\n%%EOF");
        pdf
    }

    #[test]
    fn accepts_clean_pdf() {
        let pdf = minimal_pdf(b"1 0 obj << /Type /Catalog >> endobj");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_ok());
    }

    #[test]
    fn rejects_pdf_with_javascript() {
        let pdf = minimal_pdf(b"1 0 obj << /S /JavaScript /JS (app.alert('x')) >> endobj");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    #[test]
    fn rejects_pdf_with_openaction() {
        let pdf = minimal_pdf(b"<< /Type /Catalog /OpenAction 5 0 R >>");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    #[test]
    fn rejects_encrypted_pdf() {
        let pdf = minimal_pdf(b"trailer << /Encrypt 3 0 R >>");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    // h03-pdf-dangerous-marker-hex-escape-bypass regression coverage: PDF
    // name objects permit `#xx` hex-escaping (ISO 32000-1 §7.3.5); every one
    // of these is spec-valid and must be caught exactly like its unescaped
    // literal form.

    #[test]
    fn rejects_hex_escaped_javascript_name() {
        // /J#61vaScript decodes to /JavaScript ('a' = 0x61).
        let pdf = minimal_pdf(b"1 0 obj << /S /J#61vaScript /JS (app.alert('x')) >> endobj");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    #[test]
    fn rejects_hex_escaped_openaction_name() {
        // /O#70enAction decodes to /OpenAction ('p' = 0x70).
        let pdf = minimal_pdf(b"<< /Type /Catalog /O#70enAction 5 0 R >>");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    #[test]
    fn rejects_hex_escaped_encrypt_name() {
        // /En#63rypt decodes to /Encrypt ('c' = 0x63).
        let pdf = minimal_pdf(b"trailer << /En#63rypt 3 0 R >>");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    #[test]
    fn rejects_multiple_escapes_in_one_name() {
        // Every character of JavaScript escaped, not just one.
        let pdf =
            minimal_pdf(b"<< /S /#4a#61#76#61#53#63#72#69#70#74 /JS (app.alert('x')) >> endobj");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    #[test]
    fn malformed_escape_fails_safe_without_panicking() {
        // A '#' not followed by two valid hex digits (here: 'zz', non-hex)
        // must never panic and must never cause the scanner to lose sync —
        // the literal '#' is kept verbatim, so this does not spell out a
        // real dangerous marker and the file is accepted, not misread into
        // something it isn't.
        let pdf = minimal_pdf(b"1 0 obj << /S /J#zzvaScript >> endobj");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_ok());
    }

    #[test]
    fn malformed_escape_truncated_at_end_of_name_fails_safe() {
        // '#' with fewer than two bytes remaining before end-of-input.
        let pdf = minimal_pdf(b"1 0 obj << /S /Foo#");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_ok());
    }

    #[test]
    fn literal_marker_still_rejected_after_normalization_change() {
        // Non-regression: ordinary unescaped markers (no '#' at all) must
        // still be caught exactly as before.
        let pdf = minimal_pdf(b"1 0 obj << /S /JavaScript /JS (app.alert('x')) >> endobj");
        assert!(UploadSecurityService::validate_and_sanitize(&pdf, "application/pdf").is_err());
    }

    /// Mimics a real scanned/photographed prescription: a near-white page
    /// with a gentle, slow lighting gradient (like uneven phone-camera
    /// lighting across a page) and sparse darker bands standing in for a
    /// handful of lines of text — most of a real prescription photo is
    /// blank paper, not wall-to-wall text, so bands cover a small minority
    /// of rows here, not every-other-band coverage. Deliberately a
    /// LOW-frequency pattern, not per-pixel random noise or a dense sawtooth
    /// — a real document photo is mostly large, smooth, highly-compressible
    /// regions (which is exactly why compressing it hard enough to meet the
    /// tight 40 KB per-image ceiling is realistic); a denser/higher-frequency
    /// pattern here would both misrepresent real input and made an earlier
    /// version of this test suite take minutes to run (confirmed directly,
    /// not assumed) chasing a target genuine phone photos don't need to.
    fn synthetic_large_photo(format: ImageFormat, side: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(side, side, |x, y| {
            let gradient = ((x as f32 / side as f32) * 20.0) as i32; // slow 0..20 ramp
            let is_text_line = y % 150 < 6; // sparse lines, mostly blank page
            let base: i32 = if is_text_line { 70 } else { 240 };
            let v = (base + gradient - 10).clamp(0, 255) as u8;
            image::Rgb([v, v, v])
        });
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, format)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn single_image_uses_close_to_its_full_budget_not_needlessly_over_compressed() {
        // One image gets the whole 40 KB ceiling to itself — it should land
        // close to that budget, not be hammered down toward the absolute
        // technical floors for no reason.
        let original = synthetic_large_photo(ImageFormat::Jpeg, 3000);
        let sanitized =
            UploadSecurityService::validate_and_sanitize(&original, "image/jpeg").unwrap();

        assert!(
            sanitized.len() < original.len(),
            "expected compression to shrink the file: original={}, sanitized={}",
            original.len(),
            sanitized.len()
        );
        assert!(
            sanitized.len() <= MAX_PRESCRIPTION_IMAGE_BYTES,
            "must fit the 40 KB per-image ceiling, got {} bytes",
            sanitized.len()
        );
        // Should use a meaningful fraction of its budget, not collapse to a
        // tiny fraction of it.
        assert!(
            sanitized.len() >= MAX_PRESCRIPTION_IMAGE_BYTES / 4,
            "expected the image to use close to its full budget, got only {} bytes",
            sanitized.len()
        );

        let decoded = image::load_from_memory_with_format(&sanitized, ImageFormat::Jpeg)
            .expect("sanitized output must still decode as a valid JPEG");
        assert!(decoded.width() <= MAX_IMAGE_DIMENSION);
        assert!(decoded.height() <= MAX_IMAGE_DIMENSION);
    }

    #[test]
    fn five_images_each_independently_fit_the_40kb_ceiling_not_a_divided_share() {
        // Product rule under test: each image gets its OWN independent 40 KB
        // ceiling, never a shared batch budget divided by image count (the
        // prior, incorrect "400 KB / N images" behavior). This asserts EACH
        // of 5 images, compressed independently, individually fits the full
        // 40 KB ceiling, and that the 5-image combined total exceeds a
        // single image's budget — proving no batch-wide division occurred
        // (under the old divided-by-5 behavior the total could never have
        // cleared that bar).
        let mut total = 0usize;
        for side in [3000u32, 2800, 3200, 2600, 3000] {
            let original = synthetic_large_photo(ImageFormat::Jpeg, side);
            let sanitized =
                UploadSecurityService::validate_and_sanitize(&original, "image/jpeg").unwrap();
            let decoded = image::load_from_memory_with_format(&sanitized, ImageFormat::Jpeg)
                .expect("each sanitized output must still decode as a valid JPEG");
            assert!(decoded.width() <= MAX_IMAGE_DIMENSION);
            assert!(decoded.height() <= MAX_IMAGE_DIMENSION);
            assert!(
                sanitized.len() <= MAX_PRESCRIPTION_IMAGE_BYTES,
                "each image must independently fit the 40 KB ceiling, got {} bytes",
                sanitized.len()
            );
            total += sanitized.len();
        }
        // If the old divide-by-batch-size behavior were still present, each
        // image would be capped at 40 KB / 5 = 8 KB and the 5-image total
        // could never exceed 40 KB. Proving the total clears that bar
        // demonstrates each image really did get its own full ceiling.
        assert!(
            total > MAX_PRESCRIPTION_IMAGE_BYTES,
            "5 independently-compressed images should sum to more than a single \
             image's budget, proving no batch-wide division occurred: got {total} bytes"
        );
    }

    #[test]
    fn large_png_is_downscaled_and_compressed_toward_its_own_40kb_budget() {
        // PNG is lossless, so the same pattern encodes far larger than as
        // JPEG; a smaller side keeps the *original* fixture itself under the
        // 10 MB input gate while still comfortably exceeding
        // `MAX_IMAGE_DIMENSION` so the downscale path is genuinely exercised.
        // PNG has no quality knob, so hitting the 40 KB ceiling requires the
        // iterative downscale-and-retry loop (not just one fixed resize).
        let original = synthetic_large_photo(ImageFormat::Png, 2200);
        let sanitized =
            UploadSecurityService::validate_and_sanitize(&original, "image/png").unwrap();

        assert!(
            sanitized.len() < original.len(),
            "expected the dimension downscale to shrink the file: original={}, sanitized={}",
            original.len(),
            sanitized.len()
        );
        // The guarantee: the fixed 40 KB per-image budget must never be
        // exceeded, even though PNG has no quality knob to trade — only the
        // downscale-and-retry loop, which must be willing to go all the way
        // to `ABSOLUTE_MIN_IMAGE_DIMENSION` if truly necessary.
        assert!(
            sanitized.len() <= MAX_PRESCRIPTION_IMAGE_BYTES,
            "must fit the 40 KB per-image ceiling, got {} bytes",
            sanitized.len()
        );

        let decoded = image::load_from_memory_with_format(&sanitized, ImageFormat::Png)
            .expect("sanitized output must still decode as a valid PNG");
        assert!(decoded.width() <= MAX_IMAGE_DIMENSION);
        assert!(decoded.height() <= MAX_IMAGE_DIMENSION);
        assert!(decoded.width() >= ABSOLUTE_MIN_IMAGE_DIMENSION);
        assert!(decoded.height() >= ABSOLUTE_MIN_IMAGE_DIMENSION);
    }

    #[test]
    fn small_image_is_not_needlessly_degraded() {
        // A tiny image (well under the 40 KB ceiling) should pass through on
        // an early iteration, not get hammered down toward the absolute
        // technical floors for no reason.
        let original = synthetic_large_photo(ImageFormat::Jpeg, 20);
        let sanitized =
            UploadSecurityService::validate_and_sanitize(&original, "image/jpeg").unwrap();
        image::load_from_memory_with_format(&sanitized, ImageFormat::Jpeg)
            .expect("sanitized output must still decode");
    }

    /// True pseudo-random per-pixel noise — JPEG/PNG compression's genuine
    /// worst case (no spatial redundancy for either a DCT or a DEFLATE-style
    /// filter to exploit), unlike `synthetic_large_photo`'s realistic
    /// document-photo pattern. Seeded for determinism.
    fn synthetic_worst_case_noise(format: ImageFormat, side: u32) -> Vec<u8> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let img = image::RgbImage::from_fn(side, side, |_, _| {
            image::Rgb([rng.gen::<u8>(), rng.gen::<u8>(), rng.gen::<u8>()])
        });
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, format)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn worst_case_noise_jpeg_never_exceeds_the_guaranteed_per_image_ceiling() {
        // Final design decision, directly: "the idea is system will auto
        // compress whatever is uploaded into [budget]... no rejection or
        // nothing" — this is a hard guarantee, not a best-effort target,
        // proven here against JPEG's actual worst case (uncompressible
        // per-pixel noise) rather than the realistic, much-more-compressible
        // document fixture used elsewhere in this file. Each image is
        // checked independently against its own fixed 40 KB ceiling — there
        // is no shared/divided batch budget to check against.
        for side in [2200u32, 2100, 2300, 2000, 2200] {
            let original = synthetic_worst_case_noise(ImageFormat::Jpeg, side);
            let sanitized = UploadSecurityService::validate_and_sanitize(&original, "image/jpeg")
                .expect(
                    "a hard-to-compress image must never be rejected for being hard to compress",
                );
            assert!(
                sanitized.len() <= MAX_PRESCRIPTION_IMAGE_BYTES,
                "must never exceed the 40 KB per-image ceiling, even for worst-case noise \
                 input: got {} bytes",
                sanitized.len()
            );
            image::load_from_memory_with_format(&sanitized, ImageFormat::Jpeg)
                .expect("sanitized output must still decode as a valid JPEG");
        }
    }

    #[test]
    fn worst_case_noise_png_never_exceeds_its_budget() {
        // PNG has no quality knob at all, so worst-case noise (which DEFLATE
        // essentially cannot compress) forces the downscale-and-retry loop
        // to go further than any realistic photo would ever need — this
        // proves it actually gets there rather than stopping early.
        //
        // 1400px rather than something closer to MAX_IMAGE_DIMENSION
        // (2000px): true uncompressed random-noise RGB PNG is ~3 bytes/pixel
        // regardless of resolution (DEFLATE finds essentially no redundancy
        // in real noise), so a 2000x2000 instance is ~12 MB raw — it would
        // trip the separate, pre-existing, intentionally-unrelated 10 MB
        // *input* rejection limit before compression even starts. 1400x1400
        // (~5.9 MB raw) stays under that input gate while still being far
        // larger than the 40 KB per-image budget, so the compression
        // guarantee itself is still genuinely exercised.
        let original = synthetic_worst_case_noise(ImageFormat::Png, 1400);
        let sanitized = UploadSecurityService::validate_and_sanitize(&original, "image/png")
            .expect("a hard-to-compress image must never be rejected for being hard to compress");
        assert!(
            sanitized.len() <= MAX_PRESCRIPTION_IMAGE_BYTES,
            "must never exceed the 40 KB per-image ceiling even for worst-case noise, got {} bytes",
            sanitized.len()
        );
        image::load_from_memory_with_format(&sanitized, ImageFormat::Png)
            .expect("sanitized output must still decode as a valid PNG");
    }
}
