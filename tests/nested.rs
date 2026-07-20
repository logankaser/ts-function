// Test logic adapted from the wasm-utils-rs project
// (https://github.com/ryangoree/wasm-utils-rs).
//
// Original Copyright 2024 DELV, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ts_function::ts;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[ts]
struct Token {
    symbol: String,
    decimals: Option<u8>,
    total_supply: u64,
}

#[ts]
struct Order {
    account: String,
    amount: u64,
    token: Token, // Binding to the `Token` struct
}

#[wasm_bindgen(module = "/tests/nested.js")]
extern "C" {
    fn get_order() -> Order;
}

#[wasm_bindgen_test]
fn test_nested_structs() {
    let order = get_order();

    let token = &order.token;
    let symbol = &token.symbol;
    let decimals = token.decimals.unwrap_or(18);
    let total_supply = token.total_supply;
    let account = &order.account;
    let amount = order.amount;

    assert_eq!(symbol, "FOO");
    assert_eq!(decimals, 18);
    assert_eq!(total_supply, 100);
    assert_eq!(account, "0xAlice");
    assert_eq!(amount, 500);

    let rust_token: Token = order.token.into();
    assert_eq!(rust_token.symbol, "FOO");
    assert_eq!(rust_token.decimals, None);
    assert_eq!(rust_token.total_supply, 100);
}

#[wasm_bindgen_test]
fn test_nested_try_parse_success() {
    let order: Order = get_order();
    let order_js: JsValue = order.into();
    let binding: IOrder = order_js.unchecked_into::<IOrder>();
    let parsed: Order = binding.try_parse().unwrap();

    assert_eq!(parsed.account, "0xAlice");
    assert_eq!(parsed.amount, 500);
    assert_eq!(parsed.token.symbol, "FOO");
    assert_eq!(parsed.token.decimals, None);
    assert_eq!(parsed.token.total_supply, 100);
}

#[wasm_bindgen_test]
fn test_nested_try_parse_failure_missing_nested_field() {
    let order: Order = get_order();
    let order_js: JsValue = order.into();
    // remove the symbol field from the nested token
    let token_js = js_sys::Reflect::get(&order_js, &"token".into()).unwrap();
    let token_obj = token_js.unchecked_into::<js_sys::Object>();
    js_sys::Reflect::delete_property(&token_obj, &"symbol".into()).unwrap();

    let binding: IOrder = order_js.unchecked_into::<IOrder>();
    let result = binding.try_parse();

    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        _ => panic!("Expected error"),
    };
    assert_eq!(
        err.as_string().unwrap(),
        "Invalid field `token`: Missing required field `symbol`"
    );
}

#[wasm_bindgen_test]
fn test_nested_try_parse_failure_invalid_nested_field() {
    let order: Order = get_order();
    let order_js: JsValue = order.into();
    let token_js = js_sys::Reflect::get(&order_js, &"token".into()).unwrap();
    js_sys::Reflect::set(&token_js, &"symbol".into(), &42.into()).unwrap();

    let binding: IOrder = order_js.unchecked_into::<IOrder>();
    let error = match binding.try_parse() {
        Err(error) => error,
        Ok(_) => panic!("Expected invalid nested field to fail"),
    };
    assert_eq!(
        error.as_string().unwrap(),
        "Invalid field `token`: Invalid field `symbol`: Expected a string"
    );
}
