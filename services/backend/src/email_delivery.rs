use crate::config::AppConfig;
use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// Sends a Patient or Admin MFA code through Azure Communication Services
/// Email — the single generic sender both login flows share (neither one
/// re-implements the Azure REST call independently). In the development test
/// environment the code is returned by the API instead, so local device
/// testing does not require Azure secrets.
pub async fn send_email_otp(
    config: &AppConfig,
    recipient: &str,
    code: &str,
) -> Result<(), AppError> {
    if config.environment == "development" && config.dev_auth_otp_enabled {
        return Ok(());
    }
    let endpoint = config
        .azure_email_endpoint
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Azure email OTP delivery is not configured.".into()))?;
    let access_key = config
        .azure_email_access_key
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Azure email OTP delivery is not configured.".into()))?;
    let sender = config
        .azure_email_sender
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Azure email OTP sender is not configured.".into()))?;
    let endpoint = endpoint.trim_end_matches('/');
    let url = format!("{endpoint}/emails:send?api-version=2023-03-31");
    let uri = url
        .parse::<reqwest::Url>()
        .map_err(|_| AppError::internal("Invalid Azure email endpoint.".into()))?;
    if uri.scheme() != "https"
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.port().is_some()
    {
        return Err(AppError::internal(
            "Azure email endpoint must use HTTPS with no embedded credentials or custom port."
                .into(),
        ));
    }
    let host = uri
        .host_str()
        .ok_or_else(|| AppError::internal("Azure email endpoint has no host.".into()))?;
    let path_query = uri.path().to_string()
        + uri
            .query()
            .map(|q| format!("?{q}"))
            .as_deref()
            .unwrap_or("");
    let body = serde_json::json!({
        "senderAddress": sender,
        "content": {"subject": "AfyaMkononi verification code", "plainText": format!("Your AfyaMkononi verification code is {code}. It expires in 5 minutes. If you did not request this, ignore this email.")},
        "recipients": {"to": [{"address": recipient}]}
    });
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|_| AppError::internal("Could not encode Azure email request.".into()))?;
    let content_hash = STANDARD.encode(Sha256::digest(&body_bytes));
    let date = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string();
    let string_to_sign = format!("POST\n{path_query}\n{date};{host};{content_hash}");
    let key = STANDARD
        .decode(access_key)
        .map_err(|_| AppError::internal("Azure email access key is not valid base64.".into()))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key)
        .map_err(|_| AppError::internal("Azure email signing key is invalid.".into()))?;
    mac.update(string_to_sign.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());
    let authorization = format!(
        "HMAC-SHA256 SignedHeaders=x-ms-date;host;x-ms-content-sha256&Signature={signature}"
    );
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::internal("Could not initialize email delivery.".into()))?
        .post(url)
        .header("x-ms-date", &date)
        .header("x-ms-content-sha256", &content_hash)
        .header("host", host)
        .header("Authorization", authorization)
        .header("Content-Type", "application/json")
        .body(body_bytes)
        .send()
        .await
        .map_err(|_| AppError::internal("Azure email delivery failed.".into()))?;
    if !response.status().is_success() {
        tracing::error!(status = %response.status(), "Azure email OTP delivery rejected");
        return Err(AppError::internal("Azure email delivery failed.".into()));
    }
    Ok(())
}
