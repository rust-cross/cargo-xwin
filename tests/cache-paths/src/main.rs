unsafe extern "C" {
    fn c_probe() -> i32;
    fn cpp_probe() -> i32;
}

fn main() {
    unsafe {
        assert_eq!(c_probe(), 4);
        assert_eq!(cpp_probe(), 4);
    }
}
