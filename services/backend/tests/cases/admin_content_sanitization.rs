//! Admin uploads are sanitized at the source (product owner, 2026-11-14):
//! articles, categories, campaigns and facilities refuse HTML tags, script
//! links and invisible formatting characters, while ordinary health text
//! (including "<" in "CD4 < 200") is accepted unchanged.
use super::*;
use serde_json::json;

async fn status_of(app: &axum::Router, path: &str, token: &str, body: &Value) -> StatusCode {
    post(app, path, Some(token), body).await.status()
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_text_is_sanitized_everywhere(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Categories.
    for bad in ["<b>Bold</b>", "Name\u{202E}lmth", "Two\nlines"] {
        let body = json!({"name_en": bad, "name_sw": "Jina", "sensitive": false});
        assert_eq!(
            status_of(&app, "/api/v1/admin/article-categories", &admin, &body).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad:?}"
        );
    }
    let res = post(
        &app,
        "/api/v1/admin/article-categories",
        Some(&admin),
        &json!({"name_en": "Testing & PrEP", "name_sw": "Upimaji", "sensitive": false}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let category = json_body(res).await["data"].clone();

    // Articles.
    let article = |title: &str, body: &str| json!({"id": Uuid::new_v4(), "content": {"title_en": title, "title_sw": "", "body_en": body, "body_sw": "", "category_id": category["id"], "tags": ["hiv"], "featured": false, "sensitive": false}});
    for (title, body) in [
        ("Safe title", "<script>alert(1)</script>"),
        ("Safe title", "Read [this](javascript:alert(1))"),
        ("Safe title", "![x](data:image/svg+xml;base64,PHN2Zz4=)"),
        ("Safe title", "<iframe src=https://evil.example></iframe>"),
        ("<img src=x onerror=alert(1)>", "Body"),
        ("Title\u{200B}", "Body"),
    ] {
        assert_eq!(
            status_of(
                &app,
                "/api/v1/admin/articles",
                &admin,
                &article(title, body)
            )
            .await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{title:?} / {body:?}"
        );
    }
    let ok = article("Know your status", "## Steps\n\nCD4 < 200 needs care.\n\n- Visit [the clinic](https://example.org)\n- Call [us](tel:+254700000000)");
    let res = post(&app, "/api/v1/admin/articles", Some(&admin), &ok).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "ordinary Markdown is accepted"
    );
    let saved = json_body(res).await;
    assert!(
        saved.to_string().contains("CD4 < 200"),
        "text is stored unchanged"
    );

    // Campaigns (push notifications).
    let campaign = |title: &str, body: &str| json!({"creation_key": Uuid::new_v4().to_string(), "content": {"title_en": title, "title_sw": "Kichwa", "body_en": body, "body_sw": "Ujumbe", "audience": "PATIENTS"}});
    for (title, body) in [
        ("<b>Hi</b>", "Message"),
        ("Hi", "<a href=x>tap</a>"),
        ("Hi", "Open javascript:void(0)"),
        ("Hi\u{202E}", "Message"),
    ] {
        assert_eq!(
            status_of(
                &app,
                "/api/v1/admin/campaigns",
                &admin,
                &campaign(title, body)
            )
            .await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{title:?} / {body:?}"
        );
    }
    assert!(status_of(
        &app,
        "/api/v1/admin/campaigns",
        &admin,
        &campaign("Clinic hours", "Open 8 < 5 days a week")
    )
    .await
    .is_success());

    // Facilities.
    let facility = |name: &str, description: &str| json!({"content": {"name": name, "description": description, "county": "Siaya", "location_text": "Siaya town", "contact_phone": "0712 345 678", "google_maps_url": "https://www.google.com/maps/place/Siaya", "services": [], "youth_friendly": true, "opening_hours": {}}});
    for (name, description) in [
        ("<script>x</script> Clinic", "Fine"),
        ("Siaya Clinic", "<img src=x onerror=alert(1)>"),
        ("Siaya\u{2066}Clinic", "Fine"),
    ] {
        assert_eq!(
            status_of(
                &app,
                "/api/v1/admin/facilities",
                &admin,
                &facility(name, description)
            )
            .await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{name:?} / {description:?}"
        );
    }
    assert_eq!(
        status_of(
            &app,
            "/api/v1/admin/facilities",
            &admin,
            &facility(
                "Siaya Youth Clinic",
                "Testing & counselling.\nWalk-ins welcome."
            )
        )
        .await,
        StatusCode::OK
    );
}
