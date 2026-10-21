//! Regression coverage for the profile-picture feature: migration
//! `20261112000001_profile_pictures`, `services/profile_picture_service.rs`,
//! `api/v1/profile_picture_handlers.rs`, and the new
//! `patient_repo`/`doctor_repo` `find_profile_picture`/`update_profile_picture`
//! helpers. These are real HTTP-level integration tests against the full
//! router (`use super::*;` pulls in `test_app`/`test_app_and_state`, the
//! `get`/`post`/`delete`/`json_body` request helpers, `make_token`/`uid`, the
//! seeded `DOCTOR1_*`/`DOCTOR2_*` identities, and `minimal_jpeg_base64`, the
//! same way `schedule_exclusivity.rs` and `security_acceptance.rs` do), so
//! auth middleware, ABAC, and RLS are genuinely exercised — not just the
//! service layer in isolation.
//!
//! `UploadSecurityService::sanitize_async`'s own validation internals (magic
//! bytes, decode/re-encode, the compression loop, decompression-bomb guards)
//! already have dedicated unit coverage in `upload_security_service.rs`
//! (private `#[cfg(test)]` fixtures in that module, not reusable from this
//! external integration-test crate). The malformed/spoofed-mime/oversized
//! tests below only prove `ProfilePictureService` correctly calls into it and
//! correctly surfaces both its rejections and its successful compression —
//! exercised once via the Patient path; the Doctor path shares the exact same
//! `sanitize_async` call, so it is not duplicated for Doctor here.
use super::*;
use object_store::path::Path;

// ============================================================================
// Local fixtures (kept separate from api_tests.rs's own and from
// schedule_exclusivity.rs's own, same rationale both give: these tests need
// direct control over the `patient_profiles.id`, not just a bearer token).
// ============================================================================

async fn new_patient(pool: &PgPool) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,'PATIENT'::user_role_enum,true,true,NOW(),NOW())")
        .bind(user_id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at, updated_at) \
         VALUES ($1, $2, 'Profile Picture Test Patient', '1990-01-01', 'OTHER', NOW(), NOW())",
    )
    .bind(profile_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    fixture_session(pool, user_id).await;
    (profile_id, make_token(user_id, "PATIENT"))
}

fn minimal_png_base64() -> String {
    let img = image::RgbImage::from_pixel(2, 2, image::Rgb([30, 144, 255]));
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
    STANDARD.encode(buf.into_inner())
}

/// A high-frequency synthetic photo whose naive encoding comfortably exceeds
/// `MAX_PRESCRIPTION_IMAGE_BYTES` (40KB) — a deterministic, fast-to-generate
/// stand-in for `upload_security_service.rs`'s own private RNG-based
/// `synthetic_large_photo`/`synthetic_worst_case_noise` fixtures (not
/// reusable here: they are private `#[cfg(test)]` functions in a different
/// crate-internal module). The per-pixel pattern is deliberately not a flat
/// gradient so JPEG's DCT compression cannot cheaply flatten it away, the
/// same way real worst-case input wouldn't.
fn large_photo_base64(format: image::ImageFormat, side: u32) -> String {
    let mut img = image::RgbImage::new(side, side);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = x.wrapping_mul(37).wrapping_add(y.wrapping_mul(17)) as u8;
        let g = x.wrapping_mul(11) as u8;
        let b = y.wrapping_mul(29) as u8;
        *pixel = image::Rgb([r, g, b]);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, format)
        .unwrap();
    STANDARD.encode(buf.into_inner())
}

/// Captures status/headers/body together — once the body is consumed via
/// `into_body().collect()`, the response is gone, so headers must be read
/// first.
async fn response_parts(
    res: axum::http::Response<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, bytes)
}

fn header_str(headers: &axum::http::HeaderMap, name: axum::http::HeaderName) -> &str {
    headers.get(name).unwrap().to_str().unwrap()
}

async fn upload_patient_photo(
    app: &axum::Router,
    token: &str,
    mime_type: &str,
    base64_content: &str,
    profile_id: Option<Uuid>,
) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/patients/me/photo",
        Some(token),
        &serde_json::json!({
            "mime_type": mime_type,
            "base64_content": base64_content,
            "profile_id": profile_id,
        }),
    )
    .await
}

async fn upload_doctor_photo(
    app: &axum::Router,
    token: &str,
    mime_type: &str,
    base64_content: &str,
) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/doctor/profile/photo",
        Some(token),
        &serde_json::json!({"mime_type": mime_type, "base64_content": base64_content}),
    )
    .await
}

// ============================================================================
// 1. Valid JPEG upload for a Patient succeeds; GET returns a valid,
//    <=40KB, correctly-typed, privately-cached image.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_jpeg_upload_get_roundtrip_private_cache_header(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (patient_id, token) = new_patient(&pool).await;

    let upload =
        upload_patient_photo(&app, &token, "image/jpeg", &minimal_jpeg_base64(), None).await;
    assert_eq!(
        upload.status(),
        StatusCode::OK,
        "a valid JPEG profile picture upload must succeed"
    );

    let (status, headers, bytes) =
        response_parts(get(&app, "/api/v1/patients/me/photo", Some(&token)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_str(&headers, axum::http::header::CONTENT_TYPE),
        "image/jpeg"
    );
    assert_eq!(
        header_str(&headers, axum::http::header::CACHE_CONTROL),
        "private, no-store"
    );
    assert!(
        bytes.len() <= 40 * 1024,
        "stored profile picture must respect the 40KB per-image cap, got {} bytes",
        bytes.len()
    );
    assert!(
        image::load_from_memory(&bytes).is_ok(),
        "bytes returned by GET must still decode as a valid image"
    );

    let size_in_db: Option<i32> =
        sqlx::query_scalar("SELECT profile_picture_size_bytes FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(size_in_db, Some(bytes.len() as i32));
}

// ============================================================================
// 2. Valid PNG upload for a Doctor succeeds; GET /doctor/profile/photo
//    (self) returns it.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_png_upload_self_get_roundtrip(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let upload = upload_doctor_photo(&app, &doctor_token, "image/png", &minimal_png_base64()).await;
    assert_eq!(upload.status(), StatusCode::OK);

    let (status, headers, bytes) =
        response_parts(get(&app, "/api/v1/doctor/profile/photo", Some(&doctor_token)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_str(&headers, axum::http::header::CONTENT_TYPE),
        "image/png"
    );
    assert_eq!(
        header_str(&headers, axum::http::header::CACHE_CONTROL),
        "private, no-store"
    );
    assert!(image::load_from_memory(&bytes).is_ok());
}

// ============================================================================
// 3. After a Doctor upload, a DIFFERENT authenticated Patient can GET
//    /doctors/:id/photo with the public cache header — proving the
//    SECURITY DEFINER `doctor_public_photo` path end-to-end, and that it is
//    not over-broad (a Doctor with no photo still 404s for the same viewer).
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_directory_photo_visible_to_other_patient_with_public_cache_header(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let upload = upload_doctor_photo(&app, &doctor_token, "image/png", &minimal_png_base64()).await;
    assert_eq!(upload.status(), StatusCode::OK);

    let (_viewer_id, viewer_token) = new_patient(&pool).await;
    let (status, headers, bytes) = response_parts(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/photo"),
            Some(&viewer_token),
        )
        .await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a different, authenticated Patient must be able to fetch a Doctor's \
         directory-visible photo via the SECURITY DEFINER path"
    );
    assert_eq!(
        header_str(&headers, axum::http::header::CACHE_CONTROL),
        "public, max-age=3600"
    );
    assert!(image::load_from_memory(&bytes).is_ok());

    // Negative control, same viewer: a Doctor with no uploaded photo must
    // 404, not leak an empty/garbage payload — proves
    // `doctor_public_photo`'s own `profile_picture_object_key IS NOT NULL`
    // guard actually gates the function, not just that the happy path works.
    let no_photo = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR2_PROFILE_ID}/photo"),
        Some(&viewer_token),
    )
    .await;
    assert_eq!(no_photo.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// 4. Malformed/garbage bytes are rejected; no DB row is updated; a
//    follow-up GET still shows no photo.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_malformed_image_bytes_rejected_no_photo_persisted(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (patient_id, token) = new_patient(&pool).await;
    let garbage = STANDARD.encode(b"this is not an image, just garbage padding bytes 1234567890");

    let upload = upload_patient_photo(&app, &token, "image/jpeg", &garbage, None).await;
    assert_eq!(upload.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let object_key: Option<String> =
        sqlx::query_scalar("SELECT profile_picture_object_key FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        object_key.is_none(),
        "a rejected malformed upload must not persist any object reference"
    );

    let follow_up = get(&app, "/api/v1/patients/me/photo", Some(&token)).await;
    assert_eq!(follow_up.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// 5. Spoofed MIME (real PNG bytes, declared as image/jpeg) is rejected.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_spoofed_mime_type_rejected(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (patient_id, token) = new_patient(&pool).await;

    let upload =
        upload_patient_photo(&app, &token, "image/jpeg", &minimal_png_base64(), None).await;
    assert_eq!(upload.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let object_key: Option<String> =
        sqlx::query_scalar("SELECT profile_picture_object_key FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(object_key.is_none());
}

// ============================================================================
// 6. A large valid image is accepted and compressed down to <=40KB, not
//    rejected.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_large_image_compressed_under_40kb_cap(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_patient_id, token) = new_patient(&pool).await;

    let original_b64 = large_photo_base64(image::ImageFormat::Jpeg, 1000);
    let original_len = STANDARD.decode(&original_b64).unwrap().len();
    assert!(
        original_len > 40 * 1024,
        "fixture setup check: the original upload must itself exceed the 40KB \
         cap for this test to prove anything, got {original_len} bytes"
    );

    let upload = upload_patient_photo(&app, &token, "image/jpeg", &original_b64, None).await;
    assert_eq!(
        upload.status(),
        StatusCode::OK,
        "an oversized but genuine image must be compressed, not rejected"
    );

    let (status, _headers, bytes) =
        response_parts(get(&app, "/api/v1/patients/me/photo", Some(&token)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        bytes.len() <= 40 * 1024,
        "sanitized image must be compressed to at most 40KB, got {} bytes",
        bytes.len()
    );
    assert!(image::load_from_memory(&bytes).is_ok());
}

// ============================================================================
// 7. Unauthorized access: Patient A cannot GET or DELETE Patient B's photo.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_cannot_view_or_delete_another_patients_photo(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (patient_b_id, token_b) = new_patient(&pool).await;
    let (_patient_a_id, token_a) = new_patient(&pool).await;

    let upload =
        upload_patient_photo(&app, &token_b, "image/jpeg", &minimal_jpeg_base64(), None).await;
    assert_eq!(upload.status(), StatusCode::OK);

    // Patient A attempts to address Patient B's photo via the profile_id
    // query param — the only addressing mechanism these endpoints expose.
    // Matches this codebase's existing non-leaky ownership-mismatch
    // convention (404, not 403) — see
    // `test_patient_cannot_see_or_update_another_patients_dependent`.
    let get_attempt = get(
        &app,
        &format!("/api/v1/patients/me/photo?profile_id={patient_b_id}"),
        Some(&token_a),
    )
    .await;
    assert_eq!(get_attempt.status(), StatusCode::NOT_FOUND);

    let delete_attempt = delete(
        &app,
        &format!("/api/v1/patients/me/photo?profile_id={patient_b_id}"),
        Some(&token_a),
    )
    .await;
    assert_eq!(delete_attempt.status(), StatusCode::NOT_FOUND);

    // B's photo must be completely unaffected by A's attempts.
    let (status, _headers, bytes) =
        response_parts(get(&app, "/api/v1/patients/me/photo", Some(&token_b)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!bytes.is_empty());
}

// ============================================================================
// 8. A Doctor cannot overwrite another Doctor's photo — there is no way to
//    even address another Doctor's upload endpoint (always
//    ctx.doctor_profile_id); confirmed by Doctor B's photo being unaffected
//    by Doctor A's upload.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_upload_does_not_affect_other_doctors_photo(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor1 = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let doctor2 = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    let upload1 = upload_doctor_photo(&app, &doctor1, "image/jpeg", &minimal_jpeg_base64()).await;
    assert_eq!(upload1.status(), StatusCode::OK);

    // Doctor2 never uploaded — proves Doctor1's upload created no cross-row
    // effect at all.
    let doctor2_own = get(&app, "/api/v1/doctor/profile/photo", Some(&doctor2)).await;
    assert_eq!(
        doctor2_own.status(),
        StatusCode::NOT_FOUND,
        "Doctor1's upload must not be visible as Doctor2's own photo"
    );

    let upload2 = upload_doctor_photo(&app, &doctor2, "image/png", &minimal_png_base64()).await;
    assert_eq!(upload2.status(), StatusCode::OK);

    let (_s1, h1, b1) =
        response_parts(get(&app, "/api/v1/doctor/profile/photo", Some(&doctor1)).await).await;
    let (_s2, h2, b2) =
        response_parts(get(&app, "/api/v1/doctor/profile/photo", Some(&doctor2)).await).await;
    assert_eq!(
        header_str(&h1, axum::http::header::CONTENT_TYPE),
        "image/jpeg"
    );
    assert_eq!(
        header_str(&h2, axum::http::header::CONTENT_TYPE),
        "image/png"
    );
    assert_ne!(
        b1, b2,
        "each Doctor's own photo must remain independently addressed by \
         ctx.doctor_profile_id, never cross-overwritten"
    );
}

// ============================================================================
// 9. Replace: a second upload replaces the first; the old object is no
//    longer fetchable from storage.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_photo_replace_old_object_no_longer_fetchable(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (patient_id, token) = new_patient(&pool).await;

    let upload1 =
        upload_patient_photo(&app, &token, "image/jpeg", &minimal_jpeg_base64(), None).await;
    assert_eq!(upload1.status(), StatusCode::OK);
    let old_key: String =
        sqlx::query_scalar("SELECT profile_picture_object_key FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let old_bytes = response_parts(get(&app, "/api/v1/patients/me/photo", Some(&token)).await)
        .await
        .2;

    let upload2 =
        upload_patient_photo(&app, &token, "image/png", &minimal_png_base64(), None).await;
    assert_eq!(upload2.status(), StatusCode::OK);
    let new_key: String =
        sqlx::query_scalar("SELECT profile_picture_object_key FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(
        old_key, new_key,
        "a replace must mint a brand-new object key, never reuse the old one"
    );

    let (status, headers, new_bytes) =
        response_parts(get(&app, "/api/v1/patients/me/photo", Some(&token)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_str(&headers, axum::http::header::CONTENT_TYPE),
        "image/png"
    );
    assert_ne!(new_bytes, old_bytes);

    let storage = state.object_storage.as_ref().unwrap();
    let old_object = storage.get(&Path::from(old_key.as_str())).await;
    assert!(
        matches!(old_object, Err(object_store::Error::NotFound { .. })),
        "the previous object must be deleted once the replacement has \
         committed, not left orphaned: {old_object:?}"
    );
}

// ============================================================================
// 10. Remove: DELETE clears it; a subsequent GET for the own-photo endpoint
//     returns 404.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_photo_remove_then_get_404(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (patient_id, token) = new_patient(&pool).await;
    let upload =
        upload_patient_photo(&app, &token, "image/jpeg", &minimal_jpeg_base64(), None).await;
    assert_eq!(upload.status(), StatusCode::OK);

    let remove = delete(&app, "/api/v1/patients/me/photo", Some(&token)).await;
    assert_eq!(remove.status(), StatusCode::OK);

    let after = get(&app, "/api/v1/patients/me/photo", Some(&token)).await;
    assert_eq!(after.status(), StatusCode::NOT_FOUND);

    let object_key: Option<String> =
        sqlx::query_scalar("SELECT profile_picture_object_key FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(object_key.is_none());
}

// ============================================================================
// 11. A guardian can upload/view a dependent's photo via profile_id; an
//     unrelated Patient cannot be impersonated by claiming the dependent's
//     id — ownership is enforced server-side via acting_patient_profile_id,
//     never trusted from the request.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_manages_dependent_photo_and_impersonation_rejected(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_guardian_id, guardian_token) = new_patient(&pool).await;
    let (_stranger_id, stranger_token) = new_patient(&pool).await;

    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&guardian_token),
            &serde_json::json!({
                "full_name": "Profile Picture Test Dependent",
                "date_of_birth": "2015-01-01",
                "gender": "OTHER",
                "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();

    // Guardian uploads on behalf of the dependent.
    let upload = upload_patient_photo(
        &app,
        &guardian_token,
        "image/jpeg",
        &minimal_jpeg_base64(),
        Some(dependent_id),
    )
    .await;
    assert_eq!(upload.status(), StatusCode::OK);

    // Guardian can view it scoped to the dependent.
    let dependent_view = get(
        &app,
        &format!("/api/v1/patients/me/photo?profile_id={dependent_id}"),
        Some(&guardian_token),
    )
    .await;
    assert_eq!(dependent_view.status(), StatusCode::OK);

    // The guardian's OWN photo endpoint (no profile_id) is unaffected —
    // dependent and self are fully independent rows.
    let guardian_self = get(&app, "/api/v1/patients/me/photo", Some(&guardian_token)).await;
    assert_eq!(
        guardian_self.status(),
        StatusCode::NOT_FOUND,
        "uploading for a dependent must not touch the guardian's own profile row"
    );

    // An unrelated Patient cannot address the same dependent by claiming its
    // id in profile_id.
    let impersonate_get = get(
        &app,
        &format!("/api/v1/patients/me/photo?profile_id={dependent_id}"),
        Some(&stranger_token),
    )
    .await;
    assert_eq!(impersonate_get.status(), StatusCode::NOT_FOUND);

    let impersonate_upload = upload_patient_photo(
        &app,
        &stranger_token,
        "image/png",
        &minimal_png_base64(),
        Some(dependent_id),
    )
    .await;
    assert_eq!(impersonate_upload.status(), StatusCode::NOT_FOUND);

    let impersonate_delete = delete(
        &app,
        &format!("/api/v1/patients/me/photo?profile_id={dependent_id}"),
        Some(&stranger_token),
    )
    .await;
    assert_eq!(impersonate_delete.status(), StatusCode::NOT_FOUND);

    // The dependent's real photo must be untouched by every impersonation
    // attempt above — still the guardian's original JPEG, never the
    // stranger's PNG, and never removed.
    let (status, headers, _bytes) = response_parts(
        get(
            &app,
            &format!("/api/v1/patients/me/photo?profile_id={dependent_id}"),
            Some(&guardian_token),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_str(&headers, axum::http::header::CONTENT_TYPE),
        "image/jpeg",
        "an impersonation attempt must never be able to overwrite or delete \
         the dependent's real photo"
    );

    let _ = pool;
}

// ============================================================================
// 12. Storage-layer failure: if DocumentStorageService::put fails, the DB
//     row is left unchanged (rollback), rather than referencing a key that
//     was never written. Uses the same pre-existing test seam as
//     `test_unconfigured_document_storage_does_not_confirm_upload`
//     (api_tests.rs) — `test_app_with_storage_fixture(..., false)` leaves
//     `AppState.object_storage` unset, which `DocumentStorageService::put`
//     already turns into a clean `AppError::Conflict` before ever touching
//     the database row.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_storage_unavailable_leaves_patient_photo_row_unchanged(pool: PgPool) {
    let (app, pool) = test_app_with_storage_fixture(pool, test_config(), false).await;
    let (patient_id, token) = new_patient(&pool).await;

    let upload =
        upload_patient_photo(&app, &token, "image/jpeg", &minimal_jpeg_base64(), None).await;
    let status = upload.status();
    let body = json_body(upload).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not configured"));

    let object_key: Option<String> =
        sqlx::query_scalar("SELECT profile_picture_object_key FROM patient_profiles WHERE id=$1")
            .bind(patient_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        object_key.is_none(),
        "a storage failure must roll back the transaction, never leaving a \
         dangling object-key reference"
    );
}
