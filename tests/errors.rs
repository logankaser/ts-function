use ts_function::ts_function;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts_function]
pub type ThrowCb = fn();

#[wasm_bindgen(module = "/tests/errors.js")]
extern "C" {
    fn get_throw_cb() -> js_sys::Function;
}

#[wasm_bindgen_test]
fn test_default_behavior_returns_err() {
    let cb = ThrowCb::from(get_throw_cb());

    // This call will throw an error in JS. Now it returns a Result::Err
    // instead of panicking.
    let res = cb.call();
    assert!(res.is_err());

    let err = res.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Intentional JavaScript Error"));
}
