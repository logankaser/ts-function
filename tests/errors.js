export function get_throw_cb() {
    return () => {
        throw new Error("Intentional JavaScript Error");
    };
}
