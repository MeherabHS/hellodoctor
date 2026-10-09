use crate::{crypto, error::AppError, repository::AppState};
use object_store::path::Path;
use object_store::ObjectStoreExt;
use sha2::{Digest, Sha256};

/// Private S3 objects contain authenticated ciphertext, never clinical plaintext.
/// Database metadata remains the authorization authority on every download.
pub struct DocumentStorageService;

impl DocumentStorageService {
    pub async fn put(state: &AppState, key: &str, sanitized: &[u8]) -> Result<(), AppError> {
        let storage = state.object_storage.as_ref().ok_or_else(|| {
            AppError::Conflict("Document storage is not configured. Upload is unavailable.".into())
        })?;
        let encrypted = crypto::encrypt_bytes(sanitized, &state.config.storage_encryption_key_b64)?;
        storage
            .put(&Path::from(key), encrypted.into())
            .await
            .map_err(|_| {
                AppError::internal(
                    "Private document storage is unavailable; the upload was not confirmed.".into(),
                )
            })?;
        Ok(())
    }

    pub async fn get(
        state: &AppState,
        key: &str,
        expected_hash: &str,
    ) -> Result<Vec<u8>, AppError> {
        let storage = state.object_storage.as_ref().ok_or_else(|| {
            AppError::Conflict(
                "Document storage is not configured. Download is unavailable.".into(),
            )
        })?;
        let object = storage
            .get(&Path::from(key))
            .await
            .map_err(|error| match error {
                object_store::Error::NotFound { .. } => {
                    AppError::NotFound("Document file is unavailable.".into())
                }
                _ => AppError::internal("Private document storage is unavailable.".into()),
            })?;
        if object.meta.size > (10 * 1024 * 1024 + 28) {
            return Err(AppError::internal(
                "Document size validation failed.".into(),
            ));
        }
        let encrypted = object
            .bytes()
            .await
            .map_err(|_| AppError::internal("Document download failed.".into()))?;
        let plaintext =
            crypto::decrypt_bytes(&encrypted, &state.config.storage_encryption_key_b64)?;
        let actual_hash = hex::encode(Sha256::digest(&plaintext));
        if actual_hash != expected_hash {
            return Err(AppError::internal(
                "Document integrity validation failed.".into(),
            ));
        }
        Ok(plaintext)
    }

    pub async fn delete(state: &AppState, key: &str) {
        let Some(storage) = state.object_storage.as_ref() else {
            return;
        };
        // Best-effort rollback cleanup. An orphaned private ciphertext object
        // grants no access because no authorized database document refers to it.
        if storage.delete(&Path::from(key)).await.is_err() {
            tracing::warn!("Private document rollback cleanup needs storage reconciliation");
        }
    }
}
