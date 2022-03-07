use std::env;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use crate::common::{CargoOptions, XWinOptions};
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
    pub cargo: CargoOptions,

    /// Package to run (see `cargo help pkgid`)
    #[clap(short = 'p', long = "package", value_name = "SPEC")]
    pub packages: Option<String>,

    /// Run the specified binary
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub bin: Vec<String>,

    /// Run the specified example
    #[clap(long, value_name = "NAME", multiple_values = true)]
    pub example: Vec<String>,

    #[clap(flatten)]
    pub xwin: XWinOptions,

    /// Arguments for the binary to run
    #[clap(takes_value = true, multiple_values = true)]
    pub args: Vec<String>,
}

impl Run {
    /// Execute `cargo run` command
    pub fn execute(&self) -> Result<()> {
        let build = Build {
            cargo: self.cargo.clone(),
            packages: self.packages.clone().map(|p| vec![p]).unwrap_or_default(),
            bin: self.bin.clone(),
            example: self.example.clone(),
            xwin: self.xwin.clone(),
            ..Default::default()
        };
        let mut run = build.build_command("run")?;
        if !self.args.is_empty() {
            run.arg("--");
            run.args(&self.args);
        }

        if let Some(target) = self.cargo.target.as_ref() {
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
