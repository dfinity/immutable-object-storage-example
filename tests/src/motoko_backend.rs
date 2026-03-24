//! PocketIC canister tests for the Motoko example backend.

use std::path::PathBuf;

use candid::Principal;
use pocket_ic::PocketIc;

use crate::{
    add_gateway_principal_with_sender, blob_is_live, blobs_are_live, blobs_to_delete_with_sender,
    confirm_blob_deletion, create_certificate, delete_blob_raw, deploy_canister,
    deploy_canister_with_controller, deploy_canister_with_init_args, hash_string_to_32_bytes,
    list_blobs_motoko, load_wasm, set_blob_info_motoko, InitArgs,
};

fn motoko_wasm_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../motoko-backend/.dfx/local/canisters/example_backend/example_backend.wasm")
}

#[test]
fn test_create_certificate_valid() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"a".repeat(64);
    let result = create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    assert_eq!(result.method, "upload");
    assert_eq!(result.blob_hash, hash);
}

#[test]
fn test_create_certificate_invalid_hash() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let err = create_certificate(&pic, canister_id, "not-a-hash").unwrap_err();
    assert!(err.to_string().contains("hash must be"));
}

#[test]
fn test_create_certificate_empty_hash() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let err = create_certificate(&pic, canister_id, "").unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_blob_is_live() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"b".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    let hash_bytes = hash_string_to_32_bytes(&hash);
    let live = blob_is_live(&pic, canister_id, hash_bytes).expect("blob_is_live");
    assert!(live);

    let unknown_hash_bytes = vec![0u8; 32];
    let live_unknown = blob_is_live(&pic, canister_id, unknown_hash_bytes).expect("query");
    assert!(!live_unknown);
}

#[test]
fn test_blobs_are_live_batch() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash_a = "sha256:".to_string() + &"a".repeat(64);
    let hash_b = "sha256:".to_string() + &"b".repeat(64);
    create_certificate(&pic, canister_id, &hash_a).expect("create_certificate");

    let results = blobs_are_live(
        &pic,
        canister_id,
        vec![
            hash_string_to_32_bytes(&hash_a),
            hash_string_to_32_bytes(&hash_b),
            vec![0u8; 32],
        ],
    )
    .expect("blobs_are_live");

    assert_eq!(results, vec![true, false, false]);
}

#[test]
fn test_gateway_auth() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let gateway = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let canister_id = deploy_canister_with_controller(&pic, wasm, gateway);

    let list = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert!(list.is_empty());

    add_gateway_principal_with_sender(&pic, canister_id, gateway, gateway, "addGatewayPrincipal")
        .expect("addGatewayPrincipal");

    let hash = "sha256:".to_string() + &"c".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    delete_blob_raw(&pic, canister_id, &hash, "deleteBlob").expect("deleteBlob");

    let list_after = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert_eq!(list_after, vec![hash]);
}

#[test]
fn test_add_gateway_principal_not_controller() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let gateway_principal = Principal::from_text("aaaaa-aa").unwrap();
    let non_controller = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let err = pic
        .update_call(
            canister_id,
            non_controller,
            "addGatewayPrincipal",
            candid::encode_one(gateway_principal).unwrap(),
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("only a canister controller")
            || err.to_string().contains("reject")
            || err.to_string().contains("error")
    );
}

#[test]
fn test_deletion_flow() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let gateway = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let canister_id = deploy_canister_with_controller(&pic, wasm, gateway);

    add_gateway_principal_with_sender(&pic, canister_id, gateway, gateway, "addGatewayPrincipal")
        .expect("addGatewayPrincipal");

    let hash = "sha256:".to_string() + &"d".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    let blobs_before = list_blobs_motoko(&pic, canister_id).expect("listBlobs");
    assert_eq!(blobs_before.len(), 1);

    delete_blob_raw(&pic, canister_id, &hash, "deleteBlob").expect("deleteBlob");
    let to_delete = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert_eq!(to_delete, vec![hash.clone()]);

    let hash_bytes = hash_string_to_32_bytes(&hash);
    confirm_blob_deletion(&pic, canister_id, gateway, vec![hash_bytes]).expect("confirm_blob_deletion");

    let blobs_after = list_blobs_motoko(&pic, canister_id).expect("listBlobs");
    assert!(blobs_after.is_empty());
}

#[test]
fn test_list_blobs_and_set_blob_info() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"e".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    set_blob_info_motoko(&pic, canister_id, &hash, "myfile.txt", 1234, "text/plain")
        .expect("setBlobInfo");

    let blobs = list_blobs_motoko(&pic, canister_id).expect("listBlobs");
    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0].hash, hash);
    assert_eq!(blobs[0].name, "myfile.txt");
    assert_eq!(blobs[0].size, candid::Nat::from(1234u64));
    assert_eq!(blobs[0].contentType, "text/plain");
    assert!(blobs[0].createdAt != 0_i128); // set by canister (Time.now())
}

#[test]
fn test_init_with_gateway_principals() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let gateway = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let canister_id = deploy_canister_with_init_args(
        &pic,
        wasm,
        gateway,
        Some(InitArgs {
            gateway_principals: Some(vec![gateway]),
        }),
    );

    let hash = "sha256:".to_string() + &"1".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    delete_blob_raw(&pic, canister_id, &hash, "deleteBlob").expect("deleteBlob");

    let list = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert_eq!(list, vec![hash]);
}

#[test]
fn test_blob_is_live_after_delete() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&motoko_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"f".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    let hash_bytes = hash_string_to_32_bytes(&hash);
    assert!(blob_is_live(&pic, canister_id, hash_bytes.clone()).expect("query"));

    delete_blob_raw(&pic, canister_id, &hash, "deleteBlob").expect("deleteBlob");
    let live_after = blob_is_live(&pic, canister_id, hash_bytes).expect("query");
    assert!(!live_after);
}
