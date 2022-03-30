use std::env;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use crate::common::XWinOptions;
use crate::Build;

/// Run a binary or example of the local package
#[derive(Clone, Debug, Default, Parser)]
#[clap(
    setting = clap::AppSettings::DeriveDisplayOrder,
    trailing_var_arg = true,
    after_help = "Run `cargo help run` for more detailed information.")
]
pub struct Run {
    #[clap(flatten)]
    pub xwin: XWinOptions,

    #[clap(flatten)]
    pub cargo: cargo_options::Run,
}

impl Run {
    /// Execute `cargo run` command
    pub fn execute(&self) -> Result<()> {
        let build = Build {
            cargo: self.cargo.clone().into(),
            xwin: self.xwin.clone(),
            ..Default::default()
        };
        let mut run = build.build_command("run")?;
        if !self.cargo.args.is_empty() {
            run.arg("--");
            run.args(&self.cargo.args);
        }

        for target in &self.cargo.target {
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
