let state = {
    args: []
};

export function get_args_0() { return () => { state.args = [0]; }; }
export function get_args_1() { return (a) => { state.args = [1, a]; }; }
export function get_args_2() { return (a, b) => { state.args = [2, a, b]; }; }
export function get_args_3() { return (a, b, c) => { state.args = [3, a, b, c]; }; }

export function get_args_state() {
    let res = new Float64Array(state.args.length);
    for (let i = 0; i < state.args.length; i++) res[i] = state.args[i];
    return res;
}
