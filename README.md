# ts-function

[![Crates.io](https://img.shields.io/crates/v/ts-function.svg)](https://crates.io/crates/ts-function)
[![Docs.rs](https://docs.rs/ts-function/badge.svg)](https://docs.rs/ts-function)
[![Rust 2024](https://img.shields.io/badge/edition-2024-red.svg)](https://rust-lang.org)

A proc-macro that generates TypeScript type aliases and `wasm-bindgen` ABI trait
implementations for Rust function wrapper structs, enabling strongly-typed
Typescript Functions types in `ts-macro` projects with little to no boilerplate.

## Motivation

When using [`ts-macro`](https://crates.io/crates/ts-macro) to generate
TypeScript interfaces for Rust structs, there is no built-in support for
function types. A struct field like `on_click: js_sys::Function`
produces an opaque `Function` type in TypeScript and forces you to manually
handle raw `JsValue` conversions in Rust.

`ts-function` bridges this gap. It allows you to define function signatures in
pure Rust, automatically generating the correct TypeScript types (`(args: ...)
=> void`) and implementing the `wasm-bindgen` ABI traits required to pass those
functions across the Wasm boundary safely.

## Example: Unified `#[ts]` Attribute

The `#[ts]` attribute is a one-stop-shop for generating TypeScript bindings for structs, enums, and functions.

```rust
use wasm_bindgen::prelude::*;
use ts_function::ts;

// 1. Define your enums - automatically applies #[wasm_bindgen]
#[ts]
pub enum UserStatus { Active, Inactive }

// 2. Define your function signatures using type aliases
#[ts]
pub type SingleArgFn = fn(msg: String);

#[ts]
pub type MultiArgFn = fn(a: f64, b: js_sys::Uint8Array);

// 3. Use `#[ts]` to create a struct containing your function wrappers
#[ts]
struct AppCallbacks {
    on_ready: SingleArgFn,
    on_data: MultiArgFn,
    status: UserStatus,
}

// 4. Export a function that accepts your struct from JS
#[wasm_bindgen]
pub fn execute_callbacks(cbs: IAppCallbacks) {
    // `.parse()` safely extracts the `js_sys::Function` objects into your strongly-typed wrappers
    let callbacks: AppCallbacks = cbs.parse();
    
    // Call the functions! Arguments are automatically converted to `JsValue`.
    // The `call` method returns a `Result<(), JsValue>` forwarding any JS exceptions.
    callbacks.on_ready.call("System is ready".to_string()).unwrap();
    
    let arr = js_sys::Uint8Array::new_with_length(3);
    arr.copy_from(&[1, 2, 3]);
    callbacks.on_data.call(42.5, arr).unwrap();
}
```

### Generated TypeScript (`index.d.ts`)

`wasm-bindgen` outputs the types you would expect:

```typescript
export enum UserStatus {
    Active = 0,
    Inactive = 1,
}

type SingleArgFn = (msg: string) => void;
type MultiArgFn = (a: number, b: Uint8Array) => void;

interface IAppCallbacks {
    onReady: SingleArgFn;
    onData: MultiArgFn;
    status: UserStatus;
}

export function execute_callbacks(cbs: IAppCallbacks): void;
```

### Example: Returning a Value

```rust
#[ts]
pub type CalculateFn = fn(a: f64) -> f64;

// Usage:
let result: f64 = calculate_fn.call(10.0).unwrap();
```

## Supported Types

`ts-function` supports a wide range of Rust types, automatically mapping them to their idiomatic TypeScript equivalents.

| Rust Type | TypeScript Type | Notes |
| :--- | :--- | :--- |
| `f32`, `f64`, `i8`-`i32`, `u8`-`u32` | `number` | |
| `i64`, `u64` | `bigint` | Mapped via `js_sys::BigInt` |
| `bool` | `boolean` | |
| `String`, `&str` | `string` | |
| `Option<T>` | `T | undefined` | |
| `Vec<u8>`, `&[u8]` | `Uint8Array` | See [Zero-Copy Performance Tip](#zero-copy-performance-tip) |
| `Vec<f64>`, `&[f64]` | `Float64Array` | Also supports `i8`, `u16`, `i32`, etc. |
| `JsValue` | `any` | |
| `js_sys::Object` | `object` | |
| Rust Struct | Typescript Object | Any `JsCast` type (including `ts` interfaces) |

## Return Values & Performance

The `#[ts]` macro supports returning values from JavaScript back into
Rust. It handles standard primitives, strings, options, and even numeric vectors
(via `TypedArray`).

### Zero-Copy Performance

When returning large arrays, using `Vec<u8>` or similar will force a memory copy
from the JavaScript heap to the Rust heap. To achieve zero-copy performance,
you can return a `js_sys` type directly:

```rust
#[ts]
pub type LargeDataFn = fn() -> js_sys::Uint8Array;

// Usage (Zero-Copy):
let arr: js_sys::Uint8Array = large_data_fn.call().unwrap();
```

## Advanced Usage: The Escape Hatch

If you need completely custom serialization or want to embed specific side-effects and error handling directly into the function execution, you can use the escape hatch.
By applying `#[ts]` to an `impl` block for a tuple-struct wrapping `js_sys::Function`, you take complete control of the `call` method's implementation.

**`ts-function` will still parse your `call` method's signature and automatically generate the correct TypeScript interface for it!**

```rust
use wasm_bindgen::prelude::*;
use ts_function::ts;

// 1. Define your own tuple struct
pub struct CustomLoggingFn(pub js_sys::Function);

// 2. Apply the macro to the impl block
#[ts]
impl CustomLoggingFn {
    // 3. Define the `call` signature exactly how you want it exposed to TypeScript
    pub fn call(&self, val: f64) {
        // You are responsible for all conversions and calling the JS function
        let result = self.0.call1(
            &wasm_bindgen::JsValue::NULL,
            &wasm_bindgen::JsValue::from_f64(val),
        );

        // Handle errors internally however you want
        if let Err(e) = result {
            if let Ok(err_obj) = e.dyn_into::<js_sys::Error>() {
                web_sys::console::error_1(&err_obj.message().into());
            }
        }
    }
}
```

## License

Dual-licensed under either the [MIT license](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE) at your option.
