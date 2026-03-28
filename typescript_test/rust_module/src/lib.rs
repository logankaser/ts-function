use ts_function::ts_function;
use ts_macro::ts;
use wasm_bindgen::prelude::*;

#[ts_function]
pub type SingleArgCb = fn(msg: String);

#[ts_function]
pub type MultiArgCb = fn(a: f64, b: js_sys::Uint8Array);

#[ts_function]
pub type OptionCb = fn(val: Option<String>);

#[ts]
struct AppCallbacks {
    on_ready: SingleArgCb,
    on_data: MultiArgCb,
    on_option: OptionCb,
}

#[wasm_bindgen]
pub fn execute_callbacks(cbs: IAppCallbacks) {
    let callbacks: AppCallbacks = cbs.parse();

    callbacks.on_ready.call("System is ready".to_string());

    let arr = js_sys::Uint8Array::new_with_length(3);
    arr.copy_from(&[1, 2, 3]);
    callbacks.on_data.call(42.5, arr);

    callbacks.on_option.call(Some("present".to_string()));
}

#[ts_function]
pub type ThrowCb = fn();

#[wasm_bindgen]
pub fn test_console_log(cb: js_sys::Function) {
    let cb = ThrowCb::from(cb);
    // Since the `console` feature is ON for this crate,
    // this will NOT panic. It will log to the console and continue.
    cb.call();
}
