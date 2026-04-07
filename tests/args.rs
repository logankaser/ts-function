use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
pub type Fn0 = fn();

#[ts]
pub type Fn1 = fn(a: f64);

#[ts]
pub type Fn2 = fn(a: f64, b: f64);

#[ts]
pub type Fn3 = fn(a: f64, b: f64, c: f64);

#[ts]
pub type Fn5 = fn(a: f64, b: f64, c: f64, d: f64, e: f64);

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
    let func3 = Fn3::from(get_args_3());
    func3.call(10.0, 20.0, 30.0).unwrap();
    assert_eq!(get_args_state().to_vec(), vec![3.0, 10.0, 20.0, 30.0]);

    let func5 = Fn5::from(get_args_5());
    func5.call(1.0, 2.0, 3.0, 4.0, 5.0).unwrap();
    assert_eq!(
        get_args_state().to_vec(),
        vec![5.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    );
}
