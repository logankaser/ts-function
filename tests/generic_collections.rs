use std::rc::Rc;
use std::sync::Arc;
use ts_function::ts_function;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts_function]
pub type BoxCb = fn(data: Box<[u8]>);

#[ts_function]
pub type AsRefCb = fn(data: impl AsRef<[f64]>);

#[ts_function]
pub type IntoVecCb = fn(data: impl Into<Vec<u32>>);

#[ts_function]
pub type ArcCb = fn(data: Arc<[i32]>);

#[ts_function]
pub type RcCb = fn(data: Rc<[i16]>);

#[wasm_bindgen(module = "/tests/generic_collections.js")]
extern "C" {
    fn get_array_cb() -> js_sys::Function;
    fn get_last_array() -> js_sys::Float64Array;
}

#[wasm_bindgen_test]
fn test_generic_collections() {
    let js_cb = get_array_cb();

    // 1. Test Box<[u8]>
    let cb = BoxCb::from(js_cb.clone());
    cb.call(vec![1, 2, 3].into_boxed_slice());
    assert_eq!(get_last_array().to_vec(), vec![1.0, 2.0, 3.0]);

    // 2. Test impl AsRef<[f64]>
    let cb = AsRefCb::from(js_cb.clone());
    let data = vec![4.0, 5.0];
    cb.call(&data);
    assert_eq!(get_last_array().to_vec(), vec![4.0, 5.0]);

    // 3. Test impl Into<Vec<u32>>
    let cb = IntoVecCb::from(js_cb.clone());
    cb.call(vec![10u32, 20u32]);
    assert_eq!(get_last_array().to_vec(), vec![10.0, 20.0]);

    // 4. Test Arc<[i32]>
    let cb = ArcCb::from(js_cb.clone());
    cb.call(Arc::from(vec![-1, -2].into_boxed_slice()));
    assert_eq!(get_last_array().to_vec(), vec![-1.0, -2.0]);

    // 5. Test Rc<[i16]>
    let cb = RcCb::from(js_cb);
    cb.call(Rc::from(vec![100, 200].into_boxed_slice()));
    assert_eq!(get_last_array().to_vec(), vec![100.0, 200.0]);
}
