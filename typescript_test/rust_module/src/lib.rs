use ts_function::ts;
use wasm_bindgen::prelude::*;

#[ts]
pub enum UserStatus {
    Active,
    Inactive,
}

#[ts]
pub type SingleArgFn = fn(msg: String);

#[ts]
pub type MultiArgFn = fn(a: f64, b: js_sys::Uint8Array);

#[ts]
pub type OptionFn = fn(val: Option<String>);

#[ts]
pub type ReturnValueFn = fn(a: f64) -> f64;

#[ts]
pub type StatusFn = fn(status: UserStatus);

#[ts]
pub struct AppFunctions {
    on_ready: SingleArgFn,
    on_data: MultiArgFn,
    on_option: OptionFn,
    on_calculate: ReturnValueFn,
    on_status: StatusFn,
}

#[wasm_bindgen]
pub fn execute_functions(functions: AppFunctions) {
    functions
        .on_ready
        .call("System is ready".to_string())
        .unwrap();

    let arr = js_sys::Uint8Array::new_with_length(3);
    arr.copy_from(&[1, 2, 3]);
    functions.on_data.call(42.5, arr).unwrap();

    functions
        .on_option
        .call(Some("present".to_string()))
        .unwrap();

    let result = functions.on_calculate.call(10.0).unwrap();
    if result != 20.0 {
        panic!("Calculation failed: expected 20.0, got {}", result);
    }

    functions.on_status.call(UserStatus::Active).unwrap();
}
