fn main() {
    cc::Build::new().file("src/probe.c").compile("c_probe");
    cc::Build::new()
        .cpp(true)
        .file("src/probe.cpp")
        .compile("cpp_probe");

    // The remaining include-flag consumers must receive the same intact paths.
    let target = std::env::var("TARGET").unwrap().replace('-', "_");
    for key in [
        format!("BINDGEN_EXTRA_CLANG_ARGS_{target}"),
        "RCFLAGS".into(),
    ] {
        let flags = shlex::split(&std::env::var(&key).unwrap()).unwrap();
        assert_eq!(flags.len(), 5, "{key}: {flags:?}");
        for flag in flags {
            assert!(
                std::path::Path::new(flag.strip_prefix("-I").unwrap()).is_dir(),
                "{key}: {flag}"
            );
        }
    }
}
