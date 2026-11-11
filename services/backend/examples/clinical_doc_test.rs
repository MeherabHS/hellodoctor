//! Narrowly-scoped real test of the clinical document (patient prescription
//! intake) upload/download flow against Config B, for Milestone 8 Workstream
//! 2. Uses the REAL authenticated JSON+base64 contract (NOT multipart — the
//! real endpoint, `POST /api/v1/appointments/:id/prescriptions`, takes
//! `{"files":[{"mime_type","base64_content"}]}`, confirmed by reading
//! `src/api/v1/prescription_handlers.rs`), the real 20-image-per-appointment
//! cap (`PrescriptionService::PATIENT_MAX_IMAGES_PER_CONSULTATION`), and the
//! real token-gated download flow
//! (`GET /appointments/:id/documents/:document_id` -> short-lived Redis
//! grant -> `GET /documents/download?token=...`).
//!
//! Usage: CAPACITY_BASE_URL=http://localhost:8180 \
//!   CAPACITY_DATABASE_URL=postgres://hellodoctor:hellodoctor_dev_password@localhost:5532/hellodoctor \
//!   APPOINTMENT_ID=<uuid> PATIENT_USER_ID=<uuid> \
//!   cargo run --release --example clinical_doc_test
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hellodoctor_backend::crypto::create_access_token;
use std::env;
use std::time::Instant;
use uuid::Uuid;

/// Generates a genuinely decodable small JPEG (not hand-rolled magic bytes
/// -- the real upload path re-decodes the image to validate it, confirmed
/// live: a hand-crafted byte sequence was correctly rejected with "File
/// claims to be an image but could not be decoded", proving the sanitizer
/// does real decode validation, not just a magic-byte/MIME check).
fn make_tiny_jpeg() -> Vec<u8> {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(32, 32, |x, y| {
        Rgb([((x * 7) % 256) as u8, ((y * 11) % 256) as u8, 128])
    });
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Jpeg,
    )
    .expect("failed to encode test JPEG");
    assert!(
        bytes.len() < 40 * 1024,
        "generated test JPEG unexpectedly exceeds the 40KB cap"
    );
    bytes
}

fn elapsed_ms(start: Instant) -> u128 {
    start.elapsed().as_millis()
}

#[tokio::main]
async fn main() {
    let base_url =
        env::var("CAPACITY_BASE_URL").unwrap_or_else(|_| "http://localhost:8180".to_string());
    let database_url = env::var("CAPACITY_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://hellodoctor:hellodoctor_dev_password@localhost:5532/hellodoctor".to_string()
    });
    let jwt_private_key =
        env::var("JWT_ED25519_PRIVATE_KEY_B64").expect("JWT_ED25519_PRIVATE_KEY_B64 must be set");

    let appointment_id: Uuid = env::var("APPOINTMENT_ID")
        .expect("APPOINTMENT_ID must be set")
        .parse()
        .expect("APPOINTMENT_ID must be a UUID");
    let patient_user_id: Uuid = env::var("PATIENT_USER_ID")
        .expect("PATIENT_USER_ID must be set")
        .parse()
        .expect("PATIENT_USER_ID must be a UUID");

    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("failed to connect to Config B postgres");

    let patient_token = create_access_token(
        patient_user_id,
        "PATIENT",
        patient_user_id,
        &jwt_private_key,
    )
    .expect("failed to mint patient token");
    // A second, unrelated seeded patient for the negative (unauthorized-access) test.
    let other_patient_user_id: Option<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM patient_profiles WHERE user_id <> $1 LIMIT 1")
            .bind(patient_user_id)
            .fetch_optional(&pg)
            .await
            .ok()
            .flatten();
    let other_token = other_patient_user_id.map(|uid| {
        create_access_token(uid, "PATIENT", uid, &jwt_private_key)
            .expect("failed to mint second patient token")
    });

    let client = reqwest::Client::new();
    let tiny_jpeg = make_tiny_jpeg();
    println!("generated test JPEG: {} bytes", tiny_jpeg.len());
    let b64 = STANDARD.encode(&tiny_jpeg);

    println!("=== Clinical document upload/download test (Config B, real contract) ===");
    println!("target={base_url} appointment_id={appointment_id} patient_user_id={patient_user_id}");

    // 1. Single valid-sized upload (1 image) -- exercises the real path
    //    through validation -> sanitize -> (attempted) encrypted storage put.
    let t0 = Instant::now();
    let resp = client
        .post(format!(
            "{base_url}/api/v1/appointments/{appointment_id}/prescriptions"
        ))
        .bearer_auth(&patient_token)
        .json(&serde_json::json!({
            "files": [{"mime_type": "image/jpeg", "base64_content": b64}]
        }))
        .send()
        .await
        .expect("request failed");
    let status1 = resp.status();
    let ms1 = elapsed_ms(t0);
    let body1 = resp.text().await.unwrap_or_default();
    println!("\n[1] Single-image upload: status={status1} latency_ms={ms1}\n    body={body1}");
    let uploaded_doc_id: Option<Uuid> = serde_json::from_str::<serde_json::Value>(&body1)
        .ok()
        .and_then(|v| {
            v["data"]["documents"]
                .as_array()
                .and_then(|docs| docs.first())
                .and_then(|d| d["id"].as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
        });

    if let Some(doc_id) = uploaded_doc_id {
        let t0b = Instant::now();
        let url_resp = client
            .get(format!(
                "{base_url}/api/v1/appointments/{appointment_id}/documents/{doc_id}"
            ))
            .bearer_auth(&patient_token)
            .send()
            .await
            .expect("request failed");
        let url_status = url_resp.status();
        let url_ms = elapsed_ms(t0b);
        let url_body = url_resp.text().await.unwrap_or_default();
        println!(
            "\n[1b] Get download-url for uploaded doc {doc_id}: status={url_status} latency_ms={url_ms}\n     body={url_body}"
        );
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&url_body) {
            if let Some(download_url) = v["data"]["download_url"].as_str() {
                let t0c = Instant::now();
                let dl_resp = client
                    .get(format!("{base_url}{download_url}"))
                    .send()
                    .await
                    .expect("request failed");
                let dl_status = dl_resp.status();
                let dl_ms = elapsed_ms(t0c);
                let dl_bytes = dl_resp.bytes().await.unwrap_or_default();
                let roundtrip_match = dl_bytes.as_ref() == tiny_jpeg.as_slice();
                println!(
                    "\n[1c] Actual document download: status={dl_status} latency_ms={dl_ms} bytes={} roundtrip_byte_match={roundtrip_match}",
                    dl_bytes.len()
                );
            }
        }
    } else {
        println!(
            "\n[1b/1c] SKIPPED -- upload did not return a document id (status={status1}), see body above for why"
        );
    }

    // 2. Over-cap single request: 21 images in one call -> must be 422,
    //    rejected BEFORE any storage/encryption work (pure validation path).
    let too_many: Vec<_> = (0..21)
        .map(|_| serde_json::json!({"mime_type": "image/jpeg", "base64_content": b64}))
        .collect();
    let t1 = Instant::now();
    let resp2 = client
        .post(format!(
            "{base_url}/api/v1/appointments/{appointment_id}/prescriptions"
        ))
        .bearer_auth(&patient_token)
        .json(&serde_json::json!({ "files": too_many }))
        .send()
        .await
        .expect("request failed");
    let status2 = resp2.status();
    let ms2 = elapsed_ms(t1);
    let body2 = resp2.text().await.unwrap_or_default();
    println!(
        "\n[2] 21-image single-request cap test: status={status2} latency_ms={ms2}\n    body={body2}\n    EXPECTED: 422 Unprocessable Entity (PATIENT_MAX_IMAGES_PER_CONSULTATION=20) -- {}",
        if status2.as_u16() == 422 { "PASS" } else { "FAIL / UNEXPECTED" }
    );

    // 3. Download-URL request for a random (non-existent) document id as this
    //    patient -- must 404, never disclose existence.
    let fake_doc_id = Uuid::new_v4();
    let t2 = Instant::now();
    let resp3 = client
        .get(format!(
            "{base_url}/api/v1/appointments/{appointment_id}/documents/{fake_doc_id}"
        ))
        .bearer_auth(&patient_token)
        .send()
        .await
        .expect("request failed");
    let status3 = resp3.status();
    let ms3 = elapsed_ms(t2);
    println!(
        "\n[3] Download-URL for non-existent document: status={status3} latency_ms={ms3} -- EXPECTED 404 -- {}",
        if status3.as_u16() == 404 { "PASS" } else { "FAIL / UNEXPECTED" }
    );

    // 4. Same non-existent document id, but as a DIFFERENT, unrelated patient
    //    -- negative authorization test. Must also 404 (not a different
    //    error that would disclose anything).
    if let Some(other) = &other_token {
        let t3 = Instant::now();
        let resp4 = client
            .get(format!(
                "{base_url}/api/v1/appointments/{appointment_id}/documents/{fake_doc_id}"
            ))
            .bearer_auth(other)
            .send()
            .await
            .expect("request failed");
        let status4 = resp4.status();
        let ms4 = elapsed_ms(t3);
        println!(
            "\n[4] Download-URL as unrelated patient: status={status4} latency_ms={ms4} -- EXPECTED 404 -- {}",
            if status4.as_u16() == 404 { "PASS" } else { "FAIL / UNEXPECTED" }
        );
    } else {
        println!("\n[4] SKIPPED -- no second seeded patient found for the negative test");
    }

    println!("\n=== Done ===");
}
