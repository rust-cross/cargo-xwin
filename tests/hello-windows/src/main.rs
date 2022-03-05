use winapi::um::sysinfoapi::GetVersion as WinApiGetVersion;
use windows_sys::Win32::System::SystemInformation::GetVersion as WindowsSysGetVersion;

fn main() {
    let v1 = unsafe { WinApiGetVersion() };
    println!("version from winapi: {}", v1);

    let v2 = unsafe { WindowsSysGetVersion() };
    println!("version from windows-sys: {}", v2);

    assert_eq!(v1, v2);
}

#[cfg(test)]
mod test {
    use winapi::um::sysinfoapi::GetVersion as WinApiGetVersion;
    use windows_sys::Win32::System::SystemInformation::GetVersion as WindowsSysGetVersion;

    #[test]
    fn test_winapi_get_version() {
        let v1 = unsafe { WinApiGetVersion() };
        println!("version from winapi: {}", v1);
    }

    #[test]
    fn test_windows_sys_get_version() {
        let v2 = unsafe { WindowsSysGetVersion() };
        println!("version from windows-sys: {}", v2);
    }
}
