let state = {
    val: null,
};

export function set_cb_val(val) {
    state.val = val;
}
export function get_cb() {
    return (val) => {
        state.val = val;
    };
}
export function get_cb_state() { return state.val; }
