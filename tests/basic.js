let state = {
    xor_result: new Uint8Array(),
    handled_error: "",
    msg: "",
};

// --- Example 1 ---
export function get_xor_callback() {
    return (a, b) => {
        let res = new Uint8Array(a.length);
        for (let i = 0; i < a.length; i++) {
            res[i] = a[i] ^ b[i];
        }
        state.xor_result = res;
    };
}
export function get_xor_result() { return state.xor_result; }

// --- Example 2 ---
export function get_throwing_callback() {
    return (val) => {
        throw new Error("JS error with value " + val);
    };
}
export function get_handled_error() { return state.handled_error; }
export function set_handled_error(err) { state.handled_error = err; }

// --- Example 3 ---
export function get_simple_cb() {
    return (msg) => {
        state.msg = msg;
    };
}
export function get_state_msg() { return state.msg; }
