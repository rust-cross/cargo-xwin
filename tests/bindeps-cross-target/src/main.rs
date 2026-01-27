// This test crate uses an artifact dependency targeting a different platform.
// The artifact (tool) is built for linux-musl, while this crate is built for windows-msvc.
// This tests that cargo-xwin correctly scopes its linker flags to only the Windows target.

fn main() {
    #[cfg(target_os = "windows")]
    {
        // The tool binary path is provided via environment variable by cargo
        let bin_path = env!("CARGO_BIN_FILE_LINUX_BIN");
        println!("Binary path: {}", bin_path);
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("This test is only for Windows targets");
    }
}
