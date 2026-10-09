use crate::{error::AppError, repository::AppState};
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::{IpAddr, SocketAddr};

/// Only explicitly configured physical peers may assert proxy metadata. The
/// terminating proxy must overwrite these headers, not append client input.
pub async fn transport_guard(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(ConnectInfo(peer)) = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .copied()
    else {
        return AppError::service_unavailable("Transport peer identity unavailable.".into())
            .into_response();
    };
    let trusted = state.config.trusted_proxy_ips.contains(&peer.ip());
    // Container/operator readiness over loopback is the sole plaintext deployed
    // exception. It cannot be used for credentials, metrics or API operations.
    if state.config.requires_https()
        && !(peer.ip().is_loopback() && request.uri().path() == "/healthz")
    {
        let proto = request.headers().get_all("x-forwarded-proto");
        if !trusted
            || proto.iter().count() != 1
            || proto.iter().next().and_then(|v| v.to_str().ok()) != Some("https")
        {
            return AppError::Forbidden("HTTPS through an approved TLS proxy is required.".into())
                .into_response();
        }
    }
    if trusted {
        let forwarded = request.headers().get_all("x-forwarded-for");
        let client = if forwarded.iter().count() == 1 {
            forwarded
                .iter()
                .next()
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<IpAddr>().ok())
        } else {
            None
        };
        if let Some(ip) = client {
            request
                .extensions_mut()
                .insert(ConnectInfo(SocketAddr::new(ip, peer.port())));
        } else if state.config.requires_https() && request.uri().path() != "/healthz" {
            return AppError::Forbidden(
                "Approved proxy client-address metadata is required.".into(),
            )
            .into_response();
        }
    }
    // Forwarded/X-Forwarded-* from all other peers are ignored completely.
    next.run(request).await
}
