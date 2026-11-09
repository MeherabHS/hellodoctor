//! Phase 1A preview generation: renders every email template in
//! `services::email_templates` through the **exact same Rust functions**
//! the backend would use to send them, using clearly fictional sample data,
//! and writes the resulting HTML to `docs/mail/previews/*.html` plus a
//! gallery `index.html`. No network access, no database, no Azure
//! credentials — inspect the output by opening the files in a browser.
//!
//! Usage (from `services/backend/`): `cargo run --example generate_email_previews`
//!
//! This binary renders content only — it does not send email, does not
//! touch `email_outbox`, and does not call `EmailTransport::send_email`.
//! See `services::email_templates::RenderedEmail::as_email_message` and its
//! tests for the (separately verified) proof that this exact output is
//! ready for the real transport once a trigger is activated.

use hellodoctor_backend::services::email_templates::{
    render_admin_login_otp, render_consultation_cancelled, render_consultation_confirmed,
    render_consultation_rescheduled, render_doctor_application_received, render_doctor_approved,
    render_doctor_login_otp, render_doctor_rejected, render_password_changed,
    render_password_reset_otp, render_patient_login_otp, render_patient_welcome,
    render_payment_confirmed, render_refund_processed, ConsultationCancelledParams,
    ConsultationConfirmedParams, ConsultationRescheduledParams, EmailAssets, EmailLinks,
    PaymentConfirmedParams, RefundProcessedParams, RenderedEmail,
};
use std::fs;
use std::path::Path;

struct PreviewEntry {
    file_name: &'static str,
    category: &'static str,
    description: &'static str,
    rendered: RenderedEmail,
}

fn main() {
    let out_dir = Path::new("../../docs/mail/previews");
    let assets_dir = out_dir.join("assets");
    fs::create_dir_all(&assets_dir).expect("create docs/mail/previews/assets");

    // Fictional, offline-safe sample data only — no real identifiers.
    let assets = EmailAssets::with_base_url("assets");
    let links = EmailLinks::new(
        Some("https://app.afyamkononi.app"),
        Some("https://doctor.afyamkononi.app"),
    );

    let consultation_confirmed_params = ConsultationConfirmedParams {
        first_name: "Amani",
        consultation_id: "CONS-FICT-10293",
        appointment_date: "Friday, 12 December 2026",
        appointment_time: "2:30 PM",
        timezone: "EAT (UTC+3)",
        doctor_name: "Njoroge",
    };
    let consultation_rescheduled_params = ConsultationRescheduledParams {
        first_name: "Amani",
        consultation_id: "CONS-FICT-10293",
        new_appointment_date: "Monday, 15 December 2026",
        new_appointment_time: "10:00 AM",
        timezone: "EAT (UTC+3)",
        doctor_name: "Njoroge",
    };
    let consultation_cancelled_params = ConsultationCancelledParams {
        first_name: "Amani",
        consultation_id: "CONS-FICT-10293",
        appointment_date: "Friday, 12 December 2026",
        appointment_time: "2:30 PM",
        timezone: "EAT (UTC+3)",
    };
    let payment_confirmed_params = PaymentConfirmedParams {
        first_name: "Amani",
        transaction_id: "TXN-FICT-55821",
        consultation_id: "CONS-FICT-10293",
        currency: "KES",
        amount_paid: "1,500.00",
        payment_date: "10 December 2026",
    };
    let refund_processed_params = RefundProcessedParams {
        first_name: "Amani",
        refund_reference: "RFND-FICT-00117",
        transaction_id: "TXN-FICT-55821",
        consultation_id: "CONS-FICT-10293",
        currency: "KES",
        refund_amount: "1,500.00",
        refund_date: "11 December 2026",
    };
    let entries = vec![
        PreviewEntry {
            file_name: "patient_welcome.html",
            category: "Onboarding",
            description: "Sent after a patient's first successful account activation.",
            rendered: render_patient_welcome("Amani", &assets, &links),
        },
        PreviewEntry {
            file_name: "doctor_application_received.html",
            category: "Onboarding",
            description: "Acknowledges a doctor's professional-registration application.",
            rendered: render_doctor_application_received("Njoroge", &assets),
        },
        PreviewEntry {
            file_name: "doctor_approved.html",
            category: "Onboarding",
            description: "Sent when a doctor's professional verification is approved.",
            rendered: render_doctor_approved("Njoroge", &assets, &links),
        },
        PreviewEntry {
            file_name: "doctor_rejected.html",
            category: "Onboarding",
            description: "Sent when a doctor's professional verification is not approved.",
            rendered: render_doctor_rejected("Njoroge", &assets),
        },
        PreviewEntry {
            file_name: "consultation_confirmed.html",
            category: "Consultations",
            description: "Sent when a patient's appointment booking is confirmed.",
            rendered: render_consultation_confirmed(&consultation_confirmed_params, &assets, &links),
        },
        PreviewEntry {
            file_name: "consultation_rescheduled.html",
            category: "Consultations",
            description: "Sent when a confirmed appointment's time changes.",
            rendered: render_consultation_rescheduled(
                &consultation_rescheduled_params,
                &assets,
                &links,
            ),
        },
        PreviewEntry {
            file_name: "consultation_cancelled.html",
            category: "Consultations",
            description: "Sent when a scheduled appointment is cancelled.",
            rendered: render_consultation_cancelled(&consultation_cancelled_params, &assets, &links),
        },
        PreviewEntry {
            file_name: "payment_confirmed.html",
            category: "Payments",
            description: "Sent when a consultation payment succeeds.",
            rendered: render_payment_confirmed(&payment_confirmed_params, &assets, &links),
        },
        PreviewEntry {
            file_name: "refund_processed.html",
            category: "Payments",
            description: "Sent when a refund has been processed.",
            rendered: render_refund_processed(&refund_processed_params, &assets, &links),
        },
        PreviewEntry {
            file_name: "password_changed.html",
            category: "Account security",
            description: "Sent immediately after a password change.",
            rendered: render_password_changed("Amani", &assets),
        },
        PreviewEntry {
            file_name: "password_reset_otp.html",
            category: "Account security",
            description: "Phase 2: live. Sent when a Patient or Doctor requests a self-service password reset.",
            rendered: render_password_reset_otp("Amani", "482913", 5, &assets),
        },
        PreviewEntry {
            file_name: "patient_login_otp.html",
            category: "Account security",
            description: "Live. Sent for Patient login/registration email+password verification, replacing the prior plain-text OTP email.",
            rendered: render_patient_login_otp("Amani", "482913", 5, &assets),
        },
        PreviewEntry {
            file_name: "doctor_login_otp.html",
            category: "Account security",
            description: "Live. Sent for Doctor second-factor login verification, replacing the prior plain-text OTP email.",
            rendered: render_doctor_login_otp("Njoroge", "482913", 5, &assets),
        },
        PreviewEntry {
            file_name: "admin_login_otp.html",
            category: "Account security",
            description: "Live. Sent for Admin second-factor login verification, replacing the prior plain-text OTP email.",
            rendered: render_admin_login_otp("Wanjiru", "482913", 5, &assets),
        },
    ];

    for entry in &entries {
        let path = out_dir.join(entry.file_name);
        fs::write(&path, &entry.rendered.html_body)
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }

    let index_html = build_index_html(&entries);
    let index_path = out_dir.join("index.html");
    fs::write(&index_path, index_html).expect("write index.html");
    println!("wrote {}", index_path.display());

    println!(
        "\nGenerated {} email previews from the live Rust templates. Open {} in a browser.",
        entries.len(),
        out_dir.join("index.html").display()
    );
}

fn build_index_html(entries: &[PreviewEntry]) -> String {
    let mut categories: Vec<&str> = Vec::new();
    for entry in entries {
        if !categories.contains(&entry.category) {
            categories.push(entry.category);
        }
    }

    let mut cards = String::new();
    for category in &categories {
        cards.push_str(&format!(
            "<h2 style=\"font-family:Arial,Helvetica,sans-serif;color:#164E59;font-size:16px;margin:32px 0 12px;\">{category}</h2>\n<div style=\"display:flex;flex-wrap:wrap;gap:16px;\">\n"
        ));
        for entry in entries.iter().filter(|e| &e.category == category) {
            cards.push_str(&format!(
                "<a href=\"{file}\" style=\"display:block;width:260px;text-decoration:none;border-radius:12px;overflow:hidden;border:1px solid #EAE7FA;background:#FFFFFF;box-shadow:0 1px 3px rgba(22,78,89,0.08);\">\
<div style=\"height:8px;background:#087F8C;\"></div>\
<div style=\"padding:16px;\">\
<div style=\"font-family:Arial,Helvetica,sans-serif;font-weight:bold;color:#164E59;font-size:14px;margin-bottom:6px;\">{subject}</div>\
<div style=\"font-family:Arial,Helvetica,sans-serif;color:#24343B;font-size:12px;line-height:1.5;\">{description}</div>\
<div style=\"font-family:Arial,Helvetica,sans-serif;color:#087F8C;font-size:11px;margin-top:10px;\">{file}</div>\
</div>\
</a>\n",
                file = entry.file_name,
                subject = html_escape_minimal(&entry.rendered.subject),
                description = html_escape_minimal(entry.description),
            ));
        }
        cards.push_str("</div>\n");
    }

    format!(
        "<!doctype html>\
<html lang=\"en\">\
<head>\
<meta charset=\"utf-8\" />\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\
<title>AfyaMkononi — Email Preview Gallery</title>\
</head>\
<body style=\"margin:0;padding:32px;background:#FAFAF6;font-family:Arial,Helvetica,sans-serif;color:#24343B;\">\
<div style=\"max-width:900px;margin:0 auto;\">\
<h1 style=\"color:#164E59;font-size:22px;margin:0 0 4px;\">AfyaMkononi — Email Preview Gallery</h1>\
<p style=\"color:#24343B;font-size:13px;margin:0 0 8px;\">Phase 1A. Every page below is the exact HTML a `services::email_templates` function renders — not a hand-written mockup. Sample data is fictional.</p>\
<p style=\"color:#5B6B72;font-size:12px;margin:0 0 24px;\">No event triggers are activated. No email is sent by viewing these pages.</p>\
{cards}\
</div>\
</body>\
</html>",
        cards = cards,
    )
}

/// Minimal escaping for this gallery page only — subjects/descriptions here
/// are backend-controlled static strings, not user input, but escaped
/// anyway on general principle.
fn html_escape_minimal(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
