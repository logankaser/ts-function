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

#[wasm_bindgen(module = "/tests/args.js")]
extern "C" {
    fn get_args_0() -> js_sys::Function;
    fn get_args_1() -> js_sys::Function;
    fn get_args_2() -> js_sys::Function;
    fn get_args_3() -> js_sys::Function;

    fn get_args_state() -> js_sys::Float64Array;
}

#[wasm_bindgen_test]
fn test_arg_counts() {
    let cb0 = Cb0::from(get_args_0());
    cb0.call();
    assert_eq!(get_args_state().to_vec(), vec![0.0]);

    let cb1 = Cb1::from(get_args_1());
    cb1.call(10.0);
    assert_eq!(get_args_state().to_vec(), vec![1.0, 10.0]);

    let cb2 = Cb2::from(get_args_2());
    cb2.call(10.0, 20.0);
    assert_eq!(get_args_state().to_vec(), vec![2.0, 10.0, 20.0]);

    let cb3 = Cb3::from(get_args_3());
    cb3.call(10.0, 20.0, 30.0);
    assert_eq!(get_args_state().to_vec(), vec![3.0, 10.0, 20.0, 30.0]);
}
