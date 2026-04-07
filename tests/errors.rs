use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
pub type ThrowFn = fn();

#[wasm_bindgen(module = "/tests/errors.js")]
extern "C" {
    fn get_throw_func() -> js_sys::Function;
}

#[wasm_bindgen_test]
fn test_default_behavior_returns_err() {
    let func = ThrowFn::from(get_throw_func());

    // This call will throw an error in JS. Now it returns a Result::Err
    // instead of panicking.
    let res = func.call();
    assert!(res.is_err());

    let err = res.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Intentional JavaScript Error"));
}
