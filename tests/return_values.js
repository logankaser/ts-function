export function get_identity_func() {
    return (val) => val;
}

export function get_sum_func() {
    return (a, b) => a + b;
}

export function get_concat_func() {
    return (a, b) => a + b;
}

export function get_check_func() {
    return (v) => v > 0;
}

export function get_bigint_func() {
    return (v) => v;
}

export function get_vec_func() {
    return (v) => v.map(x => x * 2);
}

export function get_option_func() {
    return (v) => v ? v + "_suffix" : null;
}

export function get_object_func() {
    return () => ({ "foo": "bar" });
}
