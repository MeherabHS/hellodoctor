use object_store::{path::Path, ObjectStore, ObjectStoreExt, PutMode, PutOptions};
use uuid::Uuid;

const AZURITE_ACCOUNT: &str = "devstoreaccount1";

fn azurite_store(container: &str) -> object_store::azure::MicrosoftAzureBuilder {
    object_store::azure::MicrosoftAzureBuilder::new()
        .with_account(AZURITE_ACCOUNT)
        .with_container_name(container)
        .with_use_emulator(true)
        .with_allow_http(true)
}

#[tokio::test]
#[ignore = "requires an explicitly owned Azurite fixture"]
async fn azurite_round_trip_isolated_and_create_only() {
    let suffix = Uuid::new_v4();
    let clinical = azurite_store("clinical-private").build().unwrap();
    let article = azurite_store("article-media").build().unwrap();
    let clinical_key = Path::from(format!("clinical/{suffix}.enc"));
    let article_key = Path::from(format!("article-media/{suffix}.jpg"));

    clinical
        .put_opts(
            &clinical_key,
            vec![1_u8, 2, 3].into(),
            PutOptions {
                mode: PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let duplicate = clinical
        .put_opts(
            &clinical_key,
            vec![9_u8].into(),
            PutOptions {
                mode: PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .expect_err("Azurite create-only writes must reject overwrites");
    assert!(matches!(
        duplicate,
        object_store::Error::AlreadyExists { .. }
    ));
    assert_eq!(
        clinical
            .get(&clinical_key)
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
        vec![1, 2, 3]
    );
    assert!(matches!(
        article.get(&clinical_key).await,
        Err(object_store::Error::NotFound { .. })
    ));

    article
        .put_opts(
            &article_key,
            vec![4_u8, 5, 6].into(),
            PutOptions {
                mode: PutMode::Create,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        article
            .get(&article_key)
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
        vec![4, 5, 6]
    );

    clinical.delete(&clinical_key).await.unwrap();
    article.delete(&article_key).await.unwrap();
    assert!(matches!(
        clinical.get(&clinical_key).await,
        Err(object_store::Error::NotFound { .. })
    ));
}
