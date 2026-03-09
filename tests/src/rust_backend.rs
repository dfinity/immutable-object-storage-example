//! PocketIC canister tests for the Rust example backend.

use std::path::PathBuf;

use candid::Principal;
use pocket_ic::PocketIc;

use crate::{
    add_gateway_principal_with_sender, blob_is_live, blobs_to_delete_with_sender,
    confirm_blob_deletion, create_certificate, delete_blob, deploy_canister,
    deploy_canister_with_controller, hash_string_to_32_bytes, list_blobs, load_wasm, set_blob_info,
};

fn rust_wasm_path() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dfx_path = manifest.join("../rust-backend/.dfx/local/canisters/example_backend/example_backend.wasm");
    let cargo_path = manifest.join("../rust-backend/target/wasm32-unknown-unknown/debug/example_backend.wasm");
    if dfx_path.exists() {
        dfx_path
    } else {
        cargo_path
    }
}

#[test]
fn test_create_certificate_valid() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"a".repeat(64);
    let result = create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    assert_eq!(result.method, "upload");
    assert_eq!(result.blob_hash, hash);
}

#[test]
fn test_create_certificate_invalid_hash() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let err = create_certificate(&pic, canister_id, "not-a-hash").unwrap_err();
    assert!(err.to_string().contains("hash must be"));
}

#[test]
fn test_create_certificate_empty_hash() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let err = create_certificate(&pic, canister_id, "").unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_blob_is_live() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
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
fn test_gateway_auth() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let gateway = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let canister_id = deploy_canister_with_controller(&pic, wasm, gateway);

    let list = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert!(list.is_empty());

    add_gateway_principal_with_sender(&pic, canister_id, gateway, gateway, "add_gateway_principal")
        .expect("add_gateway_principal");

    let hash = "sha256:".to_string() + &"c".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    delete_blob(&pic, canister_id, &hash).expect("delete_blob");

    let list_after = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert_eq!(list_after, vec![hash]);
}

#[test]
fn test_add_gateway_principal_not_controller() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let gateway_principal = Principal::from_text("aaaaa-aa").unwrap();
    let non_controller = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let err = pic
        .update_call(
            canister_id,
            non_controller,
            "add_gateway_principal",
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
    let wasm = load_wasm(&rust_wasm_path());
    let gateway = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let canister_id = deploy_canister_with_controller(&pic, wasm, gateway);

    add_gateway_principal_with_sender(&pic, canister_id, gateway, gateway, "add_gateway_principal")
        .expect("add_gateway_principal");

    let hash = "sha256:".to_string() + &"d".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    let blobs_before = list_blobs(&pic, canister_id).expect("list_blobs");
    assert_eq!(blobs_before.len(), 1);

    delete_blob(&pic, canister_id, &hash).expect("delete_blob");
    let to_delete = blobs_to_delete_with_sender(&pic, canister_id, gateway).expect("blobs_to_delete");
    assert_eq!(to_delete, vec![hash.clone()]);

    let hash_bytes = hash_string_to_32_bytes(&hash);
    confirm_blob_deletion(&pic, canister_id, gateway, vec![hash_bytes]).expect("confirm_blob_deletion");

    let blobs_after = list_blobs(&pic, canister_id).expect("list_blobs");
    assert!(blobs_after.is_empty());
}

#[test]
fn test_list_blobs_and_set_blob_info() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"e".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    set_blob_info(
        &pic,
        canister_id,
        &hash,
        "myfile.txt",
        1234,
        "text/plain",
    )
    .expect("set_blob_info");

    let blobs = list_blobs(&pic, canister_id).expect("list_blobs");
    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0].hash, hash);
    assert_eq!(blobs[0].name, "myfile.txt");
    assert_eq!(blobs[0].size, 1234);
    assert_eq!(blobs[0].content_type, "text/plain");
}

#[test]
fn test_blob_is_live_after_delete() {
    let pic = PocketIc::new();
    let wasm = load_wasm(&rust_wasm_path());
    let canister_id = deploy_canister(&pic, wasm);

    let hash = "sha256:".to_string() + &"f".repeat(64);
    create_certificate(&pic, canister_id, &hash).expect("create_certificate");
    let hash_bytes = hash_string_to_32_bytes(&hash);
    assert!(blob_is_live(&pic, canister_id, hash_bytes.clone()).expect("query"));

    delete_blob(&pic, canister_id, &hash).expect("delete_blob");
    let live_after = blob_is_live(&pic, canister_id, hash_bytes).expect("query");
    assert!(!live_after);
}
