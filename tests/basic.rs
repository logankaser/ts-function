use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

// ==========================================
// Example 1: Primary API (Type Alias)
// Taking two Uint8Array and XORing them
// ==========================================

#[ts]
pub type XorFunction = fn(a: js_sys::Uint8Array, b: js_sys::Uint8Array);

#[wasm_bindgen(module = "/tests/basic.js")]
extern "C" {
    fn get_xor_function() -> js_sys::Function;
    fn get_xor_result() -> js_sys::Uint8Array;
}

#[wasm_bindgen_test]
fn test_example_1_primary_api() {
    // 1. Get the JS function that performs the XOR operation
    let js_func = get_xor_function();

    // 2. Wrap it into our typed wrapper (ts generated the From impl)
    let func = XorFunction::from(js_func);

    // 3. Create two Uint8Array inputs
    let a_bytes: &[u8] = &[0b10101010, 0b11110000, 0b00001111];
    let b_bytes: &[u8] = &[0b01010101, 0b11001100, 0b00110011];

    let a = js_sys::Uint8Array::from(a_bytes);
    let b = js_sys::Uint8Array::from(b_bytes);

    // 4. Call the JS function securely using our strongly typed Rust signature
    func.call(a, b).unwrap();

    // 5. Verify the JavaScript environment did the correct XOR operation
    let res = get_xor_result();
    let res_vec = res.to_vec();

    assert_eq!(res_vec, vec![0b11111111, 0b00111100, 0b00111100]);
}

// ==========================================
// Example 2: Escape Hatch API
// Custom error handling when JS throws
// ==========================================

pub struct SafeFunction(pub ::js_sys::Function);

#[ts]
impl SafeFunction {
    pub fn call(&self, val: f64) {
        // Execute the function but catch the error thrown by JS
        let result = self.0.call1(
            &::wasm_bindgen::JsValue::NULL,
            &::wasm_bindgen::JsValue::from_f64(val),
        );

        if let Err(e) = result {
            // Downcast the JsValue error to a JS Error object and record its message
            if let Ok(err_obj) = e.dyn_into::<js_sys::Error>() {
                set_handled_error(err_obj.message().into());
            }
        }
    }
}

#[wasm_bindgen(module = "/tests/basic.js")]
extern "C" {
    fn get_throwing_function() -> js_sys::Function;
    fn get_handled_error() -> String;
    fn set_handled_error(err: String);
}

#[wasm_bindgen_test]
fn test_example_2_escape_hatch() {
    // 1. Reset state
    set_handled_error("".to_string());

    // 2. Get the JS function that is guaranteed to throw an error
    let js_func = get_throwing_function();
    let func = SafeFunction::from(js_func);

    // 3. This call handles the error internally instead of panicking
    func.call(42.0);

    // 4. Verify the error message was caught and exported properly
    assert_eq!(get_handled_error(), "JS error with value 42");
}

// ==========================================
// Example 3: Function Struct Pattern
// Passing a struct of functions from JS
// ==========================================

#[ts]
pub type SimpleFn = fn(msg: String);

// ts generates `wasm_bindgen` ABI traits for `MyFunctions` so it maps automatically
#[ts]
struct MyFunctions {
    on_event: SimpleFn,
}

#[wasm_bindgen(module = "/tests/basic.js")]
extern "C" {
    fn get_simple_func() -> js_sys::Function;
    fn get_state_msg() -> String;
}

#[wasm_bindgen_test]
fn test_example_3_ts_struct() {
    // 1. Simulate the JS environment creating an object with function functions
    let raw_js_obj = js_sys::Object::new();
    js_sys::Reflect::set(&raw_js_obj, &"onEvent".into(), &get_simple_func()).unwrap();

    // 2. Convert to the Rust native struct!
    let functions = MyFunctions::try_from(JsValue::from(raw_js_obj)).unwrap();

    // 3. Fire the function!
    functions
        .on_event
        .call("Hello from ts!".to_string())
        .unwrap();

    // 4. Verify the JS function ran
    assert_eq!(get_state_msg(), "Hello from ts!");
}

// ==========================================
// Example 4: Deferred Parsing
// Testing the generated I-prefixed types
// ==========================================

#[wasm_bindgen_test]
fn test_example_4_deferred_parsing() {
    // 1. Simulate the JS environment creating an object with function functions
    let raw_js_obj = js_sys::Object::new();
    js_sys::Reflect::set(&raw_js_obj, &"onEvent".into(), &get_simple_func()).unwrap();

    // 2. Convert to the generated JS interface type `IMyFunctions` directly
    // This defers the full Rust struct conversion
    let ifunctions: IMyFunctions = JsValue::from(raw_js_obj).unchecked_into();

    // 3. We can access individual properties natively via JS getters
    let raw_fn: SimpleFn = ifunctions.on_event();
    raw_fn.call("Deferred run 1!".to_string()).unwrap();
    assert_eq!(get_state_msg(), "Deferred run 1!");

    // 4. We can still convert it fully using .parse()
    let functions: MyFunctions = ifunctions.parse();

    functions
        .on_event
        .call("Deferred run 2!".to_string())
        .unwrap();

    assert_eq!(get_state_msg(), "Deferred run 2!");
}
