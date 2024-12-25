use anyhow::Result;
use fs_err as fs;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use which::which_in;

/// Sets up the environment path by adding necessary directories to the existing `PATH`.
///
/// On macOS, it checks for specific LLVM installation paths based on the architecture
/// and adds them to the front of the environment paths if they exist.
/// It then appends the `cache_dir` provided to the list of paths.
pub fn setup_env_path(cache_dir: &Path) -> Result<OsString> {
    let env_path = env::var("PATH").unwrap_or_default();
    let mut env_paths: Vec<_> = env::split_paths(&env_path).collect();
    if cfg!(target_os = "macos") {
        // setup macos homebrew llvm paths
        let usr_llvm = "/usr/local/opt/llvm/bin".into();
        let opt_llvm = "/opt/homebrew/opt/llvm/bin".into();
        if cfg!(target_arch = "x86_64")
            && Path::new(&usr_llvm).is_dir()
            && !env_paths.contains(&usr_llvm)
        {
            env_paths.insert(0, usr_llvm);
        } else if cfg!(target_arch = "aarch64")
            && Path::new(&opt_llvm).is_dir()
            && !env_paths.contains(&opt_llvm)
        {
            env_paths.insert(0, opt_llvm);
        }
    }
    env_paths.push(cache_dir.to_path_buf());
    Ok(env::join_paths(env_paths)?)
}

/// Sets up symlinks for LLVM tools in the provided environment path and cache directory.
///
/// This function creates symlinks for the following tools:
/// - `rust-lld` to `lld-link`
/// - `llvm-ar` to `llvm-lib`
/// - `llvm-ar` to `llvm-dlltool`
///
/// These symlinks are established if they do not already exist in the specified environment path.
pub fn setup_llvm_tools(env_path: &OsStr, cache_dir: &Path) -> Result<()> {
    symlink_llvm_tool("rust-lld", "lld-link", env_path, cache_dir)?;
    symlink_llvm_tool("llvm-ar", "llvm-lib", env_path, cache_dir)?;
    symlink_llvm_tool("llvm-ar", "llvm-dlltool", env_path, cache_dir)?;
    Ok(())
}

/// Configures the environment variables for the target compiler and linker.
///
/// This function sets up environment variables for the specified target compiler and linker,
/// allowing the build system to correctly use the desired tools for compilation and linking.
/// It sets up the following environment variables:
/// - `TARGET_CC` and `TARGET_CXX` with the provided compiler.
/// - `CC_<env_target>` and `CXX_<env_target>` with the provided compiler.
/// - `TARGET_AR` and `AR_<env_target>` with "llvm-lib".
/// - `CARGO_TARGET_<env_target>_LINKER` with "lld-link".
pub fn setup_target_compiler_and_linker_env(cmd: &mut Command, env_target: &str, compiler: &str) {
    cmd.env("TARGET_CC", compiler);
    cmd.env("TARGET_CXX", compiler);
    cmd.env(format!("CC_{}", env_target), compiler);
    cmd.env(format!("CXX_{}", env_target), compiler);
    cmd.env("TARGET_AR", "llvm-lib");
    cmd.env(format!("AR_{}", env_target), "llvm-lib");
    cmd.env(
        format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
        "lld-link",
    );
}

/// Configures the environment variables for CMake to use the Ninja generator and Windows system.
///
/// This function sets up the following environment variables:
/// - `CMAKE_GENERATOR` as "Ninja".
/// - `CMAKE_SYSTEM_NAME` as "Windows".
/// - `CMAKE_TOOLCHAIN_FILE_<env_target>` with the provided toolchain path, where `<env_target>` is the target string
///   converted to lowercase and hyphens replaced with underscores.
pub fn setup_cmake_env(cmd: &mut Command, target: &str, toolchain_path: PathBuf) {
    let env_target = target.to_lowercase().replace('-', "_");
    cmd.env("CMAKE_GENERATOR", "Ninja")
        .env("CMAKE_SYSTEM_NAME", "Windows")
        .env(
            format!("CMAKE_TOOLCHAIN_FILE_{}", env_target),
            toolchain_path,
        );
}

pub fn rustc_target_bin_dir() -> Result<PathBuf> {
    let output = Command::new("rustc")
        .args(["--print", "target-libdir"])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let lib_dir = Path::new(&stdout);
    let bin_dir = lib_dir.parent().unwrap().join("bin");
    Ok(bin_dir)
}

/// Symlink Rust provided llvm tool component
fn symlink_llvm_tool(
    tool: &str,
    link_name: &str,
    env_path: &OsStr,
    cache_dir: &Path,
) -> Result<()> {
    if which_in(link_name, Some(env_path), env::current_dir()?).is_err() {
        let bin_dir = rustc_target_bin_dir()?;
        let rust_tool = bin_dir.join(tool);
        if rust_tool.exists() {
            #[cfg(windows)]
            {
                let symlink = cache_dir.join(format!("{}.exe", link_name));
                if symlink.is_symlink() || symlink.is_file() {
                    fs::remove_file(&symlink)?;
                }
                fs_err::os::windows::fs::symlink_file(rust_tool, symlink)?;
            }

            #[cfg(unix)]
            {
                let symlink = cache_dir.join(link_name);
                if symlink.is_symlink() || symlink.is_file() {
                    fs::remove_file(&symlink)?;
                }
                fs_err::os::unix::fs::symlink(rust_tool, symlink)?;
            }
        }
    }
    Ok(())
}

#[cfg(target_family = "unix")]
pub fn adjust_canonicalization(p: String) -> String {
    p
}

#[cfg(target_os = "windows")]
pub fn adjust_canonicalization(p: String) -> String {
    const VERBATIM_PREFIX: &str = r#"\\?\"#;
    if let Some(p) = p.strip_prefix(VERBATIM_PREFIX) {
        p.to_string()
    } else {
        p
    }
}

pub fn default_build_target_from_config(workdir: &Path) -> Result<Option<String>> {
    let output = Command::new("cargo")
        .current_dir(workdir)
        .args([
            "config",
            "get",
            "-Z",
            "unstable-options",
            "--format",
            "json-value",
            "build.target",
        ])
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8(output.stdout)?;
    let target = stdout.trim().trim_matches('"');
    Ok(Some(target.to_string()))
}

/// Get RUSTFLAGS in the following order:
///
/// 1. `RUSTFLAGS` environment variable.
/// 2. `rustflags` cargo configuration
pub fn get_rustflags(workdir: &Path, target: &str) -> Result<Option<cargo_config2::Flags>> {
    let cargo_config = cargo_config2::Config::load_with_cwd(workdir)?;
    let rustflags = cargo_config.rustflags(target)?;
    Ok(rustflags)
}

#[cfg(any(feature = "native-tls", feature = "rustls"))]
fn tls_ca_bundle() -> Option<std::ffi::OsString> {
    env::var_os("REQUESTS_CA_BUNDLE")
        .or_else(|| env::var_os("CURL_CA_BUNDLE"))
        .or_else(|| env::var_os("SSL_CERT_FILE"))
}

#[cfg(all(feature = "native-tls", not(feature = "rustls")))]
pub fn http_agent() -> Result<ureq::Agent> {
    use fs_err::File;
    use std::io;
    use std::sync::Arc;

    let mut builder = ureq::builder().try_proxy_from_env(true);
    let mut tls_builder = native_tls_crate::TlsConnector::builder();
    if let Some(ca_bundle) = tls_ca_bundle() {
        let mut reader = io::BufReader::new(File::open(ca_bundle)?);
        for cert in rustls_pemfile::certs(&mut reader) {
            let cert = cert?;
            tls_builder.add_root_certificate(native_tls_crate::Certificate::from_pem(&cert)?);
        }
    }
    builder = builder.tls_connector(Arc::new(tls_builder.build()?));
    Ok(builder.build())
}

#[cfg(feature = "rustls")]
pub fn http_agent() -> Result<ureq::Agent> {
    use fs_err::File;
    use std::io;
    use std::sync::Arc;

    let builder = ureq::builder().try_proxy_from_env(true);
    if let Some(ca_bundle) = tls_ca_bundle() {
        let mut reader = io::BufReader::new(File::open(ca_bundle)?);
        let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
        let mut root_certs = rustls::RootCertStore::empty();
        root_certs.add_parsable_certificates(certs);
        let client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_certs)
            .with_no_client_auth();
        Ok(builder.tls_config(Arc::new(client_config)).build())
    } else {
        Ok(builder.build())
    }
}

#[cfg(not(any(feature = "native-tls", feature = "rustls")))]
pub fn http_agent() -> Result<ureq::Agent> {
    let builder = ureq::builder().try_proxy_from_env(true);
    Ok(builder.build())
}
