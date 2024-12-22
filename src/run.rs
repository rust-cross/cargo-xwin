use std::env;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::{self, Command};

use anyhow::{Context, Result};
use clap::Parser;

use crate::options::XWinOptions;

/// Run a binary or example of the local package
#[derive(Clone, Debug, Default, Parser)]
#[command(
    display_order = 1,
    after_help = "Run `cargo help run` for more detailed information."
)]
pub struct Run {
    #[command(flatten)]
    pub xwin: XWinOptions,

    #[command(flatten)]
    pub cargo: cargo_options::Run,
}

impl Run {
    /// Create a new run from manifest path
    #[allow(clippy::field_reassign_with_default)]
    pub fn new(manifest_path: Option<PathBuf>) -> Self {
        let mut build = Self::default();
        build.manifest_path = manifest_path;
        build
    }

    /// Execute `cargo run` command
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
        let mut build = self.cargo.command();
        self.xwin.apply_command_env(
            self.manifest_path.as_deref(),
            &self.cargo.common,
            &mut build,
        )?;
        Ok(build)
    }
}

impl Deref for Run {
    type Target = cargo_options::Run;

    fn deref(&self) -> &Self::Target {
        &self.cargo
    }
}

impl DerefMut for Run {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cargo
    }
}

impl From<cargo_options::Run> for Run {
    fn from(cargo: cargo_options::Run) -> Self {
        Self {
            cargo,
            ..Default::default()
        }
    }
}
