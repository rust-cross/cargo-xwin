use std::env;
use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use crate::Build;

/// Run a binary or example of the local package
#[derive(Clone, Debug, Default, Parser)]
#[clap(
    setting = clap::AppSettings::DeriveDisplayOrder,
    trailing_var_arg = true,
    after_help = "Run `cargo help run` for more detailed information.")
]
pub struct Run {
    /// Do not print cargo log messages
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Package to run (see `cargo help pkgid`)
    #[clap(
        short = 'p',
        long = "package",
        value_name = "SPEC",
        multiple_values = true
    )]
    pub packages: Vec<String>,

    /// Number of parallel jobs, defaults to # of CPUs
    #[clap(short = 'j', long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Run the specified binary
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub bin: Vec<String>,

    /// Run the specified example
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub example: Vec<String>,

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

    /// Arguments for the binary to run
    #[clap(takes_value = true, multiple_values = true)]
    pub args: Vec<String>,
}

impl Run {
    /// Execute `cargo run` command
    pub fn execute(&self) -> Result<()> {
        let build = Build {
            quiet: self.quiet,
            packages: self.packages.clone(),
            jobs: self.jobs,
            bin: self.bin.clone(),
            example: self.example.clone(),
            release: self.release,
            profile: self.profile.clone(),
            features: self.features.clone(),
            all_features: self.all_features,
            no_default_features: self.no_default_features,
            target: self.target.clone(),
            target_dir: self.target_dir.clone(),
            manifest_path: self.manifest_path.clone(),
            ignore_rust_version: self.ignore_rust_version,
            message_format: self.message_format.clone(),
            unit_graph: self.unit_graph,
            verbose: self.verbose,
            color: self.color.clone(),
            frozen: self.frozen,
            locked: self.locked,
            offline: self.offline,
            config: self.config.clone(),
            unstable_flags: self.unstable_flags.clone(),
            xwin_cache_dir: self.xwin_cache_dir.clone(),
            xwin_arch: self.xwin_arch.clone(),
            xwin_variant: self.xwin_variant.clone(),
            xwin_version: self.xwin_version.clone(),
            ..Default::default()
        };
        let mut run = build.build_command("run")?;
        if !self.args.is_empty() {
            run.arg("--");
            run.args(&self.args);
        }

        if let Some(target) = self.target.as_ref() {
            if target.contains("msvc") {
                if env::var_os("WINEDEBUG").is_none() {
                    run.env("WINEDEBUG", "-all");
                }
                let env_target = target.to_uppercase().replace('-', "_");
                let runner_env = format!("CARGO_TARGET_{}_RUNNER", env_target);
                if env::var_os(&runner_env).is_none() {
                    run.env(runner_env, "wine");
                }
            }
        }

        let mut child = run.spawn().context("Failed to run cargo run")?;
        let status = child.wait().expect("Failed to wait on cargo run process");
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}
