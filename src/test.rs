use std::env;
use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use crate::common::XWinOptions;
use crate::Build;

/// Execute all unit and integration tests and build examples of a local package
#[derive(Clone, Debug, Default, Parser)]
#[clap(
    setting = clap::AppSettings::DeriveDisplayOrder,
    trailing_var_arg = true,
    after_help = "Run `cargo help test` for more detailed information.\nRun `cargo test -- --help` for test binary options.")
]
pub struct Test {
    /// Do not print cargo log messages
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Package to build (see `cargo help pkgid`)
    #[clap(
        short = 'p',
        long = "package",
        value_name = "SPEC",
        multiple_values = true
    )]
    pub packages: Vec<String>,

    /// Test all packages in the workspace
    #[clap(long)]
    pub workspace: bool,

    /// Exclude packages from the build
    #[clap(long, value_name = "SPEC", multiple_values = true)]
    pub exclude: Vec<String>,

    /// Alias for workspace (deprecated)
    #[clap(long)]
    pub all: bool,

    /// Number of parallel jobs, defaults to # of CPUs
    #[clap(short = 'j', long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Test only this package's library
    #[clap(long)]
    pub lib: bool,

    /// Test only the specified binary
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub bin: Vec<String>,

    /// Test all binaries
    #[clap(long)]
    pub bins: bool,

    /// Test only the specified example
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub example: Vec<String>,

    /// Test all examples
    #[clap(long)]
    pub examples: bool,

    /// Test only the specified test target
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub test: Vec<String>,

    /// Test all tests
    #[clap(long)]
    pub tests: bool,

    /// Test only the specified bench target
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub bench: Vec<String>,

    /// Test all benches
    #[clap(long)]
    pub benches: bool,

    /// Test all targets
    #[clap(long)]
    pub all_targets: bool,

    /// Test only this library's documentation
    #[clap(long)]
    pub doc: bool,

    /// Compile, but don't run tests
    #[clap(long)]
    pub no_run: bool,

    /// Run all tests regardless of failure
    #[clap(long)]
    pub no_fail_fast: bool,

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

    /// Outputs a future incompatibility report at the end of the build (unstable)
    #[clap(long)]
    pub future_incompat_report: bool,

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

    #[clap(flatten)]
    pub xwin: XWinOptions,

    /// If specified, only run tests containing this string in their names
    #[clap(value_name = "TESTNAME", takes_value = true)]
    pub test_name: Option<String>,

    /// Arguments for the test binary
    #[clap(takes_value = true, multiple_values = true)]
    pub args: Vec<String>,
}

impl Test {
    /// Execute `cargo test` command
    pub fn execute(&self) -> Result<()> {
        let build = Build {
            quiet: self.quiet,
            packages: self.packages.clone(),
            workspace: self.workspace,
            exclude: self.exclude.clone(),
            all: self.all,
            jobs: self.jobs,
            lib: self.lib,
            bin: self.bin.clone(),
            bins: self.bins,
            example: self.example.clone(),
            examples: self.examples,
            test: self.test.clone(),
            tests: self.tests,
            bench: self.bench.clone(),
            benches: self.benches,
            all_targets: self.all_targets,
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
            future_incompat_report: self.future_incompat_report,
            verbose: self.verbose,
            color: self.color.clone(),
            frozen: self.frozen,
            locked: self.locked,
            offline: self.offline,
            config: self.config.clone(),
            unstable_flags: self.unstable_flags.clone(),
            xwin: self.xwin.clone(),
            ..Default::default()
        };
        let mut test = build.build_command("test")?;
        if self.doc {
            test.arg("--doc");
        }
        if self.no_run {
            test.arg("--no-run");
        }
        if self.no_fail_fast {
            test.arg("--no-fail-fast");
        }
        if let Some(test_name) = self.test_name.as_ref() {
            test.arg(test_name);
        }
        if !self.args.is_empty() {
            test.arg("--");
            test.args(&self.args);
        }

        if let Some(target) = self.target.as_ref() {
            if target.contains("msvc") {
                if env::var_os("WINEDEBUG").is_none() {
                    test.env("WINEDEBUG", "-all");
                }
                let env_target = target.to_uppercase().replace('-', "_");
                let runner_env = format!("CARGO_TARGET_{}_RUNNER", env_target);
                if env::var_os(&runner_env).is_none() {
                    test.env(runner_env, "wine");
                }
            }
        }

        let mut child = test.spawn().context("Failed to run cargo test")?;
        let status = child.wait().expect("Failed to wait on cargo test process");
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}
