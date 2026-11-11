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

pub const MAX_COVER_UPLOAD: usize = 5 * 1024 * 1024;
const MAX_COVER_BYTES: usize = 500 * 1024;
fn object_key(id: Uuid) -> String {
    format!("article-media/{id}.jpg")
}
fn storage(state: &AppState) -> Result<&Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .article_media_storage
        .as_ref()
        .ok_or_else(|| AppError::Conflict("Article media storage is not configured.".into()))
}
pub fn sanitize(bytes: &[u8], mime: &str) -> Result<Vec<u8>, AppError> {
    if bytes.len() > MAX_COVER_UPLOAD {
        return Err(AppError::PayloadTooLarge(
            "Cover upload is limited to 5 MB.".into(),
        ));
    }
    let format = image::guess_format(bytes)
        .map_err(|_| AppError::Validation("A JPEG or PNG cover is required.".into()))?;
    if !matches!(
        (format, mime),
        (image::ImageFormat::Jpeg, "image/jpeg") | (image::ImageFormat::Png, "image/png")
    ) {
        return Err(AppError::Validation(
            "Cover MIME type must match JPEG or PNG bytes.".into(),
        ));
    }
    let dimensions = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| AppError::Validation("Malformed image.".into()))?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > 8192
        || dimensions.1 > 8192
        || u64::from(dimensions.0) * u64::from(dimensions.1) > 16_000_000
    {
        return Err(AppError::Validation(
            "Cover dimensions exceed 8192 pixels or 16 megapixels.".into(),
        ));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| AppError::Validation("Malformed image.".into()))?;
    let mut decoded = decoded.thumbnail(1600, 1600);
    for _ in 0..4 {
        for quality in [86, 76, 66] {
            let mut output = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
                .encode_image(&decoded)
                .map_err(|_| AppError::Validation("Image encoding failed.".into()))?;
            if output.len() <= MAX_COVER_BYTES {
                return Ok(output);
            }
        }
        decoded = decoded.thumbnail(decoded.width() * 3 / 4, decoded.height() * 3 / 4);
    }
    Err(AppError::Validation(
        "Cover could not be optimized safely.".into(),
    ))
}
async fn sanitize_async(bytes: Vec<u8>, mime: String) -> Result<Vec<u8>, AppError> {
    static DECODERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = DECODERS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            AppError::RateLimitExceeded("Cover processing is busy. Retry shortly.".into())
        })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        sanitize(&bytes, &mime)
    })
    .await
    .map_err(|_| AppError::internal("Cover processing failed.".into()))?
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
    let sanitized = sanitize_async(body.to_vec(), mime).await?;
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
pub async fn cover(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let mut tx = state.begin_authed(&ctx).await?;
    // RLS plus explicit published guard: Admin can inspect drafts; all other callers see current published covers only.
    let row=sqlx::query("SELECT m.id,m.content_hash,m.size_bytes FROM admin_articles a JOIN article_media m ON m.id=a.cover_image_id AND m.article_id=a.id WHERE a.id=$1 AND m.ready AND (a.status='PUBLISHED' OR $2='ADMIN')").bind(id).bind(ctx.role.as_str()).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("Article cover not found.".into()))?;
    tx.commit().await?;
    let media: Uuid = row.get("id");
    let object = storage(&state)?
        .get(&ObjectPath::from(object_key(media)))
        .await
        .map_err(|_| AppError::NotFound("Article cover unavailable.".into()))?;
    if object.meta.size > (MAX_COVER_BYTES + 28) as u64 {
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
    }
    #[test]
    fn cover_rejects_extreme_dimensions_before_decode() {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8193, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        assert!(sanitize(&bytes.into_inner(), "image/png").is_err());
    }
}
