export function get_throw_func() {
  return () => {
    throw new Error("Intentional JavaScript Error");
  };
}
