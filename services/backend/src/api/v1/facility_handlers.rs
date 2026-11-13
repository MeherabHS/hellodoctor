//! Facilities & Services. Admins manage facilities (`/admin/facilities…`);
//! any signed-in user reads published ones (`/facilities…`). Row-level
//! security already hides unpublished rows from non-Admins; the patient
//! queries also filter on PUBLISHED explicitly so an Admin token on a patient
//! route still sees only what patients see.
use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    crypto,
    domain::facilities::{
        canonical_county, compute_status, timezone_offset, FacilityContent, OpeningHours,
    },
    error::AppError,
    repository::AppState,
    services::{
        article_media_service::{sanitize_async, MAX_COVER_UPLOAD},
        audit_service::{AuditLogEntry, AuditService},
    },
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use object_store::{path::Path as ObjectPath, ObjectStoreExt};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MAX_IMAGE_UPLOAD: usize = MAX_COVER_UPLOAD;
const MAX_IMAGE_BYTES: usize = 500 * 1024;

const COLUMNS: &str = "id,name,description,county,location_text,contact_phone,google_maps_url,services,youth_friendly,timezone,opening_hours,image_media_id,publication_status,version,published_at,archived_at,created_at,updated_at,created_by,updated_by";

#[derive(sqlx::FromRow)]
struct FacilityRow {
    id: Uuid,
    name: String,
    description: String,
    county: String,
    location_text: String,
    contact_phone: String,
    google_maps_url: String,
    services: Vec<String>,
    youth_friendly: bool,
    timezone: String,
    opening_hours: Option<Value>,
    image_media_id: Option<Uuid>,
    publication_status: String,
    version: i32,
    published_at: Option<DateTime<Utc>>,
    archived_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    created_by: Uuid,
    updated_by: Uuid,
}

impl FacilityRow {
    fn hours(&self) -> Option<OpeningHours> {
        self.opening_hours
            .clone()
            .and_then(|v| serde_json::from_value(v).ok())
    }
    fn content(&self) -> FacilityContent {
        FacilityContent {
            name: self.name.clone(),
            description: self.description.clone(),
            county: self.county.clone(),
            location_text: self.location_text.clone(),
            contact_phone: self.contact_phone.clone(),
            google_maps_url: self.google_maps_url.clone(),
            services: self.services.clone(),
            youth_friendly: self.youth_friendly,
            opening_hours: self.hours(),
        }
    }
    fn status(&self, now: DateTime<Utc>) -> Value {
        let offset = timezone_offset(&self.timezone)
            .or_else(|| timezone_offset("Africa/Nairobi"))
            .expect("Nairobi offset");
        json!(compute_status(self.hours().as_ref(), now, offset))
    }
    fn services_json(&self, categories: &[Category]) -> Vec<Value> {
        self.services
            .iter()
            .filter_map(|code| categories.iter().find(|c| &c.code == code))
            .map(|c| json!({"code":c.code,"name_en":c.name_en,"name_sw":c.name_sw,"icon_key":c.icon_key}))
            .collect()
    }
    /// Patient projection: no audit fields, no storage keys, no drafts.
    fn patient_json(&self, categories: &[Category], now: DateTime<Utc>) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "county": self.county,
            "location_text": self.location_text,
            "contact_phone": self.contact_phone,
            "google_maps_url": self.google_maps_url,
            "services": self.services_json(categories),
            "youth_friendly": self.youth_friendly,
            "timezone": self.timezone,
            "opening_hours": self.opening_hours,
            "status": self.status(now),
            "image_url": self.image_media_id.map(|m| format!("/facilities/{}/image?v={m}", self.id)),
        })
    }
    fn admin_json(&self, categories: &[Category], now: DateTime<Utc>) -> Value {
        let content = self.content();
        json!({
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "county": self.county,
            "location_text": self.location_text,
            "contact_phone": self.contact_phone,
            "google_maps_url": self.google_maps_url,
            "services": self.services,
            "service_details": self.services_json(categories),
            "youth_friendly": self.youth_friendly,
            "timezone": self.timezone,
            "opening_hours": self.opening_hours,
            "status": self.status(now),
            "has_image": self.image_media_id.is_some(),
            "image_version": self.image_media_id,
            "publication_status": self.publication_status,
            "version": self.version,
            "published_at": self.published_at,
            "archived_at": self.archived_at,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "created_by": self.created_by,
            "updated_by": self.updated_by,
            "publish_problems": publish_problems(&content, self.image_media_id.is_some()),
        })
    }
}

fn publish_problems(content: &FacilityContent, has_image: bool) -> Vec<&'static str> {
    let mut problems = content.publish_problems();
    if !has_image {
        problems.push("A facility photo is required.");
    }
    problems
}

#[derive(sqlx::FromRow, Clone)]
struct Category {
    code: String,
    name_en: String,
    name_sw: String,
    icon_key: String,
}

async fn categories<'e, E: sqlx::PgExecutor<'e>>(db: E) -> Result<Vec<Category>, AppError> {
    Ok(sqlx::query_as::<_, Category>(
        "SELECT code,name_en,name_sw,icon_key FROM facility_service_categories WHERE active ORDER BY sort_order,code",
    )
    .fetch_all(db)
    .await?)
}

fn ok(data: Value) -> Json<Value> {
    Json(json!({"success":true,"data":data}))
}

fn conflict() -> AppError {
    AppError::Conflict(
        "Facility changed or this action is unavailable. Refresh before retrying.".into(),
    )
}

fn db_error(error: sqlx::Error) -> AppError {
    match error.as_database_error().and_then(|e| e.code()).as_deref() {
        Some("23514" | "23503") => {
            AppError::Validation("The facility is incomplete or invalid for this action.".into())
        }
        _ => error.into(),
    }
}

async fn audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &AuthContext,
    id: Uuid,
    action: &str,
    row: &FacilityRow,
) -> Result<(), AppError> {
    AuditService::log_in_tx(
        &mut **tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            "HEALTH_FACILITY",
            Some(id),
            "SUCCESS",
        )
        .with_after(json!({"publication_status":row.publication_status,"version":row.version})),
    )
    .await?;
    Ok(())
}

async fn find<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    id: Uuid,
) -> Result<Option<FacilityRow>, AppError> {
    Ok(sqlx::query_as::<_, FacilityRow>(&format!(
        "SELECT {COLUMNS} FROM health_facilities WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?)
}

fn storage(state: &AppState) -> Result<&std::sync::Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .article_media_storage
        .as_ref()
        .ok_or_else(|| AppError::Conflict("Facility photo storage is not configured.".into()))
}

fn object_key(media: Uuid) -> String {
    format!("facility-media/{media}.jpg")
}

// ---------------------------------------------------------------------------
// Patient (read-only, published only)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
pub struct DirectoryQuery {
    pub search: Option<String>,
    pub county: Option<String>,
    /// Comma-separated service codes; a facility matches if it offers any.
    pub services: Option<String>,
    pub youth_friendly: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn directory(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<DirectoryQuery>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let search = q.search.as_deref().unwrap_or("").trim().to_owned();
    if search.chars().count() > 100 || search.contains('\0') {
        return Err(AppError::Validation(
            "Search is limited to 100 characters.".into(),
        ));
    }
    let county = match q.county.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => Some(
            canonical_county(c)
                .ok_or_else(|| AppError::Validation("Unknown county.".into()))?
                .to_owned(),
        ),
        None => None,
    };
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let offset = q.offset.unwrap_or(0).clamp(0, 10_000);
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let services: Vec<String> = match q
        .services
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(list) => {
            let codes: Vec<String> = list
                .split(',')
                .map(|c| c.trim().to_ascii_uppercase())
                .filter(|c| !c.is_empty())
                .collect();
            if codes.len() > 10 || codes.iter().any(|c| !cats.iter().any(|k| &k.code == c)) {
                return Err(AppError::Validation("Unknown service filter.".into()));
            }
            codes
        }
        None => Vec::new(),
    };
    let filter = "WHERE f.publication_status='PUBLISHED' \
        AND ($1='' OR strpos(lower(f.name||' '||f.location_text||' '||f.county),lower($1))>0 \
             OR EXISTS(SELECT 1 FROM facility_service_categories c WHERE c.code=ANY(f.services) \
                       AND strpos(lower(c.name_en||' '||c.name_sw||' '||c.code),lower($1))>0)) \
        AND ($2::text IS NULL OR f.county=$2) \
        AND (cardinality($3::text[])=0 OR f.services && $3::text[]) \
        AND ($4::bool IS NULL OR f.youth_friendly=$4)";
    let rows: Vec<FacilityRow> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM health_facilities f {filter} ORDER BY lower(f.name),f.id LIMIT $5 OFFSET $6"
    ))
    .bind(&search)
    .bind(&county)
    .bind(&services)
    .bind(q.youth_friendly)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM health_facilities f {filter}"
    ))
    .bind(&search)
    .bind(&county)
    .bind(&services)
    .bind(q.youth_friendly)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let now = Utc::now();
    let next = (offset + (rows.len() as i64) < total).then_some(offset + rows.len() as i64);
    Ok(Json(json!({
        "success": true,
        "data": rows.iter().map(|r| r.patient_json(&cats, now)).collect::<Vec<_>>(),
        "total": total,
        "next_offset": next,
        "server_time": now,
    })))
}

pub async fn counties(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let list: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT county FROM health_facilities WHERE publication_status='PUBLISHED' ORDER BY county",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ok(json!(list)))
}

pub async fn detail(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let row = find(&mut *tx, id)
        .await?
        .filter(|r| r.publication_status == "PUBLISHED")
        .ok_or_else(|| AppError::NotFound("Facility not found.".into()))?;
    tx.commit().await?;
    let now = Utc::now();
    let mut data = row.patient_json(&cats, now);
    data["server_time"] = json!(now);
    Ok(ok(data))
}

pub async fn image(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    serve_image(&state, &ctx, id, true).await
}

async fn serve_image(
    state: &AppState,
    ctx: &AuthContext,
    id: Uuid,
    published_only: bool,
) -> Result<Response, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let row: Option<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT m.id,m.content_hash,m.size_bytes FROM health_facilities f \
         JOIN facility_media m ON m.id=f.image_media_id AND m.facility_id=f.id \
         WHERE f.id=$1 AND m.ready AND (NOT $2 OR f.publication_status='PUBLISHED')",
    )
    .bind(id)
    .bind(published_only)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let (media, hash, size) =
        row.ok_or_else(|| AppError::NotFound("Facility photo not found.".into()))?;
    let object = storage(state)?
        .get(&ObjectPath::from(object_key(media)))
        .await
        .map_err(|_| AppError::NotFound("Facility photo unavailable.".into()))?;
    if object.meta.size > (MAX_IMAGE_BYTES + 28) as u64 {
        return Err(AppError::internal("Facility photo size mismatch.".into()));
    }
    let encrypted = object
        .bytes()
        .await
        .map_err(|_| AppError::internal("Facility photo download failed.".into()))?;
    let bytes = crypto::decrypt_bytes(&encrypted, &state.config.storage_encryption_key_b64)?;
    if bytes.len() != size as usize || hex::encode(Sha256::digest(&bytes)) != hash {
        return Err(AppError::internal(
            "Facility photo integrity mismatch.".into(),
        ));
    }
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg".to_owned()),
            // A new photo gets a new media id (and URL), so a day's caching is safe.
            (header::CACHE_CONTROL, "private, max-age=86400".to_owned()),
            (header::ETAG, format!("\"{hash}\"")),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
pub struct AdminListQuery {
    pub search: Option<String>,
    pub county: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn admin_list(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<AdminListQuery>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let search = q.search.as_deref().unwrap_or("").trim().to_owned();
    if search.chars().count() > 120 {
        return Err(AppError::Validation(
            "Search is limited to 120 characters.".into(),
        ));
    }
    let status = q.status.as_deref().filter(|s| !s.is_empty());
    if status.is_some_and(|s| !matches!(s, "DRAFT" | "PUBLISHED" | "ARCHIVED")) {
        return Err(AppError::Validation("Unknown status.".into()));
    }
    let county = q.county.as_deref().filter(|c| !c.is_empty());
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let offset = q.offset.unwrap_or(0).max(0);
    let filter = "WHERE ($1='' OR strpos(lower(name),lower($1))>0) AND ($2::text IS NULL OR county=$2) AND ($3::text IS NULL OR publication_status=$3)";
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let rows: Vec<FacilityRow> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM health_facilities {filter} ORDER BY updated_at DESC,id LIMIT $4 OFFSET $5"
    ))
    .bind(&search)
    .bind(county)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;
    let total: i64 =
        sqlx::query_scalar(&format!("SELECT count(*) FROM health_facilities {filter}"))
            .bind(&search)
            .bind(county)
            .bind(status)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    let now = Utc::now();
    Ok(ok(json!({
        "items": rows.iter().map(|r| r.admin_json(&cats, now)).collect::<Vec<_>>(),
        "total": total, "limit": limit, "offset": offset
    })))
}

pub async fn admin_services(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    tx.commit().await?;
    Ok(ok(json!({
        "services": cats.iter().map(|c| json!({"code":c.code,"name_en":c.name_en,"name_sw":c.name_sw,"icon_key":c.icon_key})).collect::<Vec<_>>(),
        "counties": crate::domain::facilities::KENYA_COUNTIES.to_vec(),
    })))
}

pub async fn admin_detail(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let row = find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Facility not found.".into()))?;
    tx.commit().await?;
    Ok(ok(row.admin_json(&cats, Utc::now())))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateFacility {
    pub content: FacilityContent,
}

pub async fn admin_create(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(mut p): Json<CreateFacility>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let codes: Vec<String> = cats.iter().map(|c| c.code.clone()).collect();
    p.content.validate(&codes)?;
    let c = &p.content;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO health_facilities(id,name,description,county,location_text,contact_phone,google_maps_url,services,youth_friendly,opening_hours,created_by,updated_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)")
        .bind(id).bind(&c.name).bind(&c.description).bind(&c.county).bind(&c.location_text)
        .bind(&c.contact_phone).bind(&c.google_maps_url).bind(&c.services).bind(c.youth_friendly)
        .bind(c.opening_hours.as_ref().map(|h| json!(h))).bind(ctx.user_id)
        .execute(&mut *tx).await.map_err(db_error)?;
    let row = find(&mut *tx, id).await?.ok_or_else(conflict)?;
    audit(&mut tx, &ctx, id, "FACILITY_CREATE", &row).await?;
    tx.commit().await?;
    Ok(ok(row.admin_json(&cats, Utc::now())))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditFacility {
    pub version: i32,
    pub content: FacilityContent,
}

pub async fn admin_edit(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(mut p): Json<EditFacility>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let codes: Vec<String> = cats.iter().map(|c| c.code.clone()).collect();
    p.content.validate(&codes)?;
    let old = find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Facility not found.".into()))?;
    if old.version != p.version || old.publication_status == "ARCHIVED" {
        return Err(conflict());
    }
    if old.publication_status == "PUBLISHED" {
        let problems = publish_problems(&p.content, old.image_media_id.is_some());
        if !problems.is_empty() {
            return Err(AppError::Validation(format!(
                "A published facility must stay complete: {}",
                problems.join(" ")
            )));
        }
    }
    let c = &p.content;
    let changed = sqlx::query("UPDATE health_facilities SET name=$1,description=$2,county=$3,location_text=$4,contact_phone=$5,google_maps_url=$6,services=$7,youth_friendly=$8,opening_hours=$9,version=version+1,updated_at=now(),updated_by=$10 WHERE id=$11 AND version=$12")
        .bind(&c.name).bind(&c.description).bind(&c.county).bind(&c.location_text)
        .bind(&c.contact_phone).bind(&c.google_maps_url).bind(&c.services).bind(c.youth_friendly)
        .bind(c.opening_hours.as_ref().map(|h| json!(h))).bind(ctx.user_id).bind(id).bind(p.version)
        .execute(&mut *tx).await.map_err(db_error)?.rows_affected() == 1;
    if !changed {
        return Err(conflict());
    }
    let row = find(&mut *tx, id).await?.ok_or_else(conflict)?;
    audit(&mut tx, &ctx, id, "FACILITY_EDIT", &row).await?;
    tx.commit().await?;
    Ok(ok(row.admin_json(&cats, Utc::now())))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionBody {
    pub version: i32,
}

pub async fn admin_transition(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((id, action)): Path<(Uuid, String)>,
    Json(p): Json<VersionBody>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let old = find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Facility not found.".into()))?;
    if old.version != p.version {
        return Err(conflict());
    }
    let (sql, audit_action) = match (action.as_str(), old.publication_status.as_str()) {
        ("publish", "DRAFT") => {
            let problems = publish_problems(&old.content(), old.image_media_id.is_some());
            if !problems.is_empty() {
                return Err(AppError::Validation(format!(
                    "This facility cannot be published yet: {}",
                    problems.join(" ")
                )));
            }
            (
                "publication_status='PUBLISHED',published_at=now()",
                "FACILITY_PUBLISH",
            )
        }
        ("unpublish", "PUBLISHED") => ("publication_status='DRAFT'", "FACILITY_UNPUBLISH"),
        ("archive", "DRAFT" | "PUBLISHED") => (
            "publication_status='ARCHIVED',archived_at=now()",
            "FACILITY_ARCHIVE",
        ),
        _ => return Err(conflict()),
    };
    let changed = sqlx::query(&format!(
        "UPDATE health_facilities SET {sql},version=version+1,updated_at=now(),updated_by=$1 WHERE id=$2 AND version=$3"
    ))
    .bind(ctx.user_id)
    .bind(id)
    .bind(p.version)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?
    .rows_affected()
        == 1;
    if !changed {
        return Err(conflict());
    }
    let row = find(&mut *tx, id).await?.ok_or_else(conflict)?;
    audit(&mut tx, &ctx, id, audit_action, &row).await?;
    tx.commit().await?;
    Ok(ok(row.admin_json(&cats, Utc::now())))
}

#[derive(Deserialize)]
pub struct ImageQuery {
    pub version: i32,
}

pub async fn admin_image(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    serve_image(&state, &ctx, id, false).await
}

pub async fn admin_upload_image(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Query(q): Query<ImageQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    storage(&state)?;
    crate::rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _guard =
        crate::rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 1)
            .await?;
    {
        let mut tx = state.begin_authed(&ctx).await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM health_facilities WHERE id=$1 AND version=$2 AND publication_status<>'ARCHIVED')")
            .bind(id).bind(q.version).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        if !exists {
            return Err(conflict());
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
    {
        let mut tx = state.begin_authed(&ctx).await?;
        sqlx::query("INSERT INTO facility_media(id,facility_id,object_key,content_hash,size_bytes) VALUES($1,$2,$3,$4,$5)")
            .bind(media).bind(id).bind(&key).bind(&hash).bind(sanitized.len() as i32)
            .execute(&mut *tx).await?;
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
        .map_err(|_| AppError::internal("Facility photo upload failed.".into()))?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    sqlx::query("UPDATE facility_media SET ready=true WHERE id=$1")
        .bind(media)
        .execute(&mut *tx)
        .await?;
    let changed = sqlx::query("UPDATE health_facilities SET image_media_id=$1,version=version+1,updated_at=now(),updated_by=$2 WHERE id=$3 AND version=$4 AND publication_status<>'ARCHIVED'")
        .bind(media).bind(ctx.user_id).bind(id).bind(q.version)
        .execute(&mut *tx).await.map_err(db_error)?.rows_affected() == 1;
    if !changed {
        return Err(conflict());
    }
    let row = find(&mut *tx, id).await?.ok_or_else(conflict)?;
    audit(&mut tx, &ctx, id, "FACILITY_IMAGE", &row).await?;
    tx.commit().await?;
    Ok(ok(row.admin_json(&cats, Utc::now())))
}

/// Removes the photo of a draft. A published facility must keep one, so its
/// photo can only be replaced.
pub async fn admin_remove_image(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Query(q): Query<ImageQuery>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageFacilities)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let cats = categories(&mut *tx).await?;
    let changed = sqlx::query("UPDATE health_facilities SET image_media_id=NULL,version=version+1,updated_at=now(),updated_by=$1 WHERE id=$2 AND version=$3 AND publication_status='DRAFT'")
        .bind(ctx.user_id).bind(id).bind(q.version)
        .execute(&mut *tx).await.map_err(db_error)?.rows_affected() == 1;
    if !changed {
        return Err(conflict());
    }
    let row = find(&mut *tx, id).await?.ok_or_else(conflict)?;
    audit(&mut tx, &ctx, id, "FACILITY_IMAGE_REMOVE", &row).await?;
    tx.commit().await?;
    Ok(ok(row.admin_json(&cats, Utc::now())))
}
