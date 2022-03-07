use std::path::PathBuf;

use clap::Parser;

/// common cargo options
#[derive(Clone, Debug, Default, Parser)]
pub struct CargoOptions {
    /// Do not print cargo log messages
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Number of parallel jobs, defaults to # of CPUs
    #[clap(short = 'j', long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Build artifacts in release mode, with optimizations
    #[clap(short = 'r', long)]
    pub release: bool,

    /// Build artifacts with the specified Cargo profile
    #[clap(long, value_name = "PROFILE-NAME")]
    pub profile: Option<String>,

    /// Space or comma separated list of features to activate
    #[clap(long, multiple_values = true)]
    pub features: Vec<String>,

    /// Activate all available features
    #[clap(long)]
    pub all_features: bool,

    /// Do not activate the `default` feature
    #[clap(long)]
    pub no_default_features: bool,

    /// Build for the target triple
    #[clap(long, value_name = "TRIPLE", env = "CARGO_BUILD_TARGET")]
    pub target: Option<String>,

    /// Directory for all generated artifacts
    #[clap(long, value_name = "DIRECTORY", parse(from_os_str))]
    pub target_dir: Option<PathBuf>,

    /// Path to Cargo.toml
    #[clap(long, value_name = "PATH", parse(from_os_str))]
    pub manifest_path: Option<PathBuf>,

    /// Ignore `rust-version` specification in packages
    #[clap(long)]
    pub ignore_rust_version: bool,

    /// Error format
    #[clap(long, value_name = "FMT", multiple_values = true)]
    pub message_format: Vec<String>,

    /// Output build graph in JSON (unstable)
    #[clap(long)]
    pub unit_graph: bool,

    /// Use verbose output (-vv very verbose/build.rs output)
    #[clap(short = 'v', long, parse(from_occurrences), max_occurrences = 2)]
    pub verbose: usize,

    /// Coloring: auto, always, never
    #[clap(long, value_name = "WHEN")]
    pub color: Option<String>,

    /// Require Cargo.lock and cache are up to date
    #[clap(long)]
    pub frozen: bool,

    /// Require Cargo.lock is up to date
    #[clap(long)]
    pub locked: bool,

    /// Run without accessing the network
    #[clap(long)]
    pub offline: bool,

    /// Override a configuration value (unstable)
    #[clap(long, value_name = "KEY=VALUE", multiple_values = true)]
    pub config: Vec<String>,

    /// Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details
    #[clap(short = 'Z', value_name = "FLAG", multiple_values = true)]
    pub unstable_flags: Vec<String>,
}

/// common xwin options
#[derive(Clone, Debug, Default, Parser)]
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
