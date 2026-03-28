let state = {
    last_array: [],
};

export function get_array_cb() {
    return (arr) => {
        state.last_array = Array.from(arr);
    };
}

export function get_last_array() {
    return new Float64Array(state.last_array);
}
