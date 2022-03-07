use std::env;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use crate::common::{CargoOptions, XWinOptions};
use crate::Build;

/// Execute all unit and integration tests and build examples of a local package
#[derive(Clone, Debug, Default, Parser)]
#[clap(
    setting = clap::AppSettings::DeriveDisplayOrder,
    trailing_var_arg = true,
    after_help = "Run `cargo help test` for more detailed information.\nRun `cargo test -- --help` for test binary options.")
]
pub struct Test {
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

    /// Test all packages in the workspace
    #[clap(long)]
    pub workspace: bool,

    /// Exclude packages from the build
    #[clap(long, value_name = "SPEC", multiple_values = true)]
    pub exclude: Vec<String>,

    /// Alias for workspace (deprecated)
    #[clap(long)]
    pub all: bool,

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

    /// Outputs a future incompatibility report at the end of the build (unstable)
    #[clap(long)]
    pub future_incompat_report: bool,

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
            cargo: self.cargo.clone(),
            packages: self.packages.clone(),
            workspace: self.workspace,
            exclude: self.exclude.clone(),
            all: self.all,
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
            future_incompat_report: self.future_incompat_report,
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

        if let Some(target) = self.cargo.target.as_ref() {
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
