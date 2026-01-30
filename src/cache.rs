use std::path::PathBuf;

use crate::compiler::clang::Clang;
use crate::compiler::clang_cl::ClangCl;
use crate::options::XWinOptions;
use anyhow::Result;
use clap::{Parser, Subcommand};
use fs_err as fs;

/// Manage xwin cache
#[derive(Debug, Parser)]
pub struct Cache {
    #[command(subcommand)]
    pub subcommand: CacheSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum CacheSubcommand {
    /// Pre-cache xwin (MS CRT) for clang-cl backend
    Xwin(CacheXwin),
    /// Pre-cache windows-msvc-sysroot for clang backend
    WindowsMsvcSysroot(CacheWindowsMsvcSysroot),
}

/// Pre-cache xwin (MS CRT) for clang-cl backend
#[derive(Debug, Parser)]
pub struct CacheXwin {
    #[command(flatten)]
    pub xwin_options: XWinOptions,
}

/// Pre-cache windows-msvc-sysroot for clang backend
#[derive(Debug, Parser)]
pub struct CacheWindowsMsvcSysroot {
    /// Cache directory for windows-msvc-sysroot
    #[arg(long, env = "XWIN_CACHE_DIR")]
    pub cache_dir: Option<PathBuf>,
}

/// Get the default cache directory for cargo-xwin
fn get_default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"))
        .join(env!("CARGO_PKG_NAME"))
}

/// Prepare and canonicalize a cache directory
fn prepare_cache_dir(cache_dir: Option<PathBuf>) -> Result<PathBuf> {
    let cache_dir = cache_dir.unwrap_or_else(get_default_cache_dir);
    fs::create_dir_all(&cache_dir)?;
    cache_dir.canonicalize().map_err(Into::into)
}

/// Prepare the xwin cache subdirectory
pub fn prepare_xwin_cache_dir(base_cache_dir: PathBuf) -> Result<PathBuf> {
    let xwin_cache_dir = base_cache_dir.join("xwin");
    fs::create_dir_all(&xwin_cache_dir)?;
    xwin_cache_dir.canonicalize().map_err(Into::into)
}

impl Cache {
    pub fn execute(self) -> Result<()> {
        match self.subcommand {
            CacheSubcommand::Xwin(xwin) => xwin.execute(),
            CacheSubcommand::WindowsMsvcSysroot(sysroot) => sysroot.execute(),
        }
    }
}

impl CacheXwin {
    pub fn execute(self) -> Result<()> {
        let cache_dir = prepare_cache_dir(self.xwin_options.xwin_cache_dir.clone())?;
        let xwin_cache_dir = prepare_xwin_cache_dir(cache_dir)?;

        println!("📦 Pre-caching Microsoft CRT and Windows SDK...");
        println!("📁 Cache directory: {}", xwin_cache_dir.display());

        let clang_cl = ClangCl::new(&self.xwin_options);
        clang_cl.setup_msvc_crt(xwin_cache_dir)?;

        println!("✅ xwin cache setup completed successfully!");
        Ok(())
    }
}

impl CacheWindowsMsvcSysroot {
    pub fn execute(self) -> Result<()> {
        let cache_dir = prepare_cache_dir(self.cache_dir.clone())?;

        println!("📦 Pre-caching windows-msvc-sysroot...");
        println!("📁 Cache directory: {}", cache_dir.display());

        let clang = Clang::new();
        let sysroot_dir = clang.setup_msvc_sysroot(cache_dir)?;

        println!("✅ windows-msvc-sysroot cache setup completed successfully!");
        println!("📁 Sysroot location: {}", sysroot_dir.display());
        Ok(())
    }
}
