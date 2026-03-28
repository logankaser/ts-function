#!/usr/bin/env sh
set -e

WASM_BINDGEN_TEST_ONLY_NODE=1 NODE="$(which node)" cargo test --target wasm32-unknown-unknown
cd typescript_test && pnpm test
