use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs_err as fs;
use path_slash::PathExt;
use serde::Deserialize;

use crate::compiler::common::{
    adjust_canonicalization, default_build_target_from_config, get_rustflags, http_agent,
    setup_cmake_env, setup_env_path, setup_llvm_tools, setup_target_compiler_and_linker_env,
};

const MSVC_SYSROOT_REPOSITORY: &str = "trcrsired/windows-msvc-sysroot";
const MSVC_SYSROOT_ASSET_NAME: &str = "windows-msvc-sysroot.tar.xz";
const FALLBACK_DOWNLOAD_URL: &str = "https://github.com/trcrsired/windows-msvc-sysroot/releases/download/2025-01-22/windows-msvc-sysroot.tar.xz";

#[derive(Debug)]
pub struct Clang;

impl Clang {
    pub fn new() -> Self {
        Clang
    }

    pub fn apply_command_env(
        &self,
        manifest_path: Option<&Path>,
        cargo: &cargo_options::CommonOptions,
        cache_dir: PathBuf,
        cmd: &mut Command,
    ) -> Result<()> {
        let env_path = setup_env_path(&cache_dir)?;
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
                let msvc_sysroot_dir = self
                    .setup_msvc_sysroot(cache_dir.clone())
                    .context("Failed to setup MSVC sysroot")?;
                // x86_64-pc-windows-msvc -> x86_64-windows-msvc
                let target_no_vendor = target.replace("-pc-", "-");
                let target_unknown_vendor = target.replace("-pc-", "-unknown-");
                let env_target = target.to_lowercase().replace('-', "_");

                setup_llvm_tools(&env_path, &cache_dir).context("Failed to setup LLVM tools")?;
                setup_target_compiler_and_linker_env(cmd, &env_target, "clang");

                let user_set_c_flags = env::var("CFLAGS").unwrap_or_default();
                let user_set_cxx_flags = env::var("CXXFLAGS").unwrap_or_default();
                let sysroot_dir =
                    adjust_canonicalization(msvc_sysroot_dir.to_slash_lossy().to_string());
                let clang_flags = format!(
                    "--target={target_no_vendor} -fuse-ld=lld-link -I{dir}/include -I{dir}/include/c++/stl -I{dir}/include/__msvc_vcruntime_intrinsics -L{dir}/lib/{target_unknown_vendor}",
                    dir = sysroot_dir,
                );
                cmd.env(
                    format!("CFLAGS_{env_target}"),
                    format!("{clang_flags} {user_set_c_flags}",),
                );
                cmd.env(
                    format!("CXXFLAGS_{env_target}"),
                    format!("{clang_flags} {user_set_cxx_flags}",),
                );
                cmd.env(
                    format!("BINDGEN_EXTRA_CLANG_ARGS_{env_target}"),
                    format!("-I{dir}/include -I{dir}/include/c++/stl -I{dir}/include/__msvc_vcruntime_intrinsics", dir = sysroot_dir),
                );
                cmd.env(
                    "RCFLAGS",
                    format!("-I{dir}/include -I{dir}/include/c++/stl -I{dir}/include/__msvc_vcruntime_intrinsics", dir = sysroot_dir),
                );

                let mut rustflags = get_rustflags(&workdir, target)?.unwrap_or_default();
                rustflags.flags.extend([
                    "-C".to_string(),
                    "linker-flavor=lld-link".to_string(),
                    "-C".to_string(),
                    "link-arg=-defaultlib:oldnames".to_string(),
                ]);
                rustflags.push(format!(
                    "-Lnative={dir}/lib/{target_unknown_vendor}",
                    dir = sysroot_dir,
                ));
                cmd.env("CARGO_ENCODED_RUSTFLAGS", rustflags.encode()?);
                cmd.env("PATH", &env_path);

                // CMake support
                let cmake_toolchain = self
                    .setup_cmake_toolchain(target, &sysroot_dir, &cache_dir)
                    .with_context(|| format!("Failed to setup CMake toolchain for {}", target))?;
                setup_cmake_env(cmd, target, cmake_toolchain);
            }
        }
        Ok(())
    }

    /// Download and unpack the latest MSVC sysroot from GitHub Releases.
    ///
    /// If the sysroot is already downloaded and unpacked, it will be reused.
    /// The sysroot will be stored in `<cache_dir>/windows-msvc-sysroot`.
    /// A file named `DONE` will be created in the same directory with the
    /// download URL as its content.
    ///
    /// The environment variable `XWIN_MSVC_SYSROOT_DOWNLOAD_URL` can be used
    /// to override the download URL.
    fn setup_msvc_sysroot(&self, cache_dir: PathBuf) -> Result<PathBuf> {
        let msvc_sysroot_dir = cache_dir.join("windows-msvc-sysroot");
        let done_mark_file = msvc_sysroot_dir.join("DONE");
        if msvc_sysroot_dir.is_dir() {
            if done_mark_file.is_file() {
                // Already downloaded and unpacked
                return Ok(msvc_sysroot_dir);
            } else {
                // Download again
                fs::remove_dir_all(&msvc_sysroot_dir)
                    .context("Failed to remove existing msvc sysroot")?;
            }
        }

        let agent = http_agent()?;
        // fetch release info to get download url
        let download_url = self
            .get_latest_msvc_sysroot_download_url(agent.clone())
            .unwrap_or_else(|_| FALLBACK_DOWNLOAD_URL.to_string());
        self.download_msvc_sysroot(&cache_dir, agent, &download_url)
            .context("Failed to unpack msvc sysroot")?;
        fs::write(done_mark_file, download_url)?;
        Ok(msvc_sysroot_dir)
    }

    /// Retrieves the latest MSVC sysroot download URL from GitHub Releases.
    ///
    /// The function uses the `ureq` agent to make an HTTP GET request to the GitHub API. If a
    /// `GITHUB_TOKEN` environment variable is present, it includes it as a Bearer token for
    /// authentication.
    ///
    fn get_latest_msvc_sysroot_download_url(&self, agent: ureq::Agent) -> Result<String> {
        if let Ok(url) = env::var("XWIN_MSVC_SYSROOT_DOWNLOAD_URL") {
            return Ok(url);
        }
        let mut request = agent
            .get(&format!(
                "https://api.github.com/repos/{}/releases/latest",
                MSVC_SYSROOT_REPOSITORY
            ))
            .set("X-GitHub-Api-Version", "2022-11-28");
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let response = request.call().context("Failed to get GitHub release")?;
        let release: GitHubRelease = response
            .into_json()
            .context("Failed to deserialize GitHub release")?;
        let asset = release
            .assets
            .iter()
            .find(|x| x.name == MSVC_SYSROOT_ASSET_NAME)
            .with_context(|| {
                format!("Failed to find {MSVC_SYSROOT_ASSET_NAME} in GitHub release")
            })?;
        let download_url = asset.browser_download_url.clone();
        Ok(download_url)
    }

    fn download_msvc_sysroot_once(
        &self,
        cache_dir: &Path,
        agent: &ureq::Agent,
        download_url: &str,
    ) -> Result<()> {
        use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
        use xz2::read::XzDecoder;

        let response = agent.get(download_url).call()?;
        let len = response
            .header("content-length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let pb = ProgressBar::new(len);
        pb.set_draw_target(ProgressDrawTarget::stdout());
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} {prefix:.bold} [{elapsed}] {wide_bar:.green} {bytes}/{total_bytes} {msg}",
                )?
                .progress_chars("=> "),
        );
        pb.set_prefix("sysroot");
        pb.set_message("📥 downloading");
        if pb.is_hidden() {
            eprintln!("📥 Downloading MSVC sysroot...");
        }
        let start_time = Instant::now();
        let reader = pb.wrap_read(response.into_reader());
        let tar = XzDecoder::new(reader);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(cache_dir)?;
        pb.finish_with_message("Download completed");
        if pb.is_hidden() {
            // Display elapsed time in human-readable format to seconds only
            let elapsed =
                humantime::format_duration(Duration::from_secs(start_time.elapsed().as_secs()));
            eprintln!("✅ Downloaded MSVC sysroot in {elapsed}.");
        }
        Ok(())
    }

    fn download_msvc_sysroot(
        &self,
        cache_dir: &Path,
        agent: ureq::Agent,
        download_url: &str,
    ) -> Result<()> {
        use std::time::Duration;

        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        let mut last_error = None;

        while retry_count < MAX_RETRIES {
            if retry_count > 0 {
                let wait_time = Duration::from_secs(2u64.pow(retry_count - 1));
                std::thread::sleep(wait_time);
                eprintln!(
                    "Retrying download (attempt {}/{})",
                    retry_count + 1,
                    MAX_RETRIES
                );
            }

            match self.download_msvc_sysroot_once(cache_dir, &agent, download_url) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    retry_count += 1;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to download MSVC sysroot")))
    }

    fn setup_cmake_toolchain(
        &self,
        target: &str,
        sysroot_dir: &str,
        cache_dir: &Path,
    ) -> Result<PathBuf> {
        // x86_64-pc-windows-msvc -> x86_64-windows-msvc
        let target_no_vendor = target.replace("-pc-", "-");
        let target_unknown_vendor = target.replace("-pc-", "-unknown-");
        let cmake_cache_dir = cache_dir.join("cmake").join("clang");
        fs::create_dir_all(&cmake_cache_dir)?;

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

        let content = format!(
            r#"
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR {processor})

set(CMAKE_C_COMPILER clang CACHE FILEPATH "")
set(CMAKE_CXX_COMPILER clang++ CACHE FILEPATH "")
set(CMAKE_LINKER lld-link CACHE FILEPATH "")
set(CMAKE_RC_COMPILER llvm-rc CACHE FILEPATH "")
set(CMAKE_C_COMPILER_TARGET {target} CACHE STRING "")
set(CMAKE_CXX_COMPILER_TARGET {target} CACHE STRING "")

set(COMPILE_FLAGS
    --target={target_no_vendor}
    -fuse-ld=lld-link
    -I{dir}/include
    -I{dir}/include/c++/stl
    -I{dir}/include/__msvc_vcruntime_intrinsics)

set(LINK_FLAGS
    /manifest:no
    -libpath:"{dir}/lib/{target_unknown_vendor}")
        "#,
            dir = sysroot_dir,
        );
        fs::write(&toolchain_file, content)?;
        Ok(toolchain_file)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseAsset {
    browser_download_url: String,
    name: String,
}
