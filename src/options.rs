use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser as _},
    Parser, ValueEnum,
};
use fs_err as fs;

/// MSVC cross compiler
#[derive(Clone, Debug, Default, ValueEnum)]
pub enum CrossCompiler {
    /// clang-cl backend
    #[default]
    ClangCl,
    /// clang backend
    Clang,
}

/// common xwin options
#[derive(Clone, Debug, Parser)]
pub struct XWinOptions {
    /// The cross compiler to use
    #[arg(long, env = "XWIN_CROSS_COMPILER", default_value = "clang-cl")]
    pub cross_compiler: CrossCompiler,

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

    /// If specified, this is the version of the SDK that the user wishes to use
    /// instead of defaulting to the latest SDK available in the the manifest
    #[arg(long, env = "XWIN_SDK_VERSION")]
    pub xwin_sdk_version: Option<String>,
    /// If specified, this is the version of the MSVCRT that the user wishes to use
    /// instead of defaulting to the latest MSVCRT available in the the manifest
    #[arg(long, env = "XWIN_CRT_VERSION")]
    pub xwin_crt_version: Option<String>,

    /// Whether to include the Active Template Library (ATL) in the installation
    #[arg(long, env = "XWIN_INCLUDE_ATL")]
    pub xwin_include_atl: bool,

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
            xwin_sdk_version: None,
            xwin_crt_version: None,
            xwin_include_atl: false,
            xwin_include_debug_libs: false,
            xwin_include_debug_symbols: false,
            cross_compiler: CrossCompiler::ClangCl,
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
        let cache_dir = {
            let cache_dir = self.xwin_cache_dir.clone().unwrap_or_else(|| {
                dirs::cache_dir()
                    .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"))
                    .join(env!("CARGO_PKG_NAME"))
            });
            fs::create_dir_all(&cache_dir)?;
            cache_dir.canonicalize()?
        };
        match self.cross_compiler {
            CrossCompiler::ClangCl => {
                let clang_cl = crate::compiler::clang_cl::ClangCl::new(self);
                clang_cl.apply_command_env(manifest_path, cargo, cache_dir, cmd)?;
            }
            CrossCompiler::Clang => {
                let clang = crate::compiler::clang::Clang::new();
                clang.apply_command_env(manifest_path, cargo, cache_dir, cmd)?;
            }
        }
        Ok(())
    }
}
