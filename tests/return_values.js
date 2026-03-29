export function get_identity_cb() {
    return (val) => val;
}

export function get_sum_cb() {
    return (a, b) => a + b;
}

export function get_concat_cb() {
    return (a, b) => a + b;
}

export function get_check_cb() {
    return (v) => v > 0;
}

export function get_bigint_cb() {
    return (v) => v;
}

export function get_vec_cb() {
    return (v) => v.map(x => x * 2);
}

export function get_option_cb() {
    return (v) => v ? v + "_suffix" : null;
}

export function get_object_cb() {
    return () => ({ "foo": "bar" });
}
