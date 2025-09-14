use std::ffi::CStr;

extern "C" {
    fn BZ2_bzlibVersion() -> *const std::os::raw::c_char;
}

fn main() {
    unsafe {
        let version_ptr = BZ2_bzlibVersion();
        let version = CStr::from_ptr(version_ptr).to_str().unwrap();
        println!("bzip2 version: {}", version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bzip2_version() {
        unsafe {
            let version_ptr = BZ2_bzlibVersion();
            let version = CStr::from_ptr(version_ptr).to_str().unwrap();
            println!("bzip2 version: {}", version);
            assert!(!version.is_empty());
        }
    }
}