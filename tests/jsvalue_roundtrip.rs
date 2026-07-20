use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
pub enum Status {
    Active = 1,
    Inactive = 3,
}

#[ts]
pub struct InnerStruct {
    pub value: f64,
}

#[ts]
pub struct AllTypes {
    // numbers
    pub f32_val: f32,
    pub f64_val: f64,
    pub i32_val: i32,
    pub u32_val: u32,

    // bigint
    pub i64_val: i64,
    pub u64_val: u64,

    // boolean
    pub bool_val: bool,

    // string
    pub string_val: String,

    // options
    pub opt_val: Option<String>,
    pub opt_none: Option<i32>,

    // typed arrays
    pub vec_u8: Vec<u8>,
    pub vec_f64: Vec<f64>,

    // js types
    pub js_val: JsValue,
    // Note: js_sys::Object roundtrip is tricky since parse() might just map to JsValue or not fully convert, but it should work directly via Reflect or JsCast
    pub js_obj: js_sys::Object,

    // nested struct
    pub inner: IInnerStruct,

    // enum
    pub status: Status,
}

fn all_types() -> AllTypes {
    AllTypes {
        f32_val: 1.5,
        f64_val: 2.5,
        i32_val: -10,
        u32_val: 20,
        i64_val: -100,
        u64_val: 200,
        bool_val: true,
        string_val: "test string".to_string(),
        opt_val: Some("optional test".to_string()),
        opt_none: None,
        vec_u8: vec![1, 2, 3],
        vec_f64: vec![1.1, 2.2, 3.3],
        js_val: JsValue::from_str("js_val_string"),
        js_obj: js_sys::Object::new(),
        inner: InnerStruct { value: 42.0 }.into(),
        status: Status::Inactive,
    }
}

#[wasm_bindgen_test]
fn test_round_trip() {
    let original = all_types();

    let js_val: JsValue = original.into();

    assert!(js_val.is_object());

    // Convert back using the generated `From<IAllTypes> for AllTypes` and `unchecked_into`
    let i_basic: IAllTypes = js_val.unchecked_into::<IAllTypes>();
    let back_to_rust: AllTypes = i_basic.into();

    assert_eq!(back_to_rust.f32_val, 1.5);
    assert_eq!(back_to_rust.f64_val, 2.5);
    assert_eq!(back_to_rust.i32_val, -10);
    assert_eq!(back_to_rust.u32_val, 20);
    assert_eq!(back_to_rust.i64_val, -100);
    assert_eq!(back_to_rust.u64_val, 200);
    assert_eq!(back_to_rust.bool_val, true);
    assert_eq!(back_to_rust.string_val, "test string");
    assert_eq!(back_to_rust.opt_val, Some("optional test".to_string()));
    assert_eq!(back_to_rust.opt_none, None);
    assert_eq!(back_to_rust.vec_u8, vec![1, 2, 3]);
    assert_eq!(back_to_rust.vec_f64, vec![1.1, 2.2, 3.3]);
    assert!(back_to_rust.js_val.is_string());
    assert_eq!(back_to_rust.js_val.as_string().unwrap(), "js_val_string");
    assert!(back_to_rust.js_obj.is_object());
    let back_inner: InnerStruct = back_to_rust.inner.into();
    assert_eq!(back_inner.value, 42.0);
    assert_eq!(back_to_rust.status as u32, Status::Inactive as u32);
}

#[wasm_bindgen_test]
fn test_try_parse_success() {
    let original = all_types();

    let js_val: JsValue = original.into();
    let binding: IAllTypes = js_val.unchecked_into::<IAllTypes>();
    let parsed: AllTypes = binding.try_parse().unwrap();

    assert_eq!(parsed.f32_val, 1.5);
    assert_eq!(parsed.f64_val, 2.5);
    assert_eq!(parsed.string_val, "test string");
    let inner_parsed: InnerStruct = parsed.inner.into();
    assert_eq!(inner_parsed.value, 42.0);
}

#[wasm_bindgen_test]
fn test_try_parse_missing_required() {
    let raw_js_obj = js_sys::Object::new();
    let binding: IAllTypes = JsValue::from(raw_js_obj).unchecked_into::<IAllTypes>();
    let result = binding.try_parse();

    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        _ => panic!("Expected error"),
    };
    let err_str = err.as_string().unwrap();
    assert_eq!(err_str, "Missing required field `f32Val`");
}

#[wasm_bindgen_test]
fn test_try_parse_invalid_type() {
    let original = all_types();

    let js_val: JsValue = original.into();
    // set string_val to a number, which is an invalid type
    js_sys::Reflect::set(&js_val, &"stringVal".into(), &42.into()).unwrap();

    let binding: IAllTypes = js_val.unchecked_into::<IAllTypes>();
    let result = binding.try_parse();

    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        _ => panic!("Expected error"),
    };
    assert_eq!(
        err.as_string().unwrap(),
        "Invalid field `stringVal`: Expected a string"
    );
}

#[wasm_bindgen_test]
fn test_try_parse_invalid_typed_arrays() {
    let js_val: JsValue = all_types().into();
    js_sys::Reflect::set(&js_val, &"vecU8".into(), &js_sys::Array::new()).unwrap();
    let binding: IAllTypes = js_val.unchecked_into::<IAllTypes>();
    let error = match binding.try_parse() {
        Err(error) => error,
        Ok(_) => panic!("Expected invalid Uint8Array to fail"),
    };
    assert_eq!(
        error.as_string().unwrap(),
        "Invalid field `vecU8`: Expected a Uint8Array"
    );

    let js_val: JsValue = all_types().into();
    js_sys::Reflect::set(
        &js_val,
        &"vecF64".into(),
        &JsValue::from_str("not an array"),
    )
    .unwrap();
    let binding: IAllTypes = js_val.unchecked_into::<IAllTypes>();
    let error = match binding.try_parse() {
        Err(error) => error,
        Ok(_) => panic!("Expected invalid Float64Array to fail"),
    };
    assert_eq!(
        error.as_string().unwrap(),
        "Invalid field `vecF64`: Expected a Float64Array"
    );
}

#[wasm_bindgen_test]
fn test_try_parse_invalid_enum_values() {
    for value in [
        JsValue::from_f64(-1.0),
        JsValue::from_f64(1.5),
        JsValue::from_f64(f64::NAN),
        JsValue::from_f64(2.0),
        JsValue::from_f64(99.0),
    ] {
        let error = match Status::try_from(value) {
            Err(error) => error,
            Ok(_) => panic!("Expected invalid enum variant to fail"),
        };
        assert!(
            error
                .as_string()
                .unwrap()
                .contains("Invalid Status variant")
        );
    }

    assert!(matches!(
        Status::try_from(JsValue::from_f64(1.0)),
        Ok(Status::Active)
    ));
    assert!(matches!(
        Status::try_from(JsValue::from_f64(3.0)),
        Ok(Status::Inactive)
    ));
}
