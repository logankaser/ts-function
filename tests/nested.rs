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
