use std::collections::HashSet;
use std::convert::TryInto;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs_err as fs;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use path_slash::PathExt;
use which::which_in;
use xwin::util::ProgressTarget;

use crate::compiler::common::{
    adjust_canonicalization, default_build_target_from_config, get_rustflags, http_agent,
    is_static_crt_enabled, setup_cmake_env, setup_env_path, setup_llvm_tools,
    setup_target_compiler_and_linker_env,
};
use crate::options::XWinOptions;

#[derive(Debug)]
pub struct ClangCl<'a> {
    xwin_options: &'a XWinOptions,
}

impl<'a> ClangCl<'a> {
    pub fn new(xwin_options: &'a XWinOptions) -> Self {
        Self { xwin_options }
    }

    pub fn apply_command_env(
        &self,
        manifest_path: Option<&Path>,
        cargo: &cargo_options::CommonOptions,
        cache_dir: PathBuf,
        cmd: &mut Command,
    ) -> Result<()> {
        let env_path = setup_env_path(&cache_dir)?;

        let xwin_cache_dir = cache_dir.join("xwin");
        fs::create_dir_all(&xwin_cache_dir).context("Failed to create xwin cache dir")?;
        let xwin_cache_dir = xwin_cache_dir
            .canonicalize()
            .context("Failed to canonicalize xwin cache dir")?;

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
                self.setup_msvc_crt_with_retry(xwin_cache_dir.clone())
                    .context("Failed to setup MSVC CRT")?;
                let env_target = target.to_lowercase().replace('-', "_");

                setup_clang_cl_symlink(&env_path, &cache_dir)
                    .context("Failed to setup clang-cl symlink")?;
                setup_llvm_tools(&env_path, &cache_dir).context("Failed to setup LLVM tools")?;
                setup_target_compiler_and_linker_env(cmd, &env_target, "clang-cl");

                let user_set_cl_flags = env::var("CL_FLAGS").unwrap_or_default();
                let user_set_c_flags = env::var("CFLAGS").unwrap_or_default();
                let user_set_cxx_flags = env::var("CXXFLAGS").unwrap_or_default();

                let target_arch = target
                    .split_once('-')
                    .map(|(x, _)| x)
                    .context("invalid target triple")?;
                let xwin_arch = match target_arch {
                    "i586" | "i686" => "x86",
                    _ => target_arch,
                };

                let xwin_dir = adjust_canonicalization(xwin_cache_dir.to_slash_lossy().to_string());
                let mut cl_flags = vec![
                    format!("--target={target}"),
                    "-Wno-unused-command-line-argument".to_string(),
                    "-fuse-ld=lld-link".to_string(),
                    format!("/imsvc {dir}/crt/include", dir = xwin_dir),
                    format!("/imsvc {dir}/sdk/include/ucrt", dir = xwin_dir),
                    format!("/imsvc {dir}/sdk/include/um", dir = xwin_dir),
                    format!("/imsvc {dir}/sdk/include/shared", dir = xwin_dir),
                    format!("/imsvc {dir}/sdk/include/winrt", dir = xwin_dir),
                ];
                if !user_set_cl_flags.is_empty() {
                    cl_flags.push(user_set_cl_flags.clone());
                }
                let cl_flags = cl_flags.join(" ");
                cmd.env("CL_FLAGS", &cl_flags);
                cmd.env(
                    format!("CFLAGS_{env_target}"),
                    format!("{cl_flags} {user_set_c_flags}",),
                );
                cmd.env(
                    format!("CXXFLAGS_{env_target}"),
                    format!("{cl_flags} /EHsc {user_set_cxx_flags}",),
                );

                cmd.env(
                    format!("BINDGEN_EXTRA_CLANG_ARGS_{env_target}"),
                    format!(
                        "-I{dir}/crt/include -I{dir}/sdk/include/ucrt -I{dir}/sdk/include/um -I{dir}/sdk/include/shared -I{dir}/sdk/include/winrt",
                        dir = xwin_dir
                    )
                );

                cmd.env(
                    "RCFLAGS",
                    format!(
                        "-I{dir}/crt/include -I{dir}/sdk/include/ucrt -I{dir}/sdk/include/um -I{dir}/sdk/include/shared -I{dir}/sdk/include/winrt",
                        dir = xwin_dir
                    )
                );

                // Set LIB environment variable for clang-cl library path resolution
                let lib_paths = vec![
                    format!("{dir}/crt/lib/{arch}", dir = xwin_dir, arch = xwin_arch),
                    format!("{dir}/sdk/lib/um/{arch}", dir = xwin_dir, arch = xwin_arch),
                    format!(
                        "{dir}/sdk/lib/ucrt/{arch}",
                        dir = xwin_dir,
                        arch = xwin_arch
                    ),
                ];
                let existing_lib = env::var("LIB").unwrap_or_default();
                let lib_value = if existing_lib.is_empty() {
                    lib_paths.join(";")
                } else {
                    format!("{};{}", lib_paths.join(";"), existing_lib)
                };
                cmd.env("LIB", lib_value);

                let mut rustflags = get_rustflags(&workdir, target)?.unwrap_or_default();
                rustflags
                    .flags
                    .extend(["-C".to_string(), "linker-flavor=lld-link".to_string()]);

                // Check if static CRT is enabled
                let is_static_crt = is_static_crt_enabled(&workdir, target)?;
                if is_static_crt {
                    // When using static CRT, we need to link against the static version of libucrt
                    // instead of the import library. This resolves issues with symbols like
                    // __stdio_common_vsscanf being marked as dllimport.
                    rustflags.flags.extend([
                        "-C".to_string(),
                        "link-arg=-nodefaultlib:ucrt".to_string(),
                        "-C".to_string(),
                        "link-arg=-defaultlib:libucrt".to_string(),
                    ]);
                }

                rustflags.push(format!(
                    "-Lnative={dir}/crt/lib/{arch}",
                    dir = xwin_dir,
                    arch = xwin_arch
                ));
                rustflags.push(format!(
                    "-Lnative={dir}/sdk/lib/um/{arch}",
                    dir = xwin_dir,
                    arch = xwin_arch
                ));
                rustflags.push(format!(
                    "-Lnative={dir}/sdk/lib/ucrt/{arch}",
                    dir = xwin_dir,
                    arch = xwin_arch
                ));
                // Remove RUSTFLAGS from environment so that the spawned Cargo respects our
                // CARGO_TARGET_<triple>_RUSTFLAGS. When RUSTFLAGS is present, Cargo prioritizes
                // it over CARGO_TARGET_<triple>_RUSTFLAGS. The flags from RUSTFLAGS are already
                // included in `rustflags` via cargo-config2's resolution.
                cmd.env_remove("RUSTFLAGS");

                // Use `CARGO_TARGET_<TRIPLE>_RUSTFLAGS` to avoid the flags being passed to artifact
                // dependencies built for other targets.
                cmd.env(
                    format!("CARGO_TARGET_{}_RUSTFLAGS", env_target.to_uppercase()),
                    rustflags.encode_space_separated()?,
                );
                cmd.env("PATH", &env_path);

                // CMake support
                let cmake_toolchain = self
                    .setup_cmake_toolchain(target, &xwin_cache_dir, is_static_crt)
                    .with_context(|| format!("Failed to setup CMake toolchain for {}", target))?;
                setup_cmake_env(cmd, target, cmake_toolchain);
            }
        }
        Ok(())
    }

    fn setup_msvc_crt_with_retry(&self, cache_dir: PathBuf) -> Result<()> {
        let max_retries = self.xwin_options.xwin_http_retries;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let wait_time = Duration::from_secs(2u64.pow(attempt - 1));
                std::thread::sleep(wait_time);
                eprintln!(
                    "Retrying MSVC CRT setup (attempt {}/{})",
                    attempt + 1,
                    max_retries + 1
                );
                self.cleanup_partial_download(&cache_dir);
            }

            match self.setup_msvc_crt(cache_dir.clone()) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to setup MSVC CRT")))
    }

    fn cleanup_partial_download(&self, cache_dir: &Path) {
        let dl_dir = cache_dir.join("dl");
        if dl_dir.exists() {
            let _ = fs::remove_dir_all(&dl_dir);
        }
        let unpack_dir = cache_dir.join("unpack");
        if unpack_dir.exists() {
            let _ = fs::remove_dir_all(&unpack_dir);
        }
    }

    /// Downloads and extracts the specified MSVC CRT components into the specified `cache_dir`.
    fn setup_msvc_crt(&self, cache_dir: PathBuf) -> Result<()> {
        let done_mark_file = cache_dir.join("DONE");
        let xwin_arches: HashSet<_> = self
            .xwin_options
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
        let xwin_dir = adjust_canonicalization(cache_dir.to_slash_lossy().to_string());
        // timeout defaults to 60s, retry 3 times
        let ctx = xwin::Ctx::with_dir(xwin::PathBuf::from(xwin_dir), draw_target, agent, 3)?;
        let ctx = std::sync::Arc::new(ctx);
        let pkg_manifest = self.load_manifest(&ctx, draw_target)?;

        let arches = self
            .xwin_options
            .xwin_arch
            .iter()
            .fold(0, |acc, arch| acc | *arch as u32);
        let variants = self
            .xwin_options
            .xwin_variant
            .iter()
            .fold(0, |acc, var| acc | *var as u32);
        let pruned = xwin::prune_pkg_list(
            &pkg_manifest,
            arches,
            variants,
            self.xwin_options.xwin_include_atl,
            false,
            self.xwin_options.xwin_sdk_version.clone(),
            self.xwin_options.xwin_crt_version.clone(),
        )?;
        let op = xwin::Ops::Splat(xwin::SplatConfig {
            include_debug_libs: self.xwin_options.xwin_include_debug_libs,
            include_debug_symbols: self.xwin_options.xwin_include_debug_symbols,
            enable_symlinks: !cfg!(target_os = "macos"),
            preserve_ms_arch_notation: false,
            use_winsysroot_style: false,
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
                xwin::PayloadKind::VcrDebug => {
                    let prefix = match pay.filename.to_string().contains("UCRT") {
                        true => "UCRT.Debug",
                        false => "VC.Runtime.Debug"
                    };

                    format!(
                        "{}.{}",
                        prefix,
                        pay.target_arch.map_or("all", |ta| ta.as_str())
                    )
                }

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
        if mp.is_hidden() {
            eprintln!("⏬ Downloading MSVC CRT...");
        }
        let start_time = Instant::now();
        ctx.execute(
            pkgs,
            work_items,
            pruned.crt_version,
            pruned.sdk_version,
            None,
            arches,
            variants,
            op,
        )?;

        let downloaded_arches: Vec<_> = self
            .xwin_options
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
        if mp.is_hidden() {
            // Display elapsed time in human-readable format to seconds only
            let elapsed =
                humantime::format_duration(Duration::from_secs(start_time.elapsed().as_secs()));
            eprintln!("✅ Downloaded MSVC CRT in {elapsed}.");
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

        let manifest = xwin::manifest::get_manifest(
            ctx,
            &self.xwin_options.xwin_version,
            "release",
            manifest_pb.clone(),
        )?;
        let pkg_manifest =
            xwin::manifest::get_package_manifest(ctx, &manifest, manifest_pb.clone())?;
        manifest_pb.finish_with_message("📥 downloaded");
        Ok(pkg_manifest)
    }

    fn setup_cmake_toolchain(
        &self,
        target: &str,
        xwin_cache_dir: &Path,
        is_static_crt: bool,
    ) -> Result<PathBuf> {
        let cmake_cache_dir = xwin_cache_dir
            .parent()
            .unwrap()
            .join("cmake")
            .join("clang-cl");
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
            "arm64ec" => "ARM64EC",
            _ => target_arch,
        };
        let xwin_arch = match target_arch {
            "i586" | "i686" => "x86",
            _ => target_arch,
        };

        // Due to https://github.com/rust-lang/rust/issues/39016
        // rust always links against non-debug Windows runtime
        // so we must set the runtime library to MultiThreadedDLL or MultiThreaded
        let runtime_library = if is_static_crt {
            "MultiThreaded"
        } else {
            "MultiThreadedDLL"
        };

        let content = format!(
            r#"
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR {processor})

set(CMAKE_C_COMPILER clang-cl CACHE FILEPATH "")
set(CMAKE_CXX_COMPILER clang-cl CACHE FILEPATH "")
set(CMAKE_AR llvm-lib)
set(CMAKE_LINKER lld-link CACHE FILEPATH "")
set(CMAKE_MSVC_RUNTIME_LIBRARY CACHE STRING "{runtime_library}")

set(COMPILE_FLAGS
    --target={target}
    -Wno-unused-command-line-argument
    -fuse-ld=lld-link

    /imsvc {xwin_dir}/crt/include
    /imsvc {xwin_dir}/sdk/include/ucrt
    /imsvc {xwin_dir}/sdk/include/um
    /imsvc {xwin_dir}/sdk/include/shared
    /imsvc {xwin_dir}/sdk/include/winrt)

set(LINK_FLAGS
    /manifest:no

    -libpath:"{xwin_dir}/crt/lib/{xwin_arch}"
    -libpath:"{xwin_dir}/sdk/lib/um/{xwin_arch}"
    -libpath:"{xwin_dir}/sdk/lib/ucrt/{xwin_arch}")

string(REPLACE ";" " " COMPILE_FLAGS "${{COMPILE_FLAGS}}")

set(_CMAKE_C_FLAGS_INITIAL "${{CMAKE_C_FLAGS}}" CACHE STRING "")
set(CMAKE_C_FLAGS "${{_CMAKE_C_FLAGS_INITIAL}} ${{COMPILE_FLAGS}}" CACHE STRING "" FORCE)

set(_CMAKE_CXX_FLAGS_INITIAL "${{CMAKE_CXX_FLAGS}}" CACHE STRING "")
set(CMAKE_CXX_FLAGS "${{_CMAKE_CXX_FLAGS_INITIAL}} ${{COMPILE_FLAGS}} /EHsc" CACHE STRING "" FORCE)

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
            xwin_dir = adjust_canonicalization(xwin_cache_dir.to_slash_lossy().to_string()),
        );
        fs::write(&toolchain_file, content)?;
        Ok(toolchain_file)
    }
}

/// Creates a symlink to the `clang` binary in `cache_dir` and names it
/// `clang-cl`. This is necessary because the `clang-cl` binary doesn't
/// exist on macOS, but `clang` does and can be used as a drop-in
/// replacement for `clang-cl`.
///
/// The `clang` binary is first searched for in `PATH` (skipping the system
/// clang), and if no suitable clang is found, the Xcode clang is tried as
/// a fallback. If no usable clang is found, the function does nothing.
#[cfg(target_os = "macos")]
pub fn setup_clang_cl_symlink(env_path: &OsStr, cache_dir: &Path) -> Result<()> {
    // Try PATH first, but skip system clang
    let clang = which_in("clang", Some(env_path), env::current_dir()?)
        .ok()
        .and_then(|clang| {
            if clang != PathBuf::from("/usr/bin/clang") {
                Some(clang)
            } else {
                None
            }
        });

    // Fall back to xcrun if no suitable clang found in PATH
    let clang = if let Some(clang) = clang {
        clang
    } else {
        // Try Xcode clang as fallback
        match Command::new("xcrun").args(["--find", "clang"]).output() {
            Ok(output) => {
                if output.status.success() {
                    if let Ok(path) = String::from_utf8(output.stdout) {
                        PathBuf::from(path.trim())
                    } else {
                        // No usable clang found
                        return Ok(());
                    }
                } else {
                    // No usable clang found
                    return Ok(());
                }
            }
            _ => {
                // No usable clang found
                return Ok(());
            }
        }
    };

    let symlink = cache_dir.join("clang-cl");
    if symlink.is_symlink() || symlink.is_file() {
        fs::remove_file(&symlink)?;
    }
    fs_err::os::unix::fs::symlink(clang, symlink)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn setup_clang_cl_symlink(env_path: &OsStr, cache_dir: &Path) -> Result<()> {
    if let Ok(clang) = which_in("clang", Some(env_path), env::current_dir()?) {
        #[cfg(windows)]
        {
            let symlink = cache_dir.join("clang-cl.exe");
            if symlink.exists() {
                fs::remove_file(&symlink)?;
            }
            fs_err::os::windows::fs::symlink_file(clang, symlink)?;
        }

        #[cfg(unix)]
        {
            let symlink = cache_dir.join("clang-cl");
            if symlink.exists() {
                fs::remove_file(&symlink)?;
            }
            fs_err::os::unix::fs::symlink(clang, symlink)?;
        }
    }
    Ok(())
}
