use ts_function::{ts, ts_function};
use wasm_bindgen::prelude::*;

#[ts_function]
pub type SingleArgCb = fn(msg: String);

#[ts_function]
pub type MultiArgCb = fn(a: f64, b: js_sys::Uint8Array);

#[ts_function]
pub type OptionCb = fn(val: Option<String>);

#[ts_function]
pub type ReturnValueCb = fn(a: f64) -> f64;

#[ts]
struct AppCallbacks {
    on_ready: SingleArgCb,
    on_data: MultiArgCb,
    on_option: OptionCb,
    on_calculate: ReturnValueCb,
}

#[wasm_bindgen]
pub fn execute_callbacks(cbs: IAppCallbacks) {
    let callbacks: AppCallbacks = cbs.parse();

    callbacks
        .on_ready
        .call("System is ready".to_string())
        .unwrap();

    let arr = js_sys::Uint8Array::new_with_length(3);
    arr.copy_from(&[1, 2, 3]);
    callbacks.on_data.call(42.5, arr).unwrap();

    callbacks
        .on_option
        .call(Some("present".to_string()))
        .unwrap();

    let result = callbacks.on_calculate.call(10.0).unwrap();
    if result != 20.0 {
        panic!("Calculation failed: expected 20.0, got {}", result);
    }
}
