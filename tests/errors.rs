use wasm_bindgen_test::*;
use wasm_bindgen::prelude::*;
use ts_function::ts_function;

#[ts_function]
pub type ThrowCb = fn();

#[wasm_bindgen(module = "/tests/errors.js")]
extern "C" {
    fn get_throw_cb() -> js_sys::Function;
}

#[wasm_bindgen_test]
#[should_panic(expected = "JavaScript exception: JsValue(Error: Intentional JavaScript Error")]
fn test_default_behavior_panics() {
    let cb = ThrowCb::from(get_throw_cb());
    
    // This call will throw an error in JS. Because the `console` feature is OFF
    // by default, the macro will generate code that catches the error and panics.
    cb.call();
}
