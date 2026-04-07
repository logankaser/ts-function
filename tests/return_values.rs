use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
pub type SumFn = fn(a: f64, b: f64) -> f64;

#[ts]
pub type ConcatFn = fn(a: String, b: String) -> String;

#[ts]
pub type CheckFn = fn(v: i32) -> bool;

#[ts]
pub type IdentityFn = fn(v: JsValue) -> JsValue;

#[ts]
pub type BigIntFn = fn(v: u64) -> u64;

#[ts]
pub type VecFn = fn(v: Vec<u8>) -> Vec<u8>;

#[ts]
pub type BoxSliceFn = fn(v: Vec<u8>) -> Box<[u8]>;

#[ts]
pub type OptionStringFn = fn(v: Option<String>) -> Option<String>;

#[ts]
pub type ObjectFn = fn() -> js_sys::Object;

#[wasm_bindgen(module = "/tests/return_values.js")]
extern "C" {
    fn get_sum_func() -> js_sys::Function;
    fn get_concat_func() -> js_sys::Function;
    fn get_check_func() -> js_sys::Function;
    fn get_identity_func() -> js_sys::Function;
    fn get_bigint_func() -> js_sys::Function;
    fn get_vec_func() -> js_sys::Function;
    fn get_option_func() -> js_sys::Function;
    fn get_object_func() -> js_sys::Function;
}

#[wasm_bindgen_test]
fn test_return_values() {
    // 1. Numbers
    let sum_func = SumFn::from(get_sum_func());
    let res = sum_func.call(10.5, 20.5).unwrap();
    assert_eq!(res, 31.0);

    // 2. Strings
    let concat_func = ConcatFn::from(get_concat_func());
    let res = concat_func
        .call("foo".to_string(), "bar".to_string())
        .unwrap();
    assert_eq!(res, "foobar");

    // 3. Bools
    let check_func = CheckFn::from(get_check_func());
    assert!(check_func.call(5).unwrap());
    assert!(!check_func.call(-5).unwrap());

    // 4. JsValue
    let identity_func = IdentityFn::from(get_identity_func());
    let val = JsValue::from_str("test");
    let res = identity_func.call(val.clone()).unwrap();
    assert_eq!(res, val);

    // 5. BigInt
    let bigint_func = BigIntFn::from(get_bigint_func());
    let res = bigint_func.call(12345678901234567890).unwrap();
    assert_eq!(res, 12345678901234567890);

    // 6. Vec
    let vec_func = VecFn::from(get_vec_func());
    let res = vec_func.call(vec![1, 2, 3]).unwrap();
    assert_eq!(res, vec![2, 4, 6]);

    // 7. Option
    let option_func = OptionStringFn::from(get_option_func());
    assert_eq!(
        option_func.call(Some("hi".to_string())).unwrap(),
        Some("hi_suffix".to_string())
    );
    assert_eq!(option_func.call(None).unwrap(), None);

    // 8. Box<[u8]>
    let box_slice_func = BoxSliceFn::from(get_vec_func());
    let res = box_slice_func.call(vec![1, 2, 3]).unwrap();
    assert_eq!(&*res, &[2, 4, 6]);

    // 9. JsCast fallback (Object)
    let object_func = ObjectFn::from(get_object_func());
    let res = object_func.call().unwrap();
    assert!(res.is_instance_of::<js_sys::Object>());
}
