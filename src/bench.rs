use std::env;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::{self, Command};

use anyhow::{Context, Result};
use clap::Parser;

use crate::options::{RustflagsMode, XWinOptions, append_cargo_configs};

/// Execute all benchmarks of a local package
#[derive(Clone, Debug, Default, Parser)]
#[command(
    display_order = 1,
    after_help = "Run `cargo help bench` for more detailed information."
)]
pub struct Bench {
    #[command(flatten)]
    pub xwin: XWinOptions,

    #[command(flatten)]
    pub cargo: cargo_options::Bench,
}

impl Bench {
    /// Create a new bench from manifest path
    #[allow(clippy::field_reassign_with_default)]
    pub fn new(manifest_path: Option<PathBuf>) -> Self {
        let mut build = Self::default();
        build.manifest_path = manifest_path;
        build
    }

    /// Execute `cargo bench` command
    pub fn execute(&self) -> Result<()> {
        let mut run = self.build_command()?;

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

    /// Generate cargo subcommand
    pub fn build_command(&self) -> Result<Command> {
        let mut cargo = self.cargo.clone();
        let bench_name = cargo.bench.bench_name.take();
        let args = std::mem::take(&mut cargo.bench.args);
        let mut build = cargo.command();
        let cargo_configs = self.xwin.prepare_command_env(
            self.manifest_path.as_deref(),
            &cargo.common,
            &mut build,
            RustflagsMode::CargoConfig,
        )?;
        append_cargo_configs(&mut build, cargo_configs);
        if bench_name.is_some() || !args.is_empty() {
            build.arg("--");
            if let Some(bench_name) = bench_name {
                build.arg(bench_name);
            }
            build.args(args);
        }
        Ok(build)
    }
}

impl Deref for Bench {
    type Target = cargo_options::Bench;

    fn deref(&self) -> &Self::Target {
        &self.cargo
    }
}

impl DerefMut for Bench {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cargo
    }
}

impl From<cargo_options::Bench> for Bench {
    fn from(cargo: cargo_options::Bench) -> Self {
        Self {
            cargo,
            ..Default::default()
        }
    }
}
