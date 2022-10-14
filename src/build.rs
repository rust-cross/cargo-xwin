use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::{self, Command};

use anyhow::{Context, Result};
use clap::Parser;

use crate::common::XWinOptions;

/// Compile a local package and all of its dependencies
#[derive(Clone, Debug, Default, Parser)]
#[command(
    display_order = 1,
    after_help = "Run `cargo help build` for more detailed information."
)]
pub struct Build {
    #[command(flatten)]
    pub cargo: cargo_options::Build,

    #[command(flatten)]
    pub xwin: XWinOptions,
}

impl Build {
    /// Create a new build from manifest path
    #[allow(clippy::field_reassign_with_default)]
    pub fn new(manifest_path: Option<PathBuf>) -> Self {
        let mut build = Self::default();
        build.manifest_path = manifest_path;
        build
    }

    /// Execute `cargo build` command
    pub fn execute(&self) -> Result<()> {
        let mut build = self.build_command()?;
        let mut child = build.spawn().context("Failed to run cargo build")?;
        let status = child.wait().expect("Failed to wait on cargo build process");
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }

    /// Generate cargo subcommand
    pub fn build_command(&self) -> Result<Command> {
        let mut build = self.cargo.command();
        self.xwin
            .apply_command_env(&self.cargo.common, &mut build)?;
        Ok(build)
    }
}

impl Deref for Build {
    type Target = cargo_options::Build;

    fn deref(&self) -> &Self::Target {
        &self.cargo
    }
}

impl DerefMut for Build {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cargo
    }
}

impl From<cargo_options::Build> for Build {
    fn from(cargo: cargo_options::Build) -> Self {
        Self {
            cargo,
            ..Default::default()
        }
    }
}
