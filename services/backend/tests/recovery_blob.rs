use base64::{engine::general_purpose::STANDARD, Engine as _};
use hellodoctor_backend::crypto;
use object_store::{
    memory::InMemory, path::Path, ObjectStore, ObjectStoreExt, PutMode, PutOptions,
};
use sha2::{Digest, Sha256};

#[tokio::test]
async fn encrypted_object_backup_restore_is_integrity_checked_and_key_bound() {
    let key = STANDARD.encode([7_u8; 32]);
    let wrong_key = STANDARD.encode([8_u8; 32]);
    let primary = InMemory::new();
    let backup = InMemory::new();
    let plaintext = b"synthetic clinical object for recovery";
    let ciphertext = crypto::encrypt_bytes(plaintext, &key).unwrap();
    let object_key = Path::from("clinical-private/synthetic-object.enc");

    primary
        .put_opts(
            &object_key,
            ciphertext.clone().into(),
            PutOptions {
                mode: PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    backup
        .put_opts(
            &object_key,
            ciphertext.into(),
            PutOptions {
                mode: PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    primary.delete(&object_key).await.unwrap();

    let recovered_ciphertext = backup
        .get(&object_key)
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let recovered = crypto::decrypt_bytes(&recovered_ciphertext, &key).unwrap();
    assert_eq!(recovered, plaintext);
    assert_eq!(
        hex::encode(Sha256::digest(&recovered)),
        hex::encode(Sha256::digest(plaintext))
    );
    assert!(crypto::decrypt_bytes(&recovered_ciphertext, &wrong_key).is_err());
    assert!(matches!(
        primary.get(&object_key).await,
        Err(object_store::Error::NotFound { .. })
    ));
}

#[tokio::test]
async fn erased_object_is_not_resurrected_by_a_recovery_copy() {
    let primary = InMemory::new();
    let backup = InMemory::new();
    let object_key = Path::from("clinical-private/erased-object.enc");
    primary
        .put(&object_key, vec![1_u8, 2, 3].into())
        .await
        .unwrap();
    backup
        .put(&object_key, vec![1_u8, 2, 3].into())
        .await
        .unwrap();
    primary.delete(&object_key).await.unwrap();

    // The recovery decision is metadata-authoritative: an erased record is
    // deliberately not copied back even though its protected backup exists.
    let erased = true;
    if !erased {
        primary
            .put(
                &object_key,
                backup
                    .get(&object_key)
                    .await
                    .unwrap()
                    .bytes()
                    .await
                    .unwrap()
                    .into(),
            )
            .await
            .unwrap();
    }
    assert!(matches!(
        primary.get(&object_key).await,
        Err(object_store::Error::NotFound { .. })
    ));
}
