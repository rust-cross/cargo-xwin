use std::collections::HashSet;
use std::convert::TryInto;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use anyhow::{Context, Result};
use clap::Parser;
use fs_err as fs;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use path_slash::PathExt;
use xwin::util::ProgressTarget;

use crate::common::{CargoOptions, XWinOptions};

/// Compile a local package and all of its dependencies
#[derive(Clone, Debug, Default, Parser)]
#[clap(setting = clap::AppSettings::DeriveDisplayOrder, after_help = "Run `cargo help build` for more detailed information.")]
pub struct Build {
    #[clap(flatten)]
    pub cargo: CargoOptions,

    /// Package to build (see `cargo help pkgid`)
    #[clap(
        short = 'p',
        long = "package",
        value_name = "SPEC",
        multiple_values = true
    )]
    pub packages: Vec<String>,

    /// Build all packages in the workspace
    #[clap(long)]
    pub workspace: bool,

    /// Exclude packages from the build
    #[clap(long, value_name = "SPEC", multiple_values = true)]
    pub exclude: Vec<String>,

    /// Alias for workspace (deprecated)
    #[clap(long)]
    pub all: bool,

    /// Build only this package's library
    #[clap(long)]
    pub lib: bool,

    /// Build only the specified binary
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub bin: Vec<String>,

    /// Build all binaries
    #[clap(long)]
    pub bins: bool,

    /// Build only the specified example
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub example: Vec<String>,

    /// Build all examples
    #[clap(long)]
    pub examples: bool,

    /// Build only the specified test target
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub test: Vec<String>,

    /// Build all tests
    #[clap(long)]
    pub tests: bool,

    /// Build only the specified bench target
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub bench: Vec<String>,

    /// Build all benches
    #[clap(long)]
    pub benches: bool,

    /// Build all targets
    #[clap(long)]
    pub all_targets: bool,

    /// Copy final artifacts to this directory (unstable)
    #[clap(long, value_name = "PATH", parse(from_os_str))]
    pub out_dir: Option<PathBuf>,

    /// Output the build plan in JSON (unstable)
    #[clap(long)]
    pub build_plan: bool,

    /// Outputs a future incompatibility report at the end of the build (unstable)
    #[clap(long)]
    pub future_incompat_report: bool,

    #[clap(flatten)]
    pub xwin: XWinOptions,
}

impl Build {
    /// Execute `cargo build` command
    pub fn execute(&self) -> Result<()> {
        let mut build = self.build_command("build")?;
        let mut child = build.spawn().context("Failed to run cargo build")?;
        let status = child.wait().expect("Failed to wait on cargo build process");
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }

    /// Generate cargo subcommand
    pub fn build_command(&self, subcommand: &str) -> Result<Command> {
        let xwin_cache_dir = self.xwin.xwin_cache_dir.clone().unwrap_or_else(|| {
            dirs::cache_dir()
                // If the really is no cache dir, cwd will also do
                .unwrap_or_else(|| env::current_dir().expect("Failed to get current dir"))
                .join(env!("CARGO_PKG_NAME"))
                .join("xwin")
        });
        fs::create_dir_all(&xwin_cache_dir)?;
        let xwin_cache_dir = xwin_cache_dir.canonicalize()?;

        let mut build = Command::new("cargo");
        build.arg(subcommand);

        // collect cargo build arguments
        if self.cargo.quiet {
            build.arg("--quiet");
        }
        for pkg in &self.packages {
            build.arg("--package").arg(pkg);
        }
        if self.workspace {
            build.arg("--workspace");
        }
        for item in &self.exclude {
            build.arg("--exclude").arg(item);
        }
        if self.all {
            build.arg("--all");
        }
        if let Some(jobs) = self.cargo.jobs {
            build.arg("--jobs").arg(jobs.to_string());
        }
        if self.lib {
            build.arg("--lib");
        }
        for bin in &self.bin {
            build.arg("--bin").arg(bin);
        }
        if self.bins {
            build.arg("--bins");
        }
        for example in &self.example {
            build.arg("--example").arg(example);
        }
        if self.examples {
            build.arg("--examples");
        }
        for test in &self.test {
            build.arg("--test").arg(test);
        }
        if self.tests {
            build.arg("--tests");
        }
        for bench in &self.bench {
            build.arg("--bench").arg(bench);
        }
        if self.benches {
            build.arg("--benches");
        }
        if self.all_targets {
            build.arg("--all-targets");
        }
        if self.cargo.release {
            build.arg("--release");
        }
        if let Some(profile) = self.cargo.profile.as_ref() {
            build.arg("--profile").arg(profile);
        }
        for feature in &self.cargo.features {
            build.arg("--features").arg(feature);
        }
        if self.cargo.all_features {
            build.arg("--all-features");
        }
        if self.cargo.no_default_features {
            build.arg("--no-default-features");
        }
        if let Some(target) = self.cargo.target.as_ref() {
            build.arg("--target").arg(target);
        }
        if let Some(dir) = self.cargo.target_dir.as_ref() {
            build.arg("--target-dir").arg(dir);
        }
        if let Some(dir) = self.out_dir.as_ref() {
            build.arg("--out-dir").arg(dir);
        }
        if let Some(path) = self.cargo.manifest_path.as_ref() {
            build.arg("--manifest-path").arg(path);
        }
        if self.cargo.ignore_rust_version {
            build.arg("--ignore-rust-version");
        }
        for fmt in &self.cargo.message_format {
            build.arg("--message-format").arg(fmt);
        }
        if self.build_plan {
            build.arg("--build-plan");
        }
        if self.cargo.unit_graph {
            build.arg("--unit-graph");
        }
        if self.future_incompat_report {
            build.arg("--future-incompat-report");
        }
        if self.cargo.verbose > 0 {
            build.arg(format!("-{}", "v".repeat(self.cargo.verbose)));
        }
        if let Some(color) = self.cargo.color.as_ref() {
            build.arg("--color").arg(color);
        }
        if self.cargo.frozen {
            build.arg("--frozen");
        }
        if self.cargo.locked {
            build.arg("--locked");
        }
        if self.cargo.offline {
            build.arg("--offline");
        }
        for config in &self.cargo.config {
            build.arg("--config").arg(config);
        }
        for flag in &self.cargo.unstable_flags {
            build.arg("-Z").arg(flag);
        }

        if let Some(target) = self.cargo.target.as_ref() {
            if target.contains("msvc") {
                self.setup_msvc_crt(xwin_cache_dir.clone())?;
                let env_target = target.to_lowercase().replace('-', "_");
                build.env("TARGET_CC", format!("clang-cl --target={}", target));
                build.env("TARGET_CXX", format!("clang-cl --target={}", target));
                build.env(
                    format!("CC_{}", env_target),
                    format!("clang-cl --target={}", target),
                );
                build.env(
                    format!("CXX_{}", env_target),
                    format!("clang-cl --target={}", target),
                );
                build.env("TARGET_AR", "llvm-lib");
                build.env(format!("AR_{}", env_target), "llvm-lib");
                build.env(
                    format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
                    "lld-link",
                );

                let cl_flags = format!(
                    "-Wno-unused-command-line-argument -fuse-ld=lld-link /imsvc{dir}/crt/include /imsvc{dir}/sdk/include/ucrt /imsvc{dir}/sdk/include/um /imsvc{dir}/sdk/include/shared",
                    dir = xwin_cache_dir.display()
                );
                build.env("CL_FLAGS", &cl_flags);
                build.env(format!("CFLAGS_{}", env_target), &cl_flags);
                build.env(format!("CXXFLAGS_{}", env_target), &cl_flags);

                let target_arch = target
                    .split_once('-')
                    .map(|(x, _)| x)
                    .context("invalid target triple")?;
                let xwin_arch = match target_arch {
                    "i586" | "i686" => "x86",
                    _ => target_arch,
                };

                let mut rustflags = env::var_os("RUSTFLAGS").unwrap_or_default();
                rustflags.push(format!(
                    " -Lnative={dir}/crt/lib/{arch} -Lnative={dir}/sdk/lib/um/{arch} -Lnative={dir}/sdk/lib/ucrt/{arch}",
                    dir = xwin_cache_dir.display(),
                    arch = xwin_arch,
                ));
                build.env("RUSTFLAGS", rustflags);

                #[cfg(target_os = "macos")]
                if let Ok(path) = env::var("PATH") {
                    let mut new_path = path.clone();
                    if cfg!(target_arch = "x86_64") && !path.contains("/usr/local/opt/llvm/bin") {
                        new_path.push_str(":/usr/local/opt/llvm/bin");
                    } else if cfg!(target_arch = "aarch64")
                        && !path.contains("/opt/homebrew/opt/llvm/bin")
                    {
                        new_path.push_str(":/opt/homebrew/opt/llvm/bin");
                    }
                    build.env("PATH", new_path);
                }

                // CMake support
                let cmake_toolchain = self.setup_cmake_toolchain(target, &xwin_cache_dir)?;
                build
                    .env("CMAKE_GENERATOR", "Ninja")
                    .env("CMAKE_SYSTEM_NAME", "Windows")
                    .env(
                        format!("CMAKE_TOOLCHAIN_FILE_{}", env_target),
                        cmake_toolchain,
                    );
            }
        }

        Ok(build)
    }

    fn setup_msvc_crt(&self, cache_dir: PathBuf) -> Result<()> {
        let done_mark_file = cache_dir.join("DONE");
        let xwin_arches: HashSet<_> = self
            .xwin
            .xwin_arch
            .iter()
            .map(|x| x.as_str().to_string())
            .collect();
        let mut downloaded_arches = HashSet::new();
        if let Ok(content) = fs::read_to_string(&done_mark_file) {
            for arch in content.trim().split_whitespace() {
                downloaded_arches.insert(arch.to_string());
            }
        }
        if xwin_arches.difference(&downloaded_arches).next().is_none() {
            return Ok(());
        }

        let draw_target = ProgressTarget::Stdout;

        let xwin_dir = adjust_canonicalization(cache_dir.display().to_string());
        let ctx = xwin::Ctx::with_dir(xwin::PathBuf::from(xwin_dir), draw_target)?;
        let ctx = std::sync::Arc::new(ctx);
        let pkg_manifest = self.load_manifest(&ctx, draw_target)?;

        let arches = self
            .xwin
            .xwin_arch
            .iter()
            .fold(0, |acc, arch| acc | *arch as u32);
        let variants = self
            .xwin
            .xwin_variant
            .iter()
            .fold(0, |acc, var| acc | *var as u32);
        let pruned = xwin::prune_pkg_list(&pkg_manifest, arches, variants)?;
        let op = xwin::Ops::Splat(xwin::SplatConfig {
            include_debug_libs: false,
            include_debug_symbols: false,
            enable_symlinks: !cfg!(target_os = "macos"),
            preserve_ms_arch_notation: false,
            copy: false,
            output: cache_dir.clone().try_into()?,
        });
        let pkgs = pkg_manifest.packages;

        let mp = MultiProgress::with_draw_target(draw_target.into());
        let work_items: Vec<_> = pruned
        .into_iter()
        .map(|pay| {
            let prefix = match pay.kind {
                xwin::PayloadKind::CrtHeaders => "CRT.headers".to_owned(),
                xwin::PayloadKind::CrtLibs => {
                    format!(
                        "CRT.libs.{}.{}",
                        pay.target_arch.map(|ta| ta.as_str()).unwrap_or("all"),
                        pay.variant.map(|v| v.as_str()).unwrap_or("none")
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
                ProgressBar::with_draw_target(0, draw_target.into()).with_prefix(prefix).with_style(
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
        ctx.execute(pkgs, work_items, arches, variants, op)?;

        let downloaded_arches: Vec<_> = self
            .xwin
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
        let manifest_pb = ProgressBar::with_draw_target(0, dt.into())
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
            &self.xwin.xwin_version,
            "release",
            manifest_pb.clone(),
        )?;
        let pkg_manifest =
            xwin::manifest::get_package_manifest(ctx, &manifest, manifest_pb.clone())?;
        manifest_pb.finish_with_message("📥 downloaded");
        Ok(pkg_manifest)
    }

    fn setup_cmake_toolchain(&self, target: &str, xwin_cache_dir: &Path) -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| env::current_dir().expect("Failed to get current dir"))
            .join(env!("CARGO_PKG_NAME"));
        let cmake = cache_dir.join("cmake");
        fs::create_dir_all(&cmake)?;

        let override_file = cmake.join("override.cmake");
        fs::write(override_file, include_bytes!("override.cmake"))?;

        let toolchain_file = cmake.join(format!("{}-toolchain.cmake", target));
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
            xwin_dir = adjust_canonicalization(xwin_cache_dir.to_slash_lossy()),
            xwin_arch = xwin_arch,
        );
        fs::write(&toolchain_file, &content)?;
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
    if p.starts_with(VERBATIM_PREFIX) {
        p[VERBATIM_PREFIX.len()..].to_string()
    } else {
        p
    }
}
