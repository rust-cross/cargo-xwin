use std::path::PathBuf;

use clap::Parser;

/// common xwin options
#[derive(Clone, Debug, Parser)]
pub struct XWinOptions {
    /// xwin cache directory
    #[clap(long, parse(from_os_str), env = "XWIN_CACHE_DIR", hide = true)]
    pub xwin_cache_dir: Option<PathBuf>,

    /// The architectures to include in CRT/SDK
    #[clap(
        long,
        env = "XWIN_ARCH",
        possible_values(&["x86", "x86_64", "aarch", "aarch64"]),
        use_value_delimiter = true,
        default_value = "x86_64,aarch64",
        hide = true,
    )]
    pub xwin_arch: Vec<xwin::Arch>,

    /// The variants to include
    #[clap(
        long,
        env = "XWIN_VARIANT",
        possible_values(&["desktop", "onecore", /*"store",*/ "spectre"]),
        use_value_delimiter = true,
        default_value = "desktop",
        hide = true,
    )]
    pub xwin_variant: Vec<xwin::Variant>,

    /// The version to retrieve, can either be a major version of 15 or 16, or
    /// a "<major>.<minor>" version.
    #[clap(long, env = "XWIN_VERSION", default_value = "16", hide = true)]
    pub xwin_version: String,
}

impl Default for XWinOptions {
    fn default() -> Self {
        Self {
            xwin_cache_dir: None,
            xwin_arch: vec![xwin::Arch::X86_64, xwin::Arch::Aarch64],
            xwin_variant: vec![xwin::Variant::Desktop],
            xwin_version: "16".to_string(),
        }
    }
}
