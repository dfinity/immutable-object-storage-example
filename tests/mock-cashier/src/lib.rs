use std::cell::RefCell;

use candid::Principal;

thread_local! {
    static PRINCIPALS: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
}

#[ic_cdk::init]
fn init(principals: Vec<Principal>) {
    PRINCIPALS.with(|p| *p.borrow_mut() = principals);
}

#[ic_cdk::query]
fn storage_gateway_principal_list_v1() -> Vec<Principal> {
    PRINCIPALS.with(|p| p.borrow().clone())
}

ic_cdk::export_candid!();
