//! Example canister implementing the immutable object storage protocol.
//!
//! The protocol implementation lives in [`storage`]. This file provides the
//! canister's `MemoryManager` (one per canister) and example app endpoints.

mod storage;

use std::cell::RefCell;

use candid::Principal;
use ic_cdk::init;
use ic_cdk::query;
use ic_cdk::update;
use ic_stable_structures::memory_manager::MemoryManager;
use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::DefaultMemoryImpl;
pub use storage::BlobInfo;
pub use storage::CreateCertificateResult;

// =============================================================================
// Stable memory — one MemoryManager per canister
// =============================================================================

type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
}

#[init]
fn init() {}

// =============================================================================
// App API (example — adapt to your needs)
// =============================================================================

/// Attach display metadata to a blob after upload.
#[update]
fn set_blob_info(hash: String, name: String, size: u64, content_type: String) {
    if let Some(mut info) = storage::get_blob(&hash) {
        info.name = name;
        info.size = size;
        info.content_type = content_type;
        storage::update_blob(&hash, info);
    }
}

/// List all live (non-pending-delete) blobs, newest first.
#[query]
fn list_blobs() -> Vec<BlobInfo> {
    let mut blobs = storage::list_live_blobs();
    blobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    blobs
}

/// Mark a blob for deletion.
#[update]
fn delete_blob(hash: String) {
    storage::mark_for_deletion(&hash);
}

ic_cdk::export_candid!();
