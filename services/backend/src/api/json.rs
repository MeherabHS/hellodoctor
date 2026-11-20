//! A drop-in replacement for `axum::Json` whose extraction failures become our
//! standard [`AppError`] envelope instead of axum's default plaintext
//! rejection.
//!
//! The default `axum::Json` rejection returns bodies like
//! `Failed to deserialize the JSON body into the target type: missing field
//! `email` at line 1 column 2` — which leaks the framework and the field
//! names and is inconsistent with the `{success,error,meta}` shape every other
//! error uses. This wrapper maps the rejection to [`AppError::Validation`]
//! (still HTTP 422) with a fixed, non-revealing message.
//!
//! Used on the pre-authentication auth endpoints, where a malformed body is
//! reachable without a token. It derefs to the inner value and is itself a
//! valid response type, so handlers keep using `body.field` and
//! `Json(ApiResponse { .. })` unchanged.

use crate::error::AppError;
use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::response::IntoResponse;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub struct Json<T>(pub T);

#[async_trait::async_trait]
impl<S, T> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Json(value)),
            // Deliberately generic: never echo serde's field names or offsets.
            Err(JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_)) => {
                Err(AppError::Validation(
                    "Request body is missing a required field or is not valid JSON.".into(),
                ))
            }
            Err(JsonRejection::MissingJsonContentType(_)) => Err(AppError::Validation(
                "Request body must be sent as application/json.".into(),
            )),
            Err(_) => Err(AppError::Validation("Request body is not valid.".into())),
        }
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

impl<T> std::ops::Deref for Json<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
