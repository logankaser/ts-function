use ts_function::ts_function;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts_function]
pub type Cb0 = fn();

#[ts_function]
pub type Cb1 = fn(a: f64);

#[ts_function]
pub type Cb2 = fn(a: f64, b: f64);

#[ts_function]
pub type Cb3 = fn(a: f64, b: f64, c: f64);

#[ts_function]
pub type Cb5 = fn(a: f64, b: f64, c: f64, d: f64, e: f64);

#[wasm_bindgen(module = "/tests/args.js")]
extern "C" {
    fn get_args_0() -> js_sys::Function;
    fn get_args_1() -> js_sys::Function;
    fn get_args_2() -> js_sys::Function;
    fn get_args_3() -> js_sys::Function;
    fn get_args_5() -> js_sys::Function;

    fn get_args_state() -> js_sys::Float64Array;
}

#[wasm_bindgen_test]
fn test_arg_counts() {
    // ...
    let cb3 = Cb3::from(get_args_3());
    cb3.call(10.0, 20.0, 30.0).unwrap();
    assert_eq!(get_args_state().to_vec(), vec![3.0, 10.0, 20.0, 30.0]);

    let cb5 = Cb5::from(get_args_5());
    cb5.call(1.0, 2.0, 3.0, 4.0, 5.0).unwrap();
    assert_eq!(
        get_args_state().to_vec(),
        vec![5.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    );
}
