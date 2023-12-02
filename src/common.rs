use std::collections::HashSet;
use std::convert::TryInto;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::{
    builder::{PossibleValuesParser, TypedValueParser as _},
    Parser,
};
use fs_err as fs;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use path_slash::PathExt;
use which::{which, which_in};
use xwin::util::ProgressTarget;

/// common xwin options
#[derive(Clone, Debug, Parser)]
pub struct XWinOptions {
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

    /// The version to retrieve, can either be a major version of 15 or 16, or
    /// a "<major>.<minor>" version.
    #[arg(long, env = "XWIN_VERSION", default_value = "16", hide = true)]
    pub xwin_version: String,

    /// Whether or not to include debug libs
    #[arg(long, env = "XWIN_INCLUDE_DEBUG_LIBS", hide = true)]
    pub xwin_include_debug_libs: bool,
}

impl Default for XWinOptions {
    fn default() -> Self {
        Self {
            xwin_cache_dir: None,
            xwin_arch: vec![xwin::Arch::X86_64, xwin::Arch::Aarch64],
            xwin_variant: vec![xwin::Variant::Desktop],
            xwin_version: "16".to_string(),
            xwin_include_debug_libs: false,
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
        let xwin_cache_dir = self
            .xwin_cache_dir
            .clone()
            .unwrap_or_else(|| {
                dirs::cache_dir()
                    // If the really is no cache dir, cwd will also do
                    .unwrap_or_else(|| env::current_dir().expect("Failed to get current dir"))
                    .join(env!("CARGO_PKG_NAME"))
            })
            .join("xwin");
        fs::create_dir_all(&xwin_cache_dir)?;
        let xwin_cache_dir = xwin_cache_dir.canonicalize()?;

        let env_path = env::var("PATH").unwrap_or_default();
        let mut env_paths: Vec<_> = env::split_paths(&env_path).collect();

        let env_path = if cfg!(target_os = "macos") {
            let mut new_path = env_path;
            new_path.push_str(":/opt/homebrew/opt/llvm/bin");
            new_path.push_str(":/usr/local/opt/llvm/bin");
            new_path
        } else {
            env_path
        };
        let cache_dir = xwin_cache_dir.parent().unwrap();
        env_paths.push(cache_dir.to_path_buf());

        let workdir = manifest_path
            .and_then(|p| p.parent().map(|x| x.to_path_buf()))
            .or_else(|| env::current_dir().ok())
            .unwrap();
        let mut targets = cargo.target.clone();
        if targets.is_empty() {
            if let Some(build_target) = default_build_target_from_config(&workdir)? {
                // if no target is specified, use the default build target
                // Note that this is required, otherwise it may fail with link errors
                cmd.arg("--target").arg(&build_target);
                targets.push(build_target);
            }
        }

        for target in &targets {
            if target.contains("msvc") {
                self.setup_msvc_crt(xwin_cache_dir.clone())?;
                let env_target = target.to_lowercase().replace('-', "_");

                if which_in("clang-cl", Some(env_path.clone()), env::current_dir()?).is_err() {
                    if let Ok(clang) = which("clang") {
                        #[cfg(windows)]
                        {
                            let symlink = cache_dir.join("clang-cl.exe");
                            if symlink.exists() {
                                fs::remove_file(&symlink)?;
                            }
                            std::os::windows::fs::symlink_file(clang, symlink)?;
                        }

                        #[cfg(unix)]
                        {
                            let symlink = cache_dir.join("clang-cl");
                            if symlink.exists() {
                                fs::remove_file(&symlink)?;
                            }
                            std::os::unix::fs::symlink(clang, symlink)?;
                        }
                    }
                }
                symlink_llvm_tool("rust-lld", "lld-link", env_path.clone(), cache_dir)?;
                symlink_llvm_tool("llvm-ar", "llvm-lib", env_path.clone(), cache_dir)?;
                symlink_llvm_tool("llvm-ar", "llvm-dlltool", env_path.clone(), cache_dir)?;

                cmd.env("TARGET_CC", "clang-cl");
                cmd.env("TARGET_CXX", "clang-cl");
                cmd.env(format!("CC_{}", env_target), "clang-cl");
                cmd.env(format!("CXX_{}", env_target), "clang-cl");
                cmd.env("TARGET_AR", "llvm-lib");
                cmd.env(format!("AR_{}", env_target), "llvm-lib");

                cmd.env(
                    format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
                    "lld-link",
                );

                let user_set_cl_flags = env::var("CL_FLAGS").unwrap_or_default();
                let user_set_c_flags = env::var("CFLAGS").unwrap_or_default();
                let user_set_cxx_flags = env::var("CXXFLAGS").unwrap_or_default();

                let cl_flags = format!(
                    "--target={target} -Wno-unused-command-line-argument -fuse-ld=lld-link /imsvc{dir}/crt/include /imsvc{dir}/sdk/include/ucrt /imsvc{dir}/sdk/include/um /imsvc{dir}/sdk/include/shared {user_set_cl_flags}",
                    target = target,
                    dir = xwin_cache_dir.display(),
                    user_set_cl_flags = user_set_cl_flags,
                );
                cmd.env("CL_FLAGS", &cl_flags);
                cmd.env(
                    format!("CFLAGS_{}", env_target),
                    &format!(
                        "{cl_flags} {user_set_c_flags}",
                        cl_flags = cl_flags,
                        user_set_c_flags = user_set_c_flags
                    ),
                );
                cmd.env(
                    format!("CXXFLAGS_{}", env_target),
                    &format!(
                        "{cl_flags} {user_set_cxx_flags}",
                        cl_flags = cl_flags,
                        user_set_cxx_flags = user_set_cxx_flags
                    ),
                );

                cmd.env(
                    format!("BINDGEN_EXTRA_CLANG_ARGS_{}", env_target), 
                    format!(
                        "-I{dir}/crt/include -I{dir}/sdk/include/ucrt -I{dir}/sdk/include/um -I{dir}/sdk/include/shared",
                        dir = xwin_cache_dir.display()
                    )
                );

                cmd.env(
                    "RCFLAGS", 
                    format!(
                        "-I{dir}/crt/include -I{dir}/sdk/include/ucrt -I{dir}/sdk/include/um -I{dir}/sdk/include/shared",
                        dir = xwin_cache_dir.display()
                    )
                );

                let target_arch = target
                    .split_once('-')
                    .map(|(x, _)| x)
                    .context("invalid target triple")?;
                let xwin_arch = match target_arch {
                    "i586" | "i686" => "x86",
                    _ => target_arch,
                };

                let mut rustflags = get_rustflags(&workdir, target)?.unwrap_or_default();
                rustflags
                    .flags
                    .extend(["-C".to_string(), "linker-flavor=lld-link".to_string()]);
                rustflags.push(format!(
                    "-Lnative={dir}/crt/lib/{arch}",
                    dir = xwin_cache_dir.display(),
                    arch = xwin_arch
                ));
                rustflags.push(format!(
                    "-Lnative={dir}/sdk/lib/um/{arch}",
                    dir = xwin_cache_dir.display(),
                    arch = xwin_arch
                ));
                rustflags.push(format!(
                    "-Lnative={dir}/sdk/lib/ucrt/{arch}",
                    dir = xwin_cache_dir.display(),
                    arch = xwin_arch
                ));
                cmd.env("CARGO_ENCODED_RUSTFLAGS", rustflags.encode()?);

                #[cfg(target_os = "macos")]
                {
                    let usr_llvm = "/usr/local/opt/llvm/bin".into();
                    let opt_llvm = "/opt/homebrew/opt/llvm/bin".into();
                    if cfg!(target_arch = "x86_64") && !env_paths.contains(&usr_llvm) {
                        env_paths.push(usr_llvm);
                    } else if cfg!(target_arch = "aarch64") && !env_paths.contains(&opt_llvm) {
                        env_paths.push(opt_llvm);
                    }
                }

                cmd.env("PATH", env::join_paths(env_paths.clone())?);

                // CMake support
                let cmake_toolchain = self.setup_cmake_toolchain(target, &xwin_cache_dir)?;
                cmd.env("CMAKE_GENERATOR", "Ninja")
                    .env("CMAKE_SYSTEM_NAME", "Windows")
                    .env(
                        format!("CMAKE_TOOLCHAIN_FILE_{}", env_target),
                        cmake_toolchain,
                    );
            }
        }
        Ok(())
    }

    fn setup_msvc_crt(&self, cache_dir: PathBuf) -> Result<()> {
        let done_mark_file = cache_dir.join("DONE");
        let xwin_arches: HashSet<_> = self
            .xwin_arch
            .iter()
            .map(|x| x.as_str().to_string())
            .collect();
        let mut downloaded_arches = HashSet::new();
        if let Ok(content) = fs::read_to_string(&done_mark_file) {
            for arch in content.split_whitespace() {
                downloaded_arches.insert(arch.to_string());
            }
        }
        if xwin_arches.difference(&downloaded_arches).next().is_none() {
            return Ok(());
        }

        let draw_target = ProgressTarget::Stdout;

        let agent = http_agent()?;
        let xwin_dir = adjust_canonicalization(cache_dir.display().to_string());
        // timeout defaults to 60s
        let ctx = xwin::Ctx::with_dir(xwin::PathBuf::from(xwin_dir), draw_target, agent)?;
        let ctx = std::sync::Arc::new(ctx);
        let pkg_manifest = self.load_manifest(&ctx, draw_target)?;

        let arches = self
            .xwin_arch
            .iter()
            .fold(0, |acc, arch| acc | *arch as u32);
        let variants = self
            .xwin_variant
            .iter()
            .fold(0, |acc, var| acc | *var as u32);
        let pruned = xwin::prune_pkg_list(&pkg_manifest, arches, variants, false, None, None)?;
        let op = xwin::Ops::Splat(xwin::SplatConfig {
            include_debug_libs: self.xwin_include_debug_libs,
            include_debug_symbols: false,
            enable_symlinks: !cfg!(target_os = "macos"),
            preserve_ms_arch_notation: false,
            copy: false,
            output: cache_dir.clone().try_into()?,
            map: None,
        });
        let pkgs = pkg_manifest.packages;

        let mp = MultiProgress::with_draw_target(draw_target.into());
        let work_items: Vec<_> = pruned.payloads
        .into_iter()
        .map(|pay| {
            let prefix = match pay.kind {
                xwin::PayloadKind::CrtHeaders => "CRT.headers".to_owned(),
                xwin::PayloadKind::AtlHeaders => "ATL.headers".to_owned(),
                xwin::PayloadKind::CrtLibs => {
                    format!(
                        "CRT.libs.{}.{}",
                        pay.target_arch.map(|ta| ta.as_str()).unwrap_or("all"),
                        pay.variant.map(|v| v.as_str()).unwrap_or("none")
                    )
                }
                xwin::PayloadKind::AtlLibs => {
                    format!(
                        "ATL.libs.{}",
                        pay.target_arch.map(|ta| ta.as_str()).unwrap_or("all"),
                    )
                }
                xwin::PayloadKind::SdkHeaders => {
                    format!(
                        "SDK.headers.{}.{}",
                        pay.target_arch.map(|v| v.as_str()).unwrap_or("all"),
                        pay.variant.map(|v| v.as_str()).unwrap_or("none")
                    )
                }
                xwin::PayloadKind::SdkLibs => {
                    format!(
                        "SDK.libs.{}",
                        pay.target_arch.map(|ta| ta.as_str()).unwrap_or("all")
                    )
                }
                xwin::PayloadKind::SdkStoreLibs => "SDK.libs.store.all".to_owned(),
                xwin::PayloadKind::Ucrt => "SDK.ucrt.all".to_owned(),
            };

            let pb = mp.add(
                ProgressBar::with_draw_target(Some(0), draw_target.into()).with_prefix(prefix).with_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} {prefix:.bold} [{elapsed}] {wide_bar:.green} {bytes}/{total_bytes} {msg}").unwrap()
                        .progress_chars("=> "),
                ),
            );
            xwin::WorkItem {
                payload: std::sync::Arc::new(pay),
                progress: pb,
            }
        })
        .collect();

        mp.set_move_cursor(true);
        ctx.execute(pkgs, work_items, "".to_string(), arches, variants, op)?;

        let downloaded_arches: Vec<_> = self
            .xwin_arch
            .iter()
            .map(|x| x.as_str().to_string())
            .collect();
        fs::write(done_mark_file, downloaded_arches.join(" "))?;

        let dl = cache_dir.join("dl");
        if dl.exists() {
            let _ = fs::remove_dir_all(dl);
        }
        let unpack = cache_dir.join("unpack");
        if unpack.exists() {
            let _ = fs::remove_dir_all(unpack);
        }
        Ok(())
    }

    fn load_manifest(
        &self,
        ctx: &xwin::Ctx,
        dt: ProgressTarget,
    ) -> Result<xwin::manifest::PackageManifest> {
        let manifest_pb = ProgressBar::with_draw_target(Some(0), dt.into())
            .with_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} {prefix:.bold} [{elapsed}] {wide_bar:.green} {bytes}/{total_bytes} {msg}",
                )?
                .progress_chars("=> "),
        );
        manifest_pb.set_prefix("Manifest");
        manifest_pb.set_message("📥 downloading");

        let manifest =
            xwin::manifest::get_manifest(ctx, &self.xwin_version, "release", manifest_pb.clone())?;
        let pkg_manifest =
            xwin::manifest::get_package_manifest(ctx, &manifest, manifest_pb.clone())?;
        manifest_pb.finish_with_message("📥 downloaded");
        Ok(pkg_manifest)
    }

    fn setup_cmake_toolchain(&self, target: &str, xwin_cache_dir: &Path) -> Result<PathBuf> {
        let cmake_cache_dir = self
            .xwin_cache_dir
            .clone()
            .unwrap_or_else(|| {
                dirs::cache_dir()
                    // If the really is no cache dir, cwd will also do
                    .unwrap_or_else(|| env::current_dir().expect("Failed to get current dir"))
                    .join(env!("CARGO_PKG_NAME"))
            })
            .join("cmake");
        fs::create_dir_all(&cmake_cache_dir)?;

        let override_file = cmake_cache_dir.join("override.cmake");
        fs::write(override_file, include_bytes!("override.cmake"))?;

        let toolchain_file = cmake_cache_dir.join(format!("{}-toolchain.cmake", target));
        let target_arch = target
            .split_once('-')
            .map(|(x, _)| x)
            .context("invalid target triple")?;
        let processor = match target_arch {
            "i586" | "i686" => "X86",
            "x86_64" => "AMD64",
            "aarch64" => "ARM64",
            _ => target_arch,
        };
        let xwin_arch = match target_arch {
            "i586" | "i686" => "x86",
            _ => target_arch,
        };

        let content = format!(
            r#"
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR {processor})

set(CMAKE_C_COMPILER clang-cl CACHE FILEPATH "")
set(CMAKE_CXX_COMPILER clang-cl CACHE FILEPATH "")
set(CMAKE_AR llvm-lib)
set(CMAKE_LINKER lld-link CACHE FILEPATH "")

set(COMPILE_FLAGS
    --target={target}
    -Wno-unused-command-line-argument
    -fuse-ld=lld-link

    /imsvc{xwin_dir}/crt/include
    /imsvc{xwin_dir}/sdk/include/ucrt
    /imsvc{xwin_dir}/sdk/include/um
    /imsvc{xwin_dir}/sdk/include/shared)

set(LINK_FLAGS
    /manifest:no

    -libpath:"{xwin_dir}/crt/lib/{xwin_arch}"
    -libpath:"{xwin_dir}/sdk/lib/um/{xwin_arch}"
    -libpath:"{xwin_dir}/sdk/lib/ucrt/{xwin_arch}")

string(REPLACE ";" " " COMPILE_FLAGS "${{COMPILE_FLAGS}}")

set(_CMAKE_C_FLAGS_INITIAL "${{CMAKE_C_FLAGS}}" CACHE STRING "")
set(CMAKE_C_FLAGS "${{_CMAKE_C_FLAGS_INITIAL}} ${{COMPILE_FLAGS}}" CACHE STRING "" FORCE)

set(_CMAKE_CXX_FLAGS_INITIAL "${{CMAKE_CXX_FLAGS}}" CACHE STRING "")
set(CMAKE_CXX_FLAGS "${{_CMAKE_CXX_FLAGS_INITIAL}} ${{COMPILE_FLAGS}}" CACHE STRING "" FORCE)

string(REPLACE ";" " " LINK_FLAGS "${{LINK_FLAGS}}")

set(_CMAKE_EXE_LINKER_FLAGS_INITIAL "${{CMAKE_EXE_LINKER_FLAGS}}" CACHE STRING "")
set(CMAKE_EXE_LINKER_FLAGS "${{_CMAKE_EXE_LINKER_FLAGS_INITIAL}} ${{LINK_FLAGS}}" CACHE STRING "" FORCE)

set(_CMAKE_MODULE_LINKER_FLAGS_INITIAL "${{CMAKE_MODULE_LINKER_FLAGS}}" CACHE STRING "")
set(CMAKE_MODULE_LINKER_FLAGS "${{_CMAKE_MODULE_LINKER_FLAGS_INITIAL}} ${{LINK_FLAGS}}" CACHE STRING "" FORCE)

set(_CMAKE_SHARED_LINKER_FLAGS_INITIAL "${{CMAKE_SHARED_LINKER_FLAGS}}" CACHE STRING "")
set(CMAKE_SHARED_LINKER_FLAGS "${{_CMAKE_SHARED_LINKER_FLAGS_INITIAL}} ${{LINK_FLAGS}}" CACHE STRING "" FORCE)

# CMake populates these with a bunch of unnecessary libraries, which requires
# extra case-correcting symlinks and what not. Instead, let projects explicitly
# control which libraries they require.
set(CMAKE_C_STANDARD_LIBRARIES "" CACHE STRING "" FORCE)
set(CMAKE_CXX_STANDARD_LIBRARIES "" CACHE STRING "" FORCE)

set(CMAKE_TRY_COMPILE_CONFIGURATION Release)

# Allow clang-cl to work with macOS paths.
set(CMAKE_USER_MAKE_RULES_OVERRIDE "${{CMAKE_CURRENT_LIST_DIR}}/override.cmake")
        "#,
            target = target,
            processor = processor,
            xwin_dir = adjust_canonicalization(xwin_cache_dir.to_slash_lossy().to_string()),
            xwin_arch = xwin_arch,
        );
        fs::write(&toolchain_file, content)?;
        Ok(toolchain_file)
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

fn rustc_target_bin_dir() -> Result<PathBuf> {
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

fn default_build_target_from_config(workdir: &Path) -> Result<Option<String>> {
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
fn get_rustflags(workdir: &Path, target: &str) -> Result<Option<cargo_config2::Flags>> {
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
fn http_agent() -> Result<ureq::Agent> {
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
fn http_agent() -> Result<ureq::Agent> {
    use std::fs::File;
    use std::io;
    use std::sync::Arc;

    let builder = ureq::builder().try_proxy_from_env(true);
    if let Some(ca_bundle) = tls_ca_bundle() {
        let mut reader = io::BufReader::new(File::open(ca_bundle)?);
        let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
        let mut root_certs = rustls::RootCertStore::empty();
        root_certs.add_parsable_certificates(&certs);
        let client_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_certs)
            .with_no_client_auth();
        Ok(builder.tls_config(Arc::new(client_config)).build())
    } else {
        Ok(builder.build())
    }
}

#[cfg(not(any(feature = "native-tls", feature = "rustls")))]
fn http_agent() -> Result<ureq::Agent> {
    let builder = ureq::builder().try_proxy_from_env(true);
    Ok(builder.build())
}
