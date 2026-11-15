//! Facilities & Services: Admin CRUD/publication, patient visibility (published
//! only), photo upload authorization and validation.
use super::*;
use serde_json::json;

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(8, 6, image::Rgb([20, 120, 80]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn complete(name: &str, county: &str, services: &[&str]) -> Value {
    json!({
        "name": name,
        "description": "Youth-friendly HIV testing and counselling.",
        "county": county,
        "location_text": "Siaya town, Kenya",
        "contact_phone": "0712 345 678",
        "google_maps_url": "https://www.google.com/maps/place/Siaya+County+Hospital",
        "services": services,
        "youth_friendly": true,
        "opening_hours": {
            "MON": {"periods":[{"open":"08:00","close":"17:00"}]},
            "TUE": {"periods":[{"open":"08:00","close":"12:00"},{"open":"14:00","close":"17:00"}]},
            "SAT": {"all_day": true}
        }
    })
}

async fn create(app: &axum::Router, admin: &str, content: Value) -> Value {
    let res = post(
        app,
        "/api/v1/admin/facilities",
        Some(admin),
        &json!({"content": content}),
    )
    .await;
    let status = res.status();
    let body = json_body(res).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"].clone()
}

async fn upload(
    app: &axum::Router,
    token: &str,
    f: &Value,
    mime: &str,
    bytes: Vec<u8>,
) -> axum::http::Response<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/admin/facilities/{}/image?version={}",
            f["id"].as_str().unwrap(),
            f["version"]
        ))
        .header("Content-Type", mime)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(bytes))
        .unwrap();
    req.extensions_mut().insert(random_peer());
    app.clone().oneshot(req).await.unwrap()
}

async fn act(
    app: &axum::Router,
    admin: &str,
    f: &Value,
    action: &str,
) -> axum::http::Response<Body> {
    post(
        app,
        &format!(
            "/api/v1/admin/facilities/{}/{action}",
            f["id"].as_str().unwrap()
        ),
        Some(admin),
        &json!({"version": f["version"]}),
    )
    .await
}

async fn publish_new(
    app: &axum::Router,
    admin: &str,
    name: &str,
    county: &str,
    services: &[&str],
    youth: bool,
) -> Value {
    let mut content = complete(name, county, services);
    content["youth_friendly"] = json!(youth);
    let f = create(app, admin, content).await;
    let f = json_body(upload(app, admin, &f, "image/png", png()).await).await["data"].clone();
    let res = act(app, admin, &f, "publish").await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["data"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_facility_lifecycle_and_patient_visibility(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // A draft with only a name is allowed and canonicalised.
    let draft = create(
        &app,
        &admin,
        json!({"name":"  Draft Clinic ","county":"siaya","contact_phone":"+254 712 345 678"}),
    )
    .await;
    assert_eq!(draft["name"], "Draft Clinic");
    assert_eq!(draft["county"], "Siaya");
    assert_eq!(draft["contact_phone"], "+254712345678");
    assert_eq!(draft["publication_status"], "DRAFT");
    // It cannot be published: missing fields and photo.
    let res = act(&app, &admin, &draft, "publish").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let msg = json_body(res).await.to_string();
    assert!(msg.contains("photo") && msg.contains("Location"), "{msg}");

    let f = create(
        &app,
        &admin,
        complete(
            "Siaya County Hospital",
            "Siaya",
            &["TESTING", "PREP", "ART"],
        ),
    )
    .await;
    let id = f["id"].as_str().unwrap().to_owned();
    // Drafts are invisible to patients, by list and by id.
    let list = json_body(get(&app, "/api/v1/facilities", Some(&patient)).await).await;
    assert_eq!(list["total"], 0);
    assert_eq!(
        get(&app, &format!("/api/v1/facilities/{id}"), Some(&patient))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    // Still not publishable without a photo.
    assert_eq!(
        act(&app, &admin, &f, "publish").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let f = json_body(upload(&app, &admin, &f, "image/png", png()).await).await["data"].clone();
    assert_eq!(f["has_image"], true);
    let res = act(&app, &admin, &f, "publish").await;
    assert_eq!(res.status(), StatusCode::OK);
    let f = json_body(res).await["data"].clone();
    assert_eq!(f["publication_status"], "PUBLISHED");

    // Patients see display fields only: no audit fields or storage keys.
    let list = json_body(get(&app, "/api/v1/facilities", Some(&patient)).await).await;
    assert_eq!(list["total"], 1);
    let p = &list["data"][0];
    assert_eq!(p["name"], "Siaya County Hospital");
    assert_eq!(p["contact_phone"], "+254712345678");
    assert_eq!(p["services"].as_array().unwrap().len(), 3);
    assert_eq!(p["services"][0]["code"], "TESTING");
    assert!(p["status"]["state"].is_string());
    for hidden in [
        "created_by",
        "updated_by",
        "image_media_id",
        "version",
        "publication_status",
    ] {
        assert!(p.get(hidden).is_none(), "{hidden} leaked");
    }
    let image_url = p["image_url"].as_str().unwrap().to_owned();
    assert!(!image_url.contains("facility-media"));
    let res = get(&app, &format!("/api/v1{image_url}"), Some(&patient)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/jpeg");
    let detail =
        json_body(get(&app, &format!("/api/v1/facilities/{id}"), Some(&patient)).await).await;
    assert_eq!(detail["data"]["opening_hours"]["SAT"]["all_day"], true);

    // Unpublish hides it again, including its photo.
    let res = act(&app, &admin, &f, "unpublish").await;
    assert_eq!(res.status(), StatusCode::OK);
    let f = json_body(res).await["data"].clone();
    assert_eq!(
        get(&app, &format!("/api/v1/facilities/{id}"), Some(&patient))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, &format!("/api/v1{image_url}"), Some(&patient))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    // Archive is final and hidden.
    let res = act(&app, &admin, &f, "archive").await;
    assert_eq!(res.status(), StatusCode::OK);
    let f = json_body(res).await["data"].clone();
    assert_eq!(f["publication_status"], "ARCHIVED");
    assert_eq!(
        act(&app, &admin, &f, "publish").await.status(),
        StatusCode::CONFLICT
    );
    let res = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/facilities/{id}"),
        Some(&admin),
        Some(
            &json!({"version": f["version"], "content": complete("X Clinic", "Siaya", &["PREP"])}),
        ),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let list = json_body(get(&app, "/api/v1/facilities", Some(&patient)).await).await;
    assert_eq!(list["total"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_facility_writes_are_admin_only(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let f = create(
        &app,
        &admin,
        complete("Kisumu County Hospital", "Kisumu", &["PREP"]),
    )
    .await;
    let id = f["id"].as_str().unwrap();
    for token in [&patient, &doctor] {
        assert_eq!(
            post(
                &app,
                "/api/v1/admin/facilities",
                Some(token),
                &json!({"content": complete("Evil", "Kisumu", &["PREP"])})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(&app, "PATCH", &format!("/api/v1/admin/facilities/{id}"), Some(token),
                Some(&json!({"version": f["version"], "content": complete("Evil", "Kisumu", &["PREP"])})), None).await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            act(&app, token, &f, "publish").await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            upload(&app, token, &f, "image/png", png()).await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get(&app, "/api/v1/admin/facilities", Some(token))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get(
                &app,
                &format!("/api/v1/admin/facilities/{id}/image"),
                Some(token)
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    // Unauthenticated callers get nothing at all.
    assert_eq!(
        get(&app, "/api/v1/facilities", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        upload(&app, "not-a-token", &f, "image/png", png())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // Existing Admin permissions are unchanged: article management still works.
    assert_eq!(
        get(&app, "/api/v1/admin/article-categories", Some(&admin))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/v1/admin/article-categories", Some(&patient))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_facility_validation(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    for (field, value) in [
        ("google_maps_url", json!("javascript:alert(1)")),
        (
            "google_maps_url",
            json!("http://www.google.com/maps/place/x"),
        ),
        (
            "google_maps_url",
            json!("https://www.google.com.evil.example/maps"),
        ),
        (
            "google_maps_url",
            json!("https://user:pass@www.google.com/maps"),
        ),
        (
            "google_maps_url",
            json!("https://evil.example/?u=google.com/maps"),
        ),
        ("contact_phone", json!("+1 202 555 0100")),
        ("contact_phone", json!("12345")),
        ("county", json!("Atlantis")),
        ("services", json!(["SURGERY"])),
        (
            "opening_hours",
            json!({"MON": {"periods":[{"open":"17:00","close":"08:00"}]}}),
        ),
        (
            "opening_hours",
            json!({"MON": {"periods":[{"open":"08:00","close":"13:00"},{"open":"12:00","close":"17:00"}]}}),
        ),
    ] {
        let mut content = complete("Validation Clinic", "Siaya", &["PREP"]);
        content[field] = value.clone();
        let res = post(
            &app,
            "/api/v1/admin/facilities",
            Some(&admin),
            &json!({"content": content}),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field} = {value}"
        );
    }
    // Wrong MIME and non-image bytes are refused; nothing is attached.
    let f = create(&app, &admin, complete("Image Clinic", "Siaya", &["PREP"])).await;
    assert_eq!(
        upload(&app, &admin, &f, "image/gif", png()).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        upload(&app, &admin, &f, "image/png", b"not an image".to_vec())
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let fresh = json_body(
        get(
            &app,
            &format!("/api/v1/admin/facilities/{}", f["id"].as_str().unwrap()),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(fresh["data"]["has_image"], false);
    // A stale version is a conflict, not a silent overwrite.
    let res = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/facilities/{}", f["id"].as_str().unwrap()),
        Some(&admin),
        Some(&json!({"version": 99, "content": complete("Image Clinic", "Siaya", &["PREP"])})),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_facility_directory_search_and_filters(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    publish_new(
        &app,
        &admin,
        "Siaya County Hospital",
        "Siaya",
        &["TESTING", "PREP", "ART"],
        true,
    )
    .await;
    publish_new(
        &app,
        &admin,
        "Kisumu County Hospital",
        "Kisumu",
        &["TESTING"],
        false,
    )
    .await;
    publish_new(
        &app,
        &admin,
        "Migunga Health Centre",
        "Siaya",
        &["COUNSELLING"],
        true,
    )
    .await;
    let names = |body: &Value| -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let q = |path: &'static str| get(&app, path, Some(&patient));
    let all = json_body(q("/api/v1/facilities").await).await;
    // Stable alphabetical order.
    assert_eq!(
        names(&all),
        vec![
            "Kisumu County Hospital",
            "Migunga Health Centre",
            "Siaya County Hospital"
        ]
    );
    assert_eq!(
        names(&json_body(q("/api/v1/facilities?county=siaya").await).await),
        vec!["Migunga Health Centre", "Siaya County Hospital"]
    );
    assert_eq!(
        names(&json_body(q("/api/v1/facilities?services=PREP,ART").await).await),
        vec!["Siaya County Hospital"]
    );
    assert_eq!(
        names(&json_body(q("/api/v1/facilities?services=TESTING").await).await).len(),
        2
    );
    assert_eq!(
        names(&json_body(q("/api/v1/facilities?youth_friendly=true").await).await),
        vec!["Migunga Health Centre", "Siaya County Hospital"]
    );
    // Search covers name, location, county and service names.
    assert_eq!(
        names(&json_body(q("/api/v1/facilities?search=kisumu").await).await),
        vec!["Kisumu County Hospital"]
    );
    assert_eq!(
        names(&json_body(q("/api/v1/facilities?search=counselling").await).await),
        vec!["Migunga Health Centre"]
    );
    let none = json_body(q("/api/v1/facilities?search=nowhere").await).await;
    assert_eq!(none["total"], 0);
    assert_eq!(none["data"].as_array().unwrap().len(), 0);
    // Pagination is stable and reports the next page.
    let page1 = json_body(q("/api/v1/facilities?limit=2").await).await;
    assert_eq!(page1["next_offset"], 2);
    let page2 = json_body(q("/api/v1/facilities?limit=2&offset=2").await).await;
    assert_eq!(names(&page2), vec!["Siaya County Hospital"]);
    assert!(page2["next_offset"].is_null());
    let counties = json_body(q("/api/v1/facilities/counties").await).await;
    assert_eq!(counties["data"], json!(["Kisumu", "Siaya"]));
    for bad in [
        "/api/v1/facilities?county=Atlantis",
        "/api/v1/facilities?services=SURGERY",
    ] {
        assert_eq!(
            q(bad).await.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}"
        );
    }
}
