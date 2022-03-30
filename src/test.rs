use std::env;
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
    #[clap(flatten)]
    pub xwin: XWinOptions,

    #[clap(flatten)]
    pub cargo: cargo_options::Test,
}

impl Test {
    /// Execute `cargo test` command
    pub fn execute(&self) -> Result<()> {
        let build = Build {
            cargo: self.cargo.clone().into(),
            ..Default::default()
        };
        let mut test = build.build_command("test")?;
        if self.cargo.doc {
            test.arg("--doc");
        }
        if self.cargo.no_run {
            test.arg("--no-run");
        }
        if self.cargo.no_fail_fast {
            test.arg("--no-fail-fast");
        }
        if let Some(test_name) = self.cargo.test_name.as_ref() {
            test.arg(test_name);
        }
        if !self.cargo.args.is_empty() {
            test.arg("--");
            test.args(&self.cargo.args);
        }

        for target in &self.cargo.target {
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
