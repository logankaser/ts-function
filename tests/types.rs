use ts_function::ts_function;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts_function]
pub type CbString = fn(a: String);

#[ts_function]
pub type CbIntoString = fn(a: impl Into<String>);

#[wasm_bindgen(module = "/tests/types.js")]
extern "C" {
    fn get_cb() -> js_sys::Function;
    fn get_cb_state() -> String;
}

#[wasm_bindgen_test]
fn test_complex_types() {
    let cb_string = CbString::from(get_cb());
    cb_string.call("hello".to_string());
    assert_eq!(get_cb_state(), "hello");

    let cb_into_string = CbIntoString::from(get_cb());
    cb_into_string.call("world"); // Passing &str to impl Into<String>
    assert_eq!(get_cb_state(), "world");
}
