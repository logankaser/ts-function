use ts_function::ts;
use wasm_bindgen_test::*;

#[ts]
pub enum UserStatus {
    Active,
    Inactive,
}

#[ts]
pub struct UserProfile {
    pub status: UserStatus,
}

#[ts]
pub type StatusChangeFn = fn(new_status: UserStatus);

#[wasm_bindgen_test]
fn test_enums() {
    // This is mostly to ensure it compiles and the macros don't panic.
    // The actual verification of the generated typescript is done in unit tests.
    let status = UserStatus::Active;
    let profile = UserProfile { status };
    assert!(matches!(profile.status, UserStatus::Active));
}
