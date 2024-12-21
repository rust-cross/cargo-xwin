use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use fs_err as fs;
use path_slash::PathExt;
use serde::Deserialize;

use crate::common::{
    adjust_canonicalization, default_build_target_from_config, get_rustflags, http_agent,
    symlink_llvm_tool, XWinOptions,
};

const MSVC_SYSROOT_REPOSITORY: &str = "trcrsired/windows-msvc-sysroot";
const MSVC_SYSROOT_ASSET_NAME: &str = "windows-msvc-sysroot.tar.xz";

#[derive(Debug)]
pub struct Clang<'a> {
    xwin_options: &'a XWinOptions,
}

impl<'a> Clang<'a> {
    pub fn new(xwin_options: &'a XWinOptions) -> Self {
        Self { xwin_options }
    }

    pub fn apply_command_env(
        &self,
        manifest_path: Option<&Path>,
        cargo: &cargo_options::CommonOptions,
        cmd: &mut Command,
    ) -> Result<()> {
        let cache_dir = self.xwin_options.xwin_cache_dir.clone().unwrap_or_else(|| {
            dirs::cache_dir()
                // If the really is no cache dir, cwd will also do
                .unwrap_or_else(|| env::current_dir().expect("Failed to get current dir"))
                .join(env!("CARGO_PKG_NAME"))
        });
        fs::create_dir_all(&cache_dir)?;
        let cache_dir = cache_dir.canonicalize()?;

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
        env_paths.push(cache_dir.clone());

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
                let msvc_sysroot_dir = self.setup_msvc_sysroot(cache_dir.clone())?;
                // x86_64-pc-windows-msvc -> x86_64-windows-msvc
                let target_no_vendor = target.replace("-pc-", "-");
                let target_unknown_vendor = target.replace("-pc-", "-unknown-");
                let env_target = target.to_lowercase().replace('-', "_");

                symlink_llvm_tool("rust-lld", "lld-link", env_path.clone(), &cache_dir)?;
                symlink_llvm_tool("llvm-ar", "llvm-lib", env_path.clone(), &cache_dir)?;
                symlink_llvm_tool("llvm-ar", "llvm-dlltool", env_path.clone(), &cache_dir)?;

                cmd.env("TARGET_CC", "clang");
                cmd.env("TARGET_CXX", "clang++");
                cmd.env(format!("CC_{}", env_target), "clang");
                cmd.env(format!("CXX_{}", env_target), "clang++");
                cmd.env("TARGET_AR", "llvm-lib");
                cmd.env(format!("AR_{}", env_target), "llvm-lib");
                cmd.env(
                    format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
                    "lld-link",
                );

                let user_set_c_flags = env::var("CFLAGS").unwrap_or_default();
                let user_set_cxx_flags = env::var("CXXFLAGS").unwrap_or_default();
                let sysroot_dir =
                    adjust_canonicalization(msvc_sysroot_dir.to_slash_lossy().to_string());
                let clang_flags = format!(
                    "--target={target_no_vendor} -fuse-ld=lld-link -I{dir}/include -I{dir}/include/c++/stl -L{dir}/lib/{target_unknown_vendor}",
                    dir = sysroot_dir,
                );
                cmd.env(
                    format!("CFLAGS_{env_target}"),
                    &format!("{clang_flags} {user_set_c_flags}",),
                );
                cmd.env(
                    format!("CXXFLAGS_{env_target}"),
                    &format!("{clang_flags} {user_set_cxx_flags}",),
                );
                cmd.env(
                    format!("BINDGEN_EXTRA_CLANG_ARGS_{env_target}"),
                    format!("-I{dir}/include -I{dir}/include/c++/stl", dir = sysroot_dir),
                );
                cmd.env(
                    "RCFLAGS",
                    format!("-I{dir}/include -I{dir}/include/c++/stl", dir = sysroot_dir),
                );

                let mut rustflags = get_rustflags(&workdir, target)?.unwrap_or_default();
                rustflags
                    .flags
                    .extend(["-C".to_string(), "linker-flavor=lld-link".to_string()]);
                rustflags.push(format!(
                    "-Lnative={dir}/lib/{target_unknown_vendor}",
                    dir = sysroot_dir,
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

                // TODO: CMake support
            }
        }
        Ok(())
    }

    fn setup_msvc_sysroot(&self, cache_dir: PathBuf) -> Result<PathBuf> {
        let msvc_sysroot_dir = cache_dir.join("windows-msvc-sysroot");
        if msvc_sysroot_dir.is_dir() {
            // Already downloaded and unpacked
            return Ok(msvc_sysroot_dir);
        }

        let agent = http_agent()?;
        let gh_token = env::var("GITHUB_TOKEN").ok();
        // fetch release info to get download url
        let mut request = agent
            .get(&format!(
                "https://api.github.com/repos/{}/releases/latest",
                MSVC_SYSROOT_REPOSITORY
            ))
            .set("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &gh_token {
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
        let download_url = &asset.browser_download_url;
        self.download_msvc_sysroot(&cache_dir, agent, download_url)
            .context("Failed to unpack msvc sysroot")?;
        Ok(msvc_sysroot_dir)
    }

    fn download_msvc_sysroot(
        &self,
        cache_dir: &Path,
        agent: ureq::Agent,
        download_url: &str,
    ) -> Result<()> {
        use xz2::read::XzDecoder;

        let response = agent.get(download_url).call()?;
        let tar = XzDecoder::new(response.into_reader());
        let mut archive = tar::Archive::new(tar);
        archive.unpack(cache_dir)?;
        Ok(())
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
