use ts_function::ts_function;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts_function]
pub type SumCb = fn(a: f64, b: f64) -> f64;

#[ts_function]
pub type ConcatCb = fn(a: String, b: String) -> String;

#[ts_function]
pub type CheckCb = fn(v: i32) -> bool;

#[ts_function]
pub type IdentityCb = fn(v: JsValue) -> JsValue;

#[ts_function]
pub type BigIntCb = fn(v: u64) -> u64;

#[ts_function]
pub type VecCb = fn(v: Vec<u8>) -> Vec<u8>;

#[ts_function]
pub type BoxSliceCb = fn(v: Vec<u8>) -> Box<[u8]>;

#[ts_function]
pub type OptionStringCb = fn(v: Option<String>) -> Option<String>;

#[ts_function]
pub type ObjectCb = fn() -> js_sys::Object;

#[wasm_bindgen(module = "/tests/return_values.js")]
extern "C" {
    fn get_sum_cb() -> js_sys::Function;
    fn get_concat_cb() -> js_sys::Function;
    fn get_check_cb() -> js_sys::Function;
    fn get_identity_cb() -> js_sys::Function;
    fn get_bigint_cb() -> js_sys::Function;
    fn get_vec_cb() -> js_sys::Function;
    fn get_option_cb() -> js_sys::Function;
    fn get_object_cb() -> js_sys::Function;
}

#[wasm_bindgen_test]
fn test_return_values() {
    // 1. Numbers
    let sum_cb = SumCb::from(get_sum_cb());
    let res = sum_cb.call(10.5, 20.5).unwrap();
    assert_eq!(res, 31.0);

    // 2. Strings
    let concat_cb = ConcatCb::from(get_concat_cb());
    let res = concat_cb
        .call("foo".to_string(), "bar".to_string())
        .unwrap();
    assert_eq!(res, "foobar");

    // 3. Bools
    let check_cb = CheckCb::from(get_check_cb());
    assert!(check_cb.call(5).unwrap());
    assert!(!check_cb.call(-5).unwrap());

    // 4. JsValue
    let identity_cb = IdentityCb::from(get_identity_cb());
    let val = JsValue::from_str("test");
    let res = identity_cb.call(val.clone()).unwrap();
    assert_eq!(res, val);

    // 5. BigInt
    let bigint_cb = BigIntCb::from(get_bigint_cb());
    let res = bigint_cb.call(12345678901234567890).unwrap();
    assert_eq!(res, 12345678901234567890);

    // 6. Vec
    let vec_cb = VecCb::from(get_vec_cb());
    let res = vec_cb.call(vec![1, 2, 3]).unwrap();
    assert_eq!(res, vec![2, 4, 6]);

    // 7. Option
    let option_cb = OptionStringCb::from(get_option_cb());
    assert_eq!(
        option_cb.call(Some("hi".to_string())).unwrap(),
        Some("hi_suffix".to_string())
    );
    assert_eq!(option_cb.call(None).unwrap(), None);

    // 8. Box<[u8]>
    let box_slice_cb = BoxSliceCb::from(get_vec_cb());
    let res = box_slice_cb.call(vec![1, 2, 3]).unwrap();
    assert_eq!(&*res, &[2, 4, 6]);

    // 9. JsCast fallback (Object)
    let object_cb = ObjectCb::from(get_object_cb());
    let res = object_cb.call().unwrap();
    assert!(res.is_instance_of::<js_sys::Object>());
}
