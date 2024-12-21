use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser as _},
    Parser, ValueEnum,
};
use fs_err as fs;
use which::which_in;

/// MSVC cross compiler backend
#[derive(Clone, Debug, Default, ValueEnum)]
pub enum CompilerBackend {
    /// clang-cl backend
    #[default]
    ClangCl,
}

/// common xwin options
#[derive(Clone, Debug, Parser)]
pub struct XWinOptions {
    /// The cross compiler backend to use
    #[arg(long, env = "XWIN_COMPILER_BACKEND", default_value = "clang-cl")]
    pub compiler_backend: CompilerBackend,

    /// xwin cache directory
    #[arg(long, env = "XWIN_CACHE_DIR", hide = true)]
    pub xwin_cache_dir: Option<PathBuf>,

    /// The architectures to include in CRT/SDK
    #[arg(
        long,
        env = "XWIN_ARCH",
        value_parser = PossibleValuesParser::new(["x86", "x86_64", "aarch", "aarch64"])
            .map(|s| s.parse::<xwin::Arch>().unwrap()),
        value_delimiter = ',',
        default_values_t = vec![xwin::Arch::X86_64, xwin::Arch::Aarch64],
        hide = true,
    )]
    pub xwin_arch: Vec<xwin::Arch>,

    /// The variants to include
    #[arg(
        long,
        env = "XWIN_VARIANT",
        value_parser = PossibleValuesParser::new(["desktop", "onecore", /*"store",*/ "spectre"])
            .map(|s| s.parse::<xwin::Variant>().unwrap()),
        value_delimiter = ',',
        default_values_t = vec![xwin::Variant::Desktop],
        hide = true,
    )]
    pub xwin_variant: Vec<xwin::Variant>,

    /// The version to retrieve, can either be a major version of 15, 16 or 17, or
    /// a "<major>.<minor>" version.
    #[arg(long, env = "XWIN_VERSION", default_value = "16", hide = true)]
    pub xwin_version: String,

    /// Whether or not to include debug libs
    #[arg(long, env = "XWIN_INCLUDE_DEBUG_LIBS", hide = true)]
    pub xwin_include_debug_libs: bool,

    /// Whether or not to include debug symbols (PDBs)
    #[arg(long, env = "XWIN_INCLUDE_DEBUG_SYMBOLS", hide = true)]
    pub xwin_include_debug_symbols: bool,
}

impl Default for XWinOptions {
    fn default() -> Self {
        Self {
            xwin_cache_dir: None,
            xwin_arch: vec![xwin::Arch::X86_64, xwin::Arch::Aarch64],
            xwin_variant: vec![xwin::Variant::Desktop],
            xwin_version: "16".to_string(),
            xwin_include_debug_libs: false,
            xwin_include_debug_symbols: false,
            compiler_backend: CompilerBackend::ClangCl,
        }
    }
}

impl XWinOptions {
    pub fn apply_command_env(
        &self,
        manifest_path: Option<&Path>,
        cargo: &cargo_options::CommonOptions,
        cmd: &mut Command,
    ) -> Result<()> {
        match self.compiler_backend {
            CompilerBackend::ClangCl => {
                let clang_cl = crate::backend::clang_cl::ClangCl::new(self);
                clang_cl.apply_command_env(manifest_path, cargo, cmd)?;
            }
        }
        Ok(())
    }
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
pub fn symlink_llvm_tool(
    tool: &str,
    link_name: &str,
    env_path: String,
    cache_dir: &Path,
) -> Result<()> {
    if which_in(link_name, Some(env_path), env::current_dir()?).is_err() {
        let bin_dir = rustc_target_bin_dir()?;
        let rust_tool = bin_dir.join(tool);
        if rust_tool.exists() {
            #[cfg(windows)]
            {
                let symlink = cache_dir.join(format!("{}.exe", link_name));
                if symlink.exists() {
                    fs::remove_file(&symlink)?;
                }
                std::os::windows::fs::symlink_file(rust_tool, symlink)?;
            }

            #[cfg(unix)]
            {
                let symlink = cache_dir.join(link_name);
                if symlink.exists() {
                    fs::remove_file(&symlink)?;
                }
                std::os::unix::fs::symlink(rust_tool, symlink)?;
            }
        }
    }
    Ok(())
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
    use std::fs::File;
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
    use std::fs::File;
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
