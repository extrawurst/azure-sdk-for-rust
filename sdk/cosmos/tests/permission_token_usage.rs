#![cfg(all(test, feature = "test_e2e"))]
use azure_cosmos::prelude::*;
use collection::*;

mod setup;

#[tokio::test]
async fn permission_token_usage() {
    const DATABASE_NAME: &str = "cosmos-test-db-permusage";
    const COLLECTION_NAME: &str = "cosmos-test-db-permusage";
    const USER_NAME: &str = "someone@cool.net";
    const PERMISSION: &str = "sdktest";

    let mut client = setup::initialize().unwrap();

    // create a temp database
    let _create_database_response = client
        .create_database()
        .with_database_name(&DATABASE_NAME)
        .execute()
        .await
        .unwrap();

    let database_client = client.clone().into_database_client(DATABASE_NAME);

    // create a new collection
    let indexing_policy = IndexingPolicy {
        automatic: true,
        indexing_mode: IndexingMode::Consistent,
        included_paths: vec![],
        excluded_paths: vec![],
    };

    let create_collection_response = database_client
        .create_collection()
        .with_collection_name(&COLLECTION_NAME)
        .with_offer(Offer::Throughput(400))
        .with_partition_key(&("/id".into()))
        .with_indexing_policy(&indexing_policy)
        .execute()
        .await
        .unwrap();

    let user_client = database_client.clone().into_user_client(USER_NAME);
    user_client.create_user().execute().await.unwrap();

    // create the RO permission
    let permission_client = user_client.into_permission_client(PERMISSION);
    let permission_mode = create_collection_response.collection.read_permission();

    let create_permission_response = permission_client
        .create_permission()
        .with_expiry_seconds(18000) // 5 hours, max!
        .execute_with_permission(&permission_mode)
        .await
        .unwrap();

    // change the AuthorizationToken using the token
    // of the permission.
    let new_authorization_token: AuthorizationToken = create_permission_response
        .permission
        .permission_token
        .into();
    client.with_auth_token(new_authorization_token);
    let new_database_client = client.clone().into_database_client(DATABASE_NAME);

    // let's list the collection content.
    // This must succeed.
    new_database_client
        .clone()
        .into_collection_client(COLLECTION_NAME)
        .list_documents()
        .execute::<serde_json::Value>()
        .await
        .unwrap();

    let new_collection_client = new_database_client.into_collection_client(COLLECTION_NAME);

    // Now we try to insert a document with the "read-only"
    // authorization_token just created. It must fail.
    let data = r#"
        {
            "id": "Gianluigi Bombatomica",
            "age": 43,
            "phones": [
                "+39 1234567",
                "+39 2345678"
            ]
        }"#;
    let document = Document::new(serde_json::from_str::<serde_json::Value>(data).unwrap());
    new_collection_client
        .create_document()
        .with_is_upsert(true)
        .with_partition_keys(PartitionKeys::new().push(&"Gianluigi Bombatomica").unwrap())
        .execute_with_document(&document)
        .await
        .unwrap_err();

    permission_client
        .delete_permission()
        .execute()
        .await
        .unwrap();

    // All includes read and write.
    let permission_mode = create_collection_response.collection.all_permission();
    let create_permission_response = permission_client
        .create_permission()
        .with_expiry_seconds(18000) // 5 hours, max!
        .execute_with_permission(&permission_mode)
        .await
        .unwrap();

    let new_authorization_token: AuthorizationToken = create_permission_response
        .permission
        .permission_token
        .into();
    client.with_auth_token(new_authorization_token);
    let new_database_client = client.into_database_client(DATABASE_NAME);
    let new_collection_client = new_database_client.into_collection_client(COLLECTION_NAME);

    // now we have an "All" authorization_token
    // so the create_document should succeed!
    let create_document_response = new_collection_client
        .create_document()
        .with_is_upsert(true)
        .with_partition_keys(PartitionKeys::new().push(&"Gianluigi Bombatomica").unwrap())
        .execute_with_document(&document)
        .await
        .unwrap();
    println!(
        "create_document_response == {:#?}",
        create_document_response
    );

    // cleanup
    database_client.delete_database().execute().await.unwrap();
}
