use std::rc::Rc;
use std::sync::Arc;
use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
pub type BoxFn = fn(data: Box<[u8]>);

#[ts]
pub type AsRefFn = fn(data: impl AsRef<[f64]>);

#[ts]
pub type IntoVecFn = fn(data: impl Into<Vec<u32>>);

#[ts]
pub type ArcFn = fn(data: Arc<[i32]>);

#[ts]
pub type RcFn = fn(data: Rc<[i16]>);

#[wasm_bindgen(module = "/tests/generic_collections.js")]
extern "C" {
    fn get_array_func() -> js_sys::Function;
    fn get_last_array() -> js_sys::Float64Array;
}

#[wasm_bindgen_test]
fn test_generic_collections() {
    let js_func = get_array_func();

    // 1. Test Box<[u8]>
    let func = BoxFn::from(js_func.clone());
    func.call(vec![1, 2, 3].into_boxed_slice()).unwrap();
    assert_eq!(get_last_array().to_vec(), vec![1.0, 2.0, 3.0]);

    // 2. Test impl AsRef<[f64]>
    let func = AsRefFn::from(js_func.clone());
    let data = vec![4.0, 5.0];
    func.call(&data).unwrap();
    assert_eq!(get_last_array().to_vec(), vec![4.0, 5.0]);

    // 3. Test impl Into<Vec<u32>>
    let func = IntoVecFn::from(js_func.clone());
    func.call(vec![10u32, 20u32]).unwrap();
    assert_eq!(get_last_array().to_vec(), vec![10.0, 20.0]);

    // 4. Test Arc<[i32]>
    let func = ArcFn::from(js_func.clone());
    func.call(Arc::from(vec![-1, -2].into_boxed_slice()))
        .unwrap();
    assert_eq!(get_last_array().to_vec(), vec![-1.0, -2.0]);

    // 5. Test Rc<[i16]>
    let func = RcFn::from(js_func);
    func.call(Rc::from(vec![100, 200].into_boxed_slice()))
        .unwrap();
    assert_eq!(get_last_array().to_vec(), vec![100.0, 200.0]);
}
