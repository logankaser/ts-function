use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
pub type FnString = fn(a: String);

#[ts]
pub type FnIntoString = fn(a: impl Into<String>);

#[wasm_bindgen(module = "/tests/types.js")]
extern "C" {
    fn get_func() -> js_sys::Function;
    fn get_func_state() -> String;
}

#[wasm_bindgen_test]
fn test_complex_types() {
    let func_string = FnString::from(get_func());
    func_string.call("hello".to_string()).unwrap();
    assert_eq!(get_func_state(), "hello");

    let func_into_string = FnIntoString::from(get_func());
    func_into_string.call("world").unwrap(); // Passing &str to impl Into<String>
    assert_eq!(get_func_state(), "world");
}
