const state = {
  val: null,
};

export function set_func_val(val) {
  state.val = val;
}
export function get_func() {
  return (val) => {
    state.val = val;
  };
}
export function get_func_state() {
  return state.val;
}
