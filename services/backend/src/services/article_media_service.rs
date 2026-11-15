//! Article-only encrypted object namespace. No clinical keys or arbitrary paths enter this API.
use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    crypto,
    error::AppError,
    repository::{admin_article_repo, AppState, DbRole},
    services::audit_service::{AuditLogEntry, AuditService},
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use object_store::{path::Path as ObjectPath, ObjectStoreExt};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
};
use uuid::Uuid;

/// Doctor-written article covers (unchanged): 5 MB in, at most 500 KB stored.
pub const MAX_COVER_UPLOAD: usize = 5 * 1024 * 1024;
const MAX_COVER_BYTES: usize = 500 * 1024;
/// Admin-posted images (article covers, facility photos), product-owner
/// instruction 2026-11-14: the Admin may upload a photo of practically any
/// size; the system compresses it automatically and stores at most 1 MB.
/// The input ceiling (25 MB, 50 megapixels) exists only to keep decoding
/// memory bounded; it admits ordinary phone and camera photos.
pub const MAX_ADMIN_IMAGE_UPLOAD: usize = 25 * 1024 * 1024;
pub const MAX_ADMIN_IMAGE_BYTES: usize = 1024 * 1024;

struct ImageProfile {
    max_upload: usize,
    max_bytes: usize,
    max_side: u32,
    max_pixels: u64,
    max_alloc: u64,
    target_side: u32,
    qualities: &'static [u8],
    too_large: &'static str,
}

const DOCTOR_COVER: ImageProfile = ImageProfile {
    max_upload: MAX_COVER_UPLOAD,
    max_bytes: MAX_COVER_BYTES,
    max_side: 8192,
    max_pixels: 16_000_000,
    max_alloc: 128 * 1024 * 1024,
    target_side: 1600,
    qualities: &[86, 76, 66],
    too_large: "Cover upload is limited to 5 MB.",
};

const ADMIN_IMAGE: ImageProfile = ImageProfile {
    max_upload: MAX_ADMIN_IMAGE_UPLOAD,
    max_bytes: MAX_ADMIN_IMAGE_BYTES,
    max_side: 12_000,
    max_pixels: 50_000_000,
    max_alloc: 256 * 1024 * 1024,
    target_side: 2048,
    qualities: &[88, 80, 72, 64],
    too_large: "Image upload is limited to 25 MB.",
};

fn object_key(id: Uuid) -> String {
    format!("article-media/{id}.jpg")
}
fn doctor_object_key(id: Uuid) -> String {
    format!("doctor-article-media/{id}.jpg")
}
fn storage(state: &AppState) -> Result<&Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .article_media_storage
        .as_ref()
        .ok_or_else(|| AppError::Conflict("Article media storage is not configured.".into()))
}
/// Doctor article cover: validated, re-encoded to JPEG, at most 500 KB.
pub fn sanitize(bytes: &[u8], mime: &str) -> Result<Vec<u8>, AppError> {
    sanitize_with(bytes, mime, &DOCTOR_COVER)
}

/// Admin article cover or facility photo: validated, re-encoded to JPEG,
/// automatically compressed to at most 1 MB.
pub fn sanitize_admin(bytes: &[u8], mime: &str) -> Result<Vec<u8>, AppError> {
    sanitize_with(bytes, mime, &ADMIN_IMAGE)
}

fn sanitize_with(bytes: &[u8], mime: &str, profile: &ImageProfile) -> Result<Vec<u8>, AppError> {
    if bytes.len() > profile.max_upload {
        return Err(AppError::PayloadTooLarge(profile.too_large.into()));
    }
    let format = image::guess_format(bytes)
        .map_err(|_| AppError::Validation("A JPEG or PNG image is required.".into()))?;
    if !matches!(
        (format, mime),
        (image::ImageFormat::Jpeg, "image/jpeg") | (image::ImageFormat::Png, "image/png")
    ) {
        return Err(AppError::Validation(
            "Image MIME type must match JPEG or PNG bytes.".into(),
        ));
    }
    let dimensions = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| AppError::Validation("Malformed image.".into()))?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > profile.max_side
        || dimensions.1 > profile.max_side
        || u64::from(dimensions.0) * u64::from(dimensions.1) > profile.max_pixels
    {
        return Err(AppError::Validation(format!(
            "Image dimensions exceed {} pixels or {} megapixels.",
            profile.max_side,
            profile.max_pixels / 1_000_000
        )));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(profile.max_alloc);
    limits.max_image_width = Some(profile.max_side);
    limits.max_image_height = Some(profile.max_side);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| AppError::Validation("Malformed image.".into()))?;
    let mut decoded = decoded.thumbnail(profile.target_side, profile.target_side);
    for _ in 0..6 {
        for &quality in profile.qualities {
            let mut output = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
                .encode_image(&decoded)
                .map_err(|_| AppError::Validation("Image encoding failed.".into()))?;
            if output.len() <= profile.max_bytes {
                return Ok(output);
            }
        }
        decoded = decoded.thumbnail(decoded.width() * 3 / 4, decoded.height() * 3 / 4);
    }
    Err(AppError::Validation(
        "Image could not be optimized safely.".into(),
    ))
}

/// Doctor article covers. Bounded to two concurrent decodes.
pub async fn sanitize_async(bytes: Vec<u8>, mime: String) -> Result<Vec<u8>, AppError> {
    static DECODERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    run_bounded(&DECODERS, 2, move || sanitize(&bytes, &mime)).await
}

/// Admin article covers and facility photos. Large inputs decode to up to
/// 256 MB, so only one runs at a time per process.
pub async fn sanitize_admin_async(bytes: Vec<u8>, mime: String) -> Result<Vec<u8>, AppError> {
    static DECODERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    run_bounded(&DECODERS, 1, move || sanitize_admin(&bytes, &mime)).await
}

async fn run_bounded(
    cell: &'static OnceLock<Arc<tokio::sync::Semaphore>>,
    permits: usize,
    work: impl FnOnce() -> Result<Vec<u8>, AppError> + Send + 'static,
) -> Result<Vec<u8>, AppError> {
    let permit = cell
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(permits)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            AppError::RateLimitExceeded("Image processing is busy. Retry shortly.".into())
        })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|_| AppError::internal("Image processing failed.".into()))?
}
#[derive(Deserialize)]
pub struct CoverQuery {
    pub version: i32,
}
pub async fn admin_cover(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    cover(State(state), ctx, Path(id)).await
}
pub async fn upload(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Query(q): Query<CoverQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    storage(&state)?;
    crate::rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _guard =
        crate::rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 1)
            .await?;
    // Cheap RLS/version preflight before reserving a decoder; no transaction held during decoding.
    {
        let mut tx = state.begin_authed(&ctx).await?;
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_articles WHERE id=$1 AND version=$2 AND status<>'ARCHIVED')").bind(id).bind(q.version).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        if !exists {
            return Err(AppError::Conflict(
                "Article changed or archived. Refresh before uploading.".into(),
            ));
        }
    }
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let sanitized = sanitize_admin_async(body.to_vec(), mime).await?;
    let media = Uuid::new_v4();
    let key = object_key(media);
    let hash = hex::encode(Sha256::digest(&sanitized));
    // Durable intent precedes the object write. A cancelled request/crash leaves a cleanup candidate.
    {
        let mut tx = state.begin_authed(&ctx).await?;
        sqlx::query("INSERT INTO article_media(id,article_id,object_key,content_hash,size_bytes) VALUES($1,$2,$3,$4,$5)").bind(media).bind(id).bind(&key).bind(&hash).bind(sanitized.len() as i32).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    let encrypted = crypto::encrypt_bytes(&sanitized, &state.config.storage_encryption_key_b64)?;
    storage(&state)?
        .put_opts(
            &ObjectPath::from(key),
            encrypted.into(),
            object_store::PutOptions {
                mode: object_store::PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .map_err(|_| AppError::internal("Article media upload failed.".into()))?;
    let mut tx = state.begin_authed(&ctx).await?;
    sqlx::query("UPDATE article_media SET ready=true WHERE id=$1")
        .bind(media)
        .execute(&mut *tx)
        .await?;
    let changed=sqlx::query("UPDATE admin_articles SET cover_image_id=$1,status='DRAFT',version=version+1,content_version=content_version+1,approved_version=NULL,approved_by=NULL,approved_at=NULL,scheduled_for=NULL,updated_at=now() WHERE id=$2 AND version=$3 AND status<>'ARCHIVED'").bind(media).bind(id).bind(q.version).execute(&mut *tx).await?.rows_affected()==1;
    if !changed {
        return Err(AppError::Conflict(
            "Article changed during upload. The abandoned cover will be cleaned up.".into(),
        ));
    }
    let data = admin_article_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    crate::api::v1::admin_article_handlers::audit(&mut tx, &ctx, id, "ADMIN_ARTICLE_COVER", &data)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":data})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoctorCoverUpload {
    pub version: i32,
    pub mime_type: String,
    pub base64_content: String,
}
/// Doctor Web redesign (2026-11-14): a Doctor sets the cover of their own non-archived
/// article. Same sanitising/re-encoding, encryption and size limits as Admin covers.
pub async fn doctor_upload(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<DoctorCoverUpload>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::DoctorArticles)?;
    let author = ctx.verified_doctor_profile_id(&state).await?;
    storage(&state)?;
    crate::rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _guard =
        crate::rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 1)
            .await?;
    {
        let mut tx = state.begin_authed(&ctx).await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM doctor_articles WHERE id=$1 AND author_id=$2 AND version=$3 AND status<>'ARCHIVED')")
            .bind(id).bind(author).bind(payload.version).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        if !exists {
            return Err(AppError::Conflict(
                "Article changed or archived. Refresh before uploading.".into(),
            ));
        }
    }
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(payload.base64_content.split(',').next_back().unwrap_or(""))
        .map_err(|_| AppError::Validation("Cover content is not valid base64.".into()))?;
    let sanitized = sanitize_async(raw, payload.mime_type).await?;
    let media = Uuid::new_v4();
    let key = doctor_object_key(media);
    let hash = hex::encode(Sha256::digest(&sanitized));
    {
        let mut tx = state.begin_authed(&ctx).await?;
        sqlx::query("INSERT INTO doctor_article_media(id,article_id,object_key,content_hash,size_bytes) VALUES($1,$2,$3,$4,$5)")
            .bind(media).bind(id).bind(&key).bind(&hash).bind(sanitized.len() as i32).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    let encrypted = crypto::encrypt_bytes(&sanitized, &state.config.storage_encryption_key_b64)?;
    storage(&state)?
        .put_opts(
            &ObjectPath::from(key),
            encrypted.into(),
            object_store::PutOptions {
                mode: object_store::PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .map_err(|_| AppError::internal("Article media upload failed.".into()))?;
    let mut tx = state.begin_authed(&ctx).await?;
    sqlx::query("UPDATE doctor_article_media SET ready=true WHERE id=$1")
        .bind(media)
        .execute(&mut *tx)
        .await?;
    let changed = sqlx::query("UPDATE doctor_articles SET cover_image_id=$1,version=version+1,updated_at=now() WHERE id=$2 AND author_id=$3 AND version=$4 AND status<>'ARCHIVED'")
        .bind(media).bind(id).bind(author).bind(payload.version).execute(&mut *tx).await?.rows_affected() == 1;
    if !changed {
        return Err(AppError::Conflict(
            "Article changed during upload. The abandoned cover will be cleaned up.".into(),
        ));
    }
    let article = crate::repository::article_repo::find(&mut *tx, id, Some(author))
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "ARTICLE_COVER",
            "DOCTOR_ARTICLE",
            Some(id),
            "SUCCESS",
        )
        .with_after(json!({"version": article.version, "cover_image_id": media})),
    )
    .await?;
    tx.commit().await?;
    state
        .content_cache
        .invalidate(crate::content_cache::Area::Articles);
    Ok(Json(json!({"success":true,"data":article})))
}
pub async fn cover(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let mut tx = state.begin_authed(&ctx).await?;
    // RLS plus explicit published guard: Admin can inspect drafts; all other callers see current published covers only.
    let admin_row=sqlx::query("SELECT m.id,m.content_hash,m.size_bytes FROM admin_articles a JOIN article_media m ON m.id=a.cover_image_id AND m.article_id=a.id WHERE a.id=$1 AND m.ready AND (a.status='PUBLISHED' OR $2='ADMIN')").bind(id).bind(ctx.role.as_str()).fetch_optional(&mut *tx).await?;
    // Doctor article covers: RLS shows the author their own drafts and everyone else
    // only the current cover of a published article.
    let (row, key) = match admin_row {
        Some(row) => {
            let media: Uuid = row.get("id");
            (row, object_key(media))
        }
        None => {
            let row = sqlx::query("SELECT m.id,m.content_hash,m.size_bytes FROM doctor_articles a JOIN doctor_article_media m ON m.id=a.cover_image_id AND m.article_id=a.id WHERE a.id=$1 AND m.ready")
                .bind(id).fetch_optional(&mut *tx).await?
                .ok_or_else(|| AppError::NotFound("Article cover not found.".into()))?;
            let media: Uuid = row.get("id");
            (row, doctor_object_key(media))
        }
    };
    tx.commit().await?;
    let object = storage(&state)?
        .get(&ObjectPath::from(key))
        .await
        .map_err(|_| AppError::NotFound("Article cover unavailable.".into()))?;
    if object.meta.size > (MAX_ADMIN_IMAGE_BYTES + 28) as u64 {
        return Err(AppError::internal("Article media size mismatch.".into()));
    }
    let encrypted = object
        .bytes()
        .await
        .map_err(|_| AppError::internal("Article media download failed.".into()))?;
    let bytes = crypto::decrypt_bytes(&encrypted, &state.config.storage_encryption_key_b64)?;
    if bytes.len() != row.get::<i32, _>("size_bytes") as usize
        || hex::encode(Sha256::digest(&bytes)) != row.get::<String, _>("content_hash")
    {
        return Err(AppError::internal(
            "Article media integrity mismatch.".into(),
        ));
    }
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}
pub async fn maintenance(state: &AppState) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("UPDATE admin_articles SET status='PUBLISHED',version=version+1,published_at=now(),scheduled_for=NULL,updated_at=now() WHERE id IN (SELECT id FROM admin_articles WHERE status='SCHEDULED' AND scheduled_for<=now() AND approved_version=content_version ORDER BY scheduled_for,id LIMIT 50 FOR UPDATE SKIP LOCKED) AND status='SCHEDULED' AND approved_version=content_version RETURNING id").fetch_all(&mut *tx).await?;
    let published_any = !ids.is_empty();
    for id in ids {
        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                None,
                ctx.role,
                "ADMIN_ARTICLE_SCHEDULED_PUBLISH",
                "ADMIN_ARTICLE",
                Some(id),
                "SUCCESS",
            ),
        )
        .await?;
    }
    tx.commit().await?;
    if published_any {
        // After the commit, so a reader cannot re-cache the old feed.
        state
            .content_cache
            .invalidate(crate::content_cache::Area::Articles);
    }
    if state.object_storage.is_none() {
        return Ok(());
    }
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM article_media m WHERE created_at<now()-interval '24 hours' AND NOT EXISTS(SELECT 1 FROM admin_articles a WHERE a.cover_image_id=m.id) ORDER BY created_at,id LIMIT 20 FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for id in ids {
        match storage(state)?
            .delete(&ObjectPath::from(object_key(id)))
            .await
        {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                sqlx::query("DELETE FROM article_media WHERE id=$1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            Err(_) => {
                return Err(AppError::internal(
                    "Article media cleanup failed; will retry.".into(),
                ))
            }
        }
    }
    tx.commit().await?;
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM doctor_article_media m WHERE created_at<now()-interval '24 hours' AND NOT EXISTS(SELECT 1 FROM doctor_articles a WHERE a.cover_image_id=m.id) ORDER BY created_at,id LIMIT 20 FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for id in ids {
        match storage(state)?
            .delete(&ObjectPath::from(doctor_object_key(id)))
            .await
        {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                sqlx::query("DELETE FROM doctor_article_media WHERE id=$1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            Err(_) => {
                return Err(AppError::internal(
                    "Doctor article media cleanup failed; will retry.".into(),
                ))
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png() -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(16, 16)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    #[test]
    fn cover_reencodes_and_rejects_unsafe_types() {
        let result = sanitize(&png(), "image/png").unwrap();
        assert_eq!(
            image::guess_format(&result).unwrap(),
            image::ImageFormat::Jpeg
        );
        assert!(result.len() < MAX_COVER_BYTES);
        for (body, mime) in [
            (b"<script>alert(1)</script>".as_slice(), "image/png"),
            (b"\xff\xd8\xff\xe0", "image/jpeg"),
            (b"<svg/>", "image/svg+xml"),
        ] {
            assert!(sanitize(body, mime).is_err());
        }
        assert!(sanitize(&png(), "image/jpeg").is_err());
        assert!(sanitize(&vec![0; MAX_COVER_UPLOAD + 1], "image/png").is_err());
    }
    #[test]
    fn cover_keys_are_fixed_namespace() {
        let id = Uuid::new_v4();
        assert_eq!(object_key(id), format!("article-media/{id}.jpg"));
        assert_eq!(
            doctor_object_key(id),
            format!("doctor-article-media/{id}.jpg")
        );
    }
    #[test]
    fn cover_rejects_extreme_dimensions_before_decode() {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8193, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        assert!(sanitize(&bytes.into_inner(), "image/png").is_err());
    }

    /// Worst case for JPEG: full-colour noise. Deterministic, no RNG crate.
    fn noise_png(width: u32, height: u32) -> Vec<u8> {
        let mut seed: u32 = 0x9e37_79b9;
        let image = image::RgbImage::from_fn(width, height, |_, _| {
            let mut next = || {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            };
            image::Rgb([next(), next(), next()])
        });
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn admin_image_over_5mb_is_accepted_and_compressed_to_1mb() {
        let input = noise_png(2600, 2400);
        assert!(
            input.len() > 5 * 1024 * 1024,
            "fixture must exceed the old 5 MB limit"
        );
        assert!(input.len() <= MAX_ADMIN_IMAGE_UPLOAD);
        let stored = sanitize_admin(&input, "image/png").unwrap();
        assert!(
            stored.len() <= MAX_ADMIN_IMAGE_BYTES,
            "{} bytes",
            stored.len()
        );
        assert_eq!(
            image::guess_format(&stored).unwrap(),
            image::ImageFormat::Jpeg
        );
        let decoded = image::load_from_memory(&stored).unwrap();
        assert!(decoded.width() <= 2048 && decoded.height() <= 2048);
    }

    #[test]
    fn admin_image_keeps_more_detail_than_doctor_cover() {
        // A photo-like gradient: the Admin profile keeps 2048 px, the Doctor
        // cover profile still stops at 1600 px.
        let image = image::RgbImage::from_fn(2400, 1600, |x, y| {
            image::Rgb([(x * 255 / 2400) as u8, (y * 255 / 1600) as u8, 128])
        });
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let input = out.into_inner();
        let admin = image::load_from_memory(&sanitize_admin(&input, "image/png").unwrap()).unwrap();
        assert_eq!(admin.width(), 2048);
        let doctor = image::load_from_memory(&sanitize(&input, "image/png").unwrap()).unwrap();
        assert_eq!(doctor.width(), 1600);
    }

    #[test]
    fn admin_image_input_ceiling_is_25mb() {
        assert!(sanitize_admin(&vec![0; MAX_ADMIN_IMAGE_UPLOAD + 1], "image/png").is_err());
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(12_001, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        assert!(sanitize_admin(&bytes.into_inner(), "image/png").is_err());
    }
}
