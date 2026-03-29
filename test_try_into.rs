use wasm_bindgen::JsValue;

fn test(v: JsValue) {
    let _: Result<i64, _> = std::convert::TryInto::<i64>::try_into(v);
}
fn main() {}
