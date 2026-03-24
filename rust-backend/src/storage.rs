//! Immutable object storage protocol implementation (example canister).
//!
//! ## Integration checklist
//!
//! Copy this file into your canister, then:
//!
//! 1. Make `crate::MEMORY_MANAGER` accessible (one per canister).
//! 2. Choose non-conflicting `MemoryId`s for `BLOBS`, `PENDING_DELETE`, and
//!    `GATEWAY_PRINCIPALS`.
//! 3. Replace `caller_is_gateway()` with your own authorization check.
//!
//! ## Expected flow
//!
//! 1. **Fetch gateway principals** — call
//!    `_immutableObjectStorageUpdateGatewayPrincipals` after deployment. This
//!    queries the Cashier canister for the current list of authorized gateways.
//!    The list of gateways is dynamic and may change over time as new gateways
//!    are added or removed by Caffeine.
//! 2. **Upload** — your app calls `_immutableObjectStorageCreateCertificate`
//!    with a blob hash. The canister records the hash as live and returns a
//!    certificate the client forwards to the gateway.
//! 3. **Deletion** — mark blobs via your app API (e.g. `delete_blob`). The
//!    gateway periodically calls `_immutableObjectStorageBlobsToDelete`, deletes
//!    the objects from storage, then confirms via
//!    `_immutableObjectStorageConfirmBlobDeletion`.
//!
//! ## Production considerations
//!
//! - `_immutableObjectStorageCreateCertificate` has **no caller auth** in this
//!   example. In production, restrict who may issue upload certificates.
//! - `_immutableObjectStorageBlobsToDelete` is a `#[query]` for performance.
//!   Query responses are not certified by IC consensus; consider `#[update]` if
//!   a compromised replica could trick the gateway into confirming spurious
//!   deletions.

use std::borrow::Cow;
use std::cell::RefCell;

use candid::CandidType;
use candid::Decode;
use candid::Encode;
use candid::Principal;
use ic_cdk::api::msg_caller;
use ic_cdk::api::time;
use ic_cdk::call::Call;
use ic_cdk::query;
use ic_cdk::update;
use ic_stable_structures::memory_manager::MemoryId;
use ic_stable_structures::storable::Bound;
use ic_stable_structures::StableBTreeMap;
use ic_stable_structures::Storable;
use serde::Deserialize;
use serde::Serialize;

use crate::Memory;

// =============================================================================
// Stable memory — allocate IDs from your MemoryManager
// =============================================================================

const BLOBS_MEMORY_ID: MemoryId = MemoryId::new(0);
const PENDING_DELETE_MEMORY_ID: MemoryId = MemoryId::new(1);
const GATEWAY_PRINCIPALS_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    static BLOBS: RefCell<StableBTreeMap<HashKey, BlobInfo, Memory>> = RefCell::new(
        StableBTreeMap::init(crate::MEMORY_MANAGER.with(|m| m.borrow().get(BLOBS_MEMORY_ID)))
    );

    static PENDING_DELETE: RefCell<StableBTreeMap<HashKey, Empty, Memory>> = RefCell::new(
        StableBTreeMap::init(crate::MEMORY_MANAGER.with(|m| m.borrow().get(PENDING_DELETE_MEMORY_ID)))
    );

    static GATEWAY_PRINCIPALS: RefCell<StableBTreeMap<Principal, Empty, Memory>> = RefCell::new(
        StableBTreeMap::init(crate::MEMORY_MANAGER.with(|m| m.borrow().get(GATEWAY_PRINCIPALS_MEMORY_ID)))
    );

    static CASHIER_CANISTER_ID: RefCell<Option<Principal>> = const { RefCell::new(None) };
}

pub(crate) fn set_cashier_canister_id(id: Principal) {
    CASHIER_CANISTER_ID.with(|c| *c.borrow_mut() = Some(id));
}

// =============================================================================
// Storable helpers
// =============================================================================

/// Newtype for blob hash strings stored as StableBTreeMap keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HashKey(String);

impl Storable for HashKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(self.0.as_bytes().to_vec())
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(String::from_utf8_lossy(&bytes).into_owned())
    }
    const BOUND: Bound = Bound::Unbounded;
}

/// Zero-size value for set-like StableBTreeMaps.
#[derive(Clone, Copy)]
struct Empty;

impl Storable for Empty {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&[])
    }
    fn into_bytes(self) -> Vec<u8> {
        vec![]
    }
    fn from_bytes(_: Cow<[u8]>) -> Self {
        Self
    }
    const BOUND: Bound = Bound::Bounded {
        max_size: 0,
        is_fixed_size: true,
    };
}

// =============================================================================
// Candid types
// =============================================================================

#[derive(CandidType, Clone, Deserialize, Serialize)]
pub struct BlobInfo {
    pub hash: String,
    pub name: String,
    pub size: u64,
    pub content_type: String,
    pub created_at: u64,
}

impl Storable for BlobInfo {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(Encode!(self).expect("BlobInfo encode"))
    }
    fn into_bytes(self) -> Vec<u8> {
        Encode!(&self).expect("BlobInfo encode")
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, BlobInfo).expect("BlobInfo decode")
    }
    const BOUND: Bound = Bound::Unbounded;
}

#[derive(CandidType, Deserialize)]
pub struct CreateCertificateResult {
    pub method: String,
    pub blob_hash: String,
}

// =============================================================================
// Helpers
// =============================================================================

fn bytes_to_sha256_string(bytes: &[u8]) -> Option<String> {
    if bytes.len() == 32 {
        Some(format!("sha256:{}", hex::encode(bytes)))
    } else {
        None
    }
}

pub(crate) fn caller_is_gateway() -> bool {
    let caller = msg_caller();
    if caller == Principal::anonymous() {
        return false;
    }
    GATEWAY_PRINCIPALS.with(|g| g.borrow().contains_key(&caller))
}

// =============================================================================
// Protocol endpoints: _immutableObjectStorage*
// =============================================================================

/// Fetches the current list of gateway principals from the Cashier canister
/// and replaces the local authorized set. Call after deployment and
/// periodically to pick up gateway changes.
#[update(name = "_immutableObjectStorageUpdateGatewayPrincipals")]
async fn update_gateway_principals() {
    let cashier_id = CASHIER_CANISTER_ID
        .with(|c| *c.borrow())
        .unwrap_or_else(|| ic_cdk::trap("cashier canister ID not configured"));

    let response = Call::bounded_wait(cashier_id, "storage_gateway_principal_list_v1")
        .await
        .unwrap_or_else(|e| {
            ic_cdk::trap(&format!("failed to query cashier: {e:?}"))
        });
    let principals: Vec<Principal> = response.candid().unwrap_or_else(|e| {
        ic_cdk::trap(&format!("failed to decode cashier response: {e:?}"))
    });

    GATEWAY_PRINCIPALS.with(|g| {
        let mut map = g.borrow_mut();
        let existing: Vec<Principal> = map.iter().map(|e| e.key().clone()).collect();
        for k in existing {
            map.remove(&k);
        }
        for p in principals {
            map.insert(p, Empty);
        }
    });
}

/// Returns whether each blob (identified by a 32-byte hash) is still live.
/// Input and output vecs have the same length and matching indices.
#[query(name = "_immutableObjectStorageBlobsAreLive")]
fn blobs_are_live(hash_bytes_list: Vec<Vec<u8>>) -> Vec<bool> {
    hash_bytes_list
        .iter()
        .map(|hash_bytes| {
            let Some(hash) = bytes_to_sha256_string(hash_bytes) else {
                return false;
            };
            let key = HashKey(hash);
            BLOBS.with(|b| b.borrow().contains_key(&key))
                && !PENDING_DELETE.with(|p| p.borrow().contains_key(&key))
        })
        .collect()
}

/// Returns hashes this canister has marked for deletion (gateway-only).
#[query(name = "_immutableObjectStorageBlobsToDelete")]
fn blobs_to_delete() -> Vec<String> {
    if !caller_is_gateway() {
        return vec![];
    }
    PENDING_DELETE.with(|p| p.borrow().iter().map(|e| e.key().0.clone()).collect())
}

/// Confirms blobs have been deleted from object storage.
#[update(name = "_immutableObjectStorageConfirmBlobDeletion")]
fn confirm_blob_deletion(hash_bytes_list: Vec<Vec<u8>>) {
    if !caller_is_gateway() {
        return;
    }
    for hash_bytes in &hash_bytes_list {
        if let Some(hash) = bytes_to_sha256_string(hash_bytes) {
            let key = HashKey(hash);
            PENDING_DELETE.with(|p| p.borrow_mut().remove(&key));
            BLOBS.with(|b| b.borrow_mut().remove(&key));
        }
    }
}

/// Creates an upload certificate. Registers the hash as a live blob.
#[update(name = "_immutableObjectStorageCreateCertificate")]
fn create_certificate(hash: String) -> CreateCertificateResult {
    if hash.is_empty() {
        ic_cdk::trap("hash must not be empty");
    }
    if !hash.starts_with("sha256:") || hash.len() != 71 {
        ic_cdk::trap("hash must be 'sha256:<64-hex-chars>'");
    }
    let key = HashKey(hash.clone());
    PENDING_DELETE.with(|p| p.borrow_mut().remove(&key));
    BLOBS.with(|b| {
        let mut map = b.borrow_mut();
        if !map.contains_key(&key) {
            map.insert(
                key,
                BlobInfo {
                    hash: hash.clone(),
                    name: String::new(),
                    size: 0,
                    content_type: String::new(),
                    created_at: time(),
                },
            );
        }
    });
    CreateCertificateResult {
        method: "upload".into(),
        blob_hash: hash,
    }
}

// =============================================================================
// Queries used by the app API in lib.rs
// =============================================================================

pub(crate) fn get_blob(hash: &str) -> Option<BlobInfo> {
    BLOBS.with(|b| b.borrow().get(&HashKey(hash.to_owned())))
}

pub(crate) fn update_blob(hash: &str, info: BlobInfo) {
    BLOBS.with(|b| b.borrow_mut().insert(HashKey(hash.to_owned()), info));
}

pub(crate) fn list_live_blobs() -> Vec<BlobInfo> {
    BLOBS.with(|b| {
        PENDING_DELETE.with(|p| {
            let pending = p.borrow();
            b.borrow()
                .iter()
                .filter(|e| !pending.contains_key(e.key()))
                .map(|e| e.value().clone())
                .collect()
        })
    })
}

pub(crate) fn mark_for_deletion(hash: &str) {
    let key = HashKey(hash.to_owned());
    if !BLOBS.with(|b| b.borrow().contains_key(&key)) {
        ic_cdk::trap("blob not found");
    }
    PENDING_DELETE.with(|p| p.borrow_mut().insert(key, Empty));
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use ic_stable_structures::Storable;

    use super::{BlobInfo, HashKey};

    #[test]
    fn bytes_to_sha256_string_valid_32_bytes() {
        let bytes: [u8; 32] = [0xba; 32];
        let s = super::bytes_to_sha256_string(&bytes).expect("32 bytes should produce Some");
        assert_eq!(s, "sha256:babababababababababababababababababababababababababababababababa");
        assert!(s.starts_with("sha256:"));
        assert_eq!(s.len(), 71);
    }

    #[test]
    fn bytes_to_sha256_string_wrong_length_returns_none() {
        assert!(super::bytes_to_sha256_string(&[]).is_none());
        assert!(super::bytes_to_sha256_string(&[0u8; 31]).is_none());
        assert!(super::bytes_to_sha256_string(&[0u8; 33]).is_none());
    }

    #[test]
    fn hash_key_storable_roundtrip() {
        let key = HashKey::from_bytes(Cow::Borrowed(b"sha256:abc123"));
        let bytes = key.to_bytes();
        let key2 = HashKey::from_bytes(Cow::Owned(bytes.into_owned()));
        assert_eq!(key, key2);
    }

    #[test]
    fn blob_info_storable_roundtrip() {
        let info = BlobInfo {
            hash: "sha256:".to_string() + &"a".repeat(64),
            name: "test.txt".to_string(),
            size: 1024,
            content_type: "text/plain".to_string(),
            created_at: 1234567890,
        };
        let bytes = info.clone().into_bytes();
        let decoded = BlobInfo::from_bytes(Cow::Owned(bytes));
        assert_eq!(info.hash, decoded.hash);
        assert_eq!(info.name, decoded.name);
        assert_eq!(info.size, decoded.size);
        assert_eq!(info.content_type, decoded.content_type);
        assert_eq!(info.created_at, decoded.created_at);
    }
}
