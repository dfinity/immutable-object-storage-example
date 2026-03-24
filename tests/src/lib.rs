//! Shared helpers and Candid types for example-backend canister tests.

#[cfg(test)]
mod motoko_backend;
#[cfg(test)]
mod rust_backend;

use std::path::Path;

use candid::{types::number::Nat, CandidType, Decode, Principal};
use pocket_ic::PocketIc;
use serde::Deserialize;
use serde::Serialize;

// =============================================================================
// Candid types (mirror example_backend.did)
// =============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, CandidType)]
pub struct BlobInfo {
    pub hash: String,
    pub name: String,
    pub size: u64,
    pub content_type: String,
    pub created_at: u64,
}

/// Motoko backend returns BlobInfo with camelCase field names.
/// Motoko uses Int for createdAt (Time.now()) and Nat for size; decode as i128 and Nat.
#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize, CandidType)]
pub struct BlobInfoMotoko {
    pub hash: String,
    pub name: String,
    /// Motoko Nat; use candid::Nat to accept Candid nat.
    pub size: Nat,
    pub contentType: String,
    /// Motoko Int (Time.now()); decode as i128 to accept Candid int.
    pub createdAt: i128,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType)]
pub struct CreateCertificateResult {
    pub method: String,
    pub blob_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType)]
pub struct InitArgs {
    pub gateway_principals: Option<Vec<Principal>>,
}

// =============================================================================
// Deploy
// =============================================================================

/// Load wasm bytes from a path. Panics with a clear message if the file is missing.
pub fn load_wasm(wasm_path: &Path) -> Vec<u8> {
    std::fs::read(wasm_path).unwrap_or_else(|e| {
        panic!(
            "WASM not found at {}. Run `dfx build example_backend` (or for Rust: `cargo build --target wasm32-unknown-unknown`) in the backend directory first. Error: {}",
            wasm_path.display(),
            e
        )
    })
}

/// Create canister, add cycles, install wasm with empty init args. Returns canister id.
/// Controller is set to `controller` so that principal can call add_gateway_principal.
pub fn deploy_canister_with_controller(
    pic: &PocketIc,
    wasm_bytes: Vec<u8>,
    controller: Principal,
) -> Principal {
    deploy_canister_with_init_args(pic, wasm_bytes, controller, None)
}

/// Deploy with init args (including optional gateway principals).
pub fn deploy_canister_with_init_args(
    pic: &PocketIc,
    wasm_bytes: Vec<u8>,
    controller: Principal,
    init_args: Option<InitArgs>,
) -> Principal {
    let canister_id = pic.create_canister_with_settings(Some(controller), None);
    pic.add_cycles(canister_id, 10_000_000_000_000u128); // 10T
    let encoded = candid::encode_one(init_args).expect("encode init");
    pic.install_canister(canister_id, wasm_bytes, encoded, Some(controller));
    canister_id
}

/// Deploy with anonymous as controller (convenience for tests that use anonymous).
pub fn deploy_canister(pic: &PocketIc, wasm_bytes: Vec<u8>) -> Principal {
    deploy_canister_with_controller(pic, wasm_bytes, Principal::anonymous())
}

// =============================================================================
// Typed call wrappers
// =============================================================================

const SENDER: Principal = Principal::anonymous();

pub fn create_certificate(
    pic: &PocketIc,
    canister_id: Principal,
    hash: &str,
) -> Result<CreateCertificateResult, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(hash).expect("encode");
    let bytes = pic.update_call(canister_id, SENDER, "_immutableObjectStorageCreateCertificate", payload)?;
    Ok(Decode!(&bytes, CreateCertificateResult).expect("decode"))
}

pub fn blobs_are_live(
    pic: &PocketIc,
    canister_id: Principal,
    hash_bytes_list: Vec<Vec<u8>>,
) -> Result<Vec<bool>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(hash_bytes_list).expect("encode");
    let bytes = pic.query_call(
        canister_id,
        SENDER,
        "_immutableObjectStorageBlobsAreLive",
        payload,
    )?;
    Ok(Decode!(&bytes, Vec<bool>).expect("decode"))
}

/// Convenience wrapper for checking a single blob.
pub fn blob_is_live(
    pic: &PocketIc,
    canister_id: Principal,
    hash_bytes: Vec<u8>,
) -> Result<bool, pocket_ic::RejectResponse> {
    let results = blobs_are_live(pic, canister_id, vec![hash_bytes])?;
    Ok(results.into_iter().next().unwrap_or(false))
}

pub fn blobs_to_delete(
    pic: &PocketIc,
    canister_id: Principal,
) -> Result<Vec<String>, pocket_ic::RejectResponse> {
    blobs_to_delete_with_sender(pic, canister_id, SENDER)
}

/// Call _immutableObjectStorageBlobsToDelete as a specific sender (e.g. the gateway principal).
pub fn blobs_to_delete_with_sender(
    pic: &PocketIc,
    canister_id: Principal,
    sender: Principal,
) -> Result<Vec<String>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(()).expect("encode");
    let bytes = pic.query_call(
        canister_id,
        sender,
        "_immutableObjectStorageBlobsToDelete",
        payload,
    )?;
    Ok(Decode!(&bytes, Vec<String>).expect("decode"))
}

pub fn confirm_blob_deletion(
    pic: &PocketIc,
    canister_id: Principal,
    sender: Principal,
    hash_bytes_list: Vec<Vec<u8>>,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(hash_bytes_list).expect("encode");
    pic.update_call(
        canister_id,
        sender,
        "_immutableObjectStorageConfirmBlobDeletion",
        payload,
    )
}

pub fn add_gateway_principal(
    pic: &PocketIc,
    canister_id: Principal,
    principal: Principal,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    add_gateway_principal_with_sender(pic, canister_id, principal, SENDER, "add_gateway_principal")
}

/// Call add_gateway_principal with a given method name (e.g. "addGatewayPrincipal" for Motoko).
pub fn add_gateway_principal_raw(
    pic: &PocketIc,
    canister_id: Principal,
    principal: Principal,
    method: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    add_gateway_principal_with_sender(pic, canister_id, principal, SENDER, method)
}

/// Call add_gateway_principal as a specific sender (must be canister controller).
pub fn add_gateway_principal_with_sender(
    pic: &PocketIc,
    canister_id: Principal,
    principal: Principal,
    sender: Principal,
    method: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(principal).expect("encode");
    pic.update_call(canister_id, sender, method, payload)
}

pub fn list_blobs(pic: &PocketIc, canister_id: Principal) -> Result<Vec<BlobInfo>, pocket_ic::RejectResponse> {
    list_blobs_raw(pic, canister_id, "list_blobs")
}

/// Call list_blobs with a given method name (e.g. "listBlobs" for Motoko).
pub fn list_blobs_raw(
    pic: &PocketIc,
    canister_id: Principal,
    method: &str,
) -> Result<Vec<BlobInfo>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(()).expect("encode");
    let bytes = pic.query_call(canister_id, SENDER, method, payload)?;
    Ok(Decode!(&bytes, Vec<BlobInfo>).expect("decode"))
}

/// Call listBlobs and decode as Motoko's BlobInfo (camelCase fields).
pub fn list_blobs_motoko(
    pic: &PocketIc,
    canister_id: Principal,
) -> Result<Vec<BlobInfoMotoko>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(()).expect("encode");
    let bytes = pic.query_call(canister_id, SENDER, "listBlobs", payload)?;
    Ok(Decode!(&bytes, Vec<BlobInfoMotoko>).expect("decode"))
}

pub fn set_blob_info(
    pic: &PocketIc,
    canister_id: Principal,
    hash: &str,
    name: &str,
    size: u64,
    content_type: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    set_blob_info_raw(pic, canister_id, hash, name, size, content_type, "set_blob_info")
}

/// Call set_blob_info with a given method name (e.g. "setBlobInfo" for Motoko).
pub fn set_blob_info_raw(
    pic: &PocketIc,
    canister_id: Principal,
    hash: &str,
    name: &str,
    size: u64,
    content_type: &str,
    method: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    let payload = candid::encode_args((hash, name, size, content_type)).expect("encode");
    pic.update_call(canister_id, SENDER, method, payload)
}

/// Call setBlobInfo on Motoko backend (expects Nat for size, not nat64).
pub fn set_blob_info_motoko(
    pic: &PocketIc,
    canister_id: Principal,
    hash: &str,
    name: &str,
    size: u64,
    content_type: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    let payload =
        candid::encode_args((hash, name, Nat::from(size), content_type)).expect("encode");
    pic.update_call(canister_id, SENDER, "setBlobInfo", payload)
}

pub fn delete_blob(
    pic: &PocketIc,
    canister_id: Principal,
    hash: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    delete_blob_raw(pic, canister_id, hash, "delete_blob")
}

/// Call delete_blob with a given method name (e.g. "deleteBlob" for Motoko).
pub fn delete_blob_raw(
    pic: &PocketIc,
    canister_id: Principal,
    hash: &str,
    method: &str,
) -> Result<Vec<u8>, pocket_ic::RejectResponse> {
    let payload = candid::encode_one(hash).expect("encode");
    pic.update_call(canister_id, SENDER, method, payload)
}

// =============================================================================
// Helpers
// =============================================================================

/// Parse "sha256:<64 hex chars>" into 32 bytes. Panics if invalid.
pub fn hash_string_to_32_bytes(hash: &str) -> Vec<u8> {
    let hex_part = hash
        .strip_prefix("sha256:")
        .expect("hash must start with sha256:");
    assert_eq!(hex_part.len(), 64, "hash must be 64 hex chars");
    hex::decode(hex_part).expect("hash must be valid hex")
}
