use std::env;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use clap::Parser;

use crate::options::XWinOptions;

/// Print environment variables required for cross-compilation
#[derive(Clone, Debug, Default, Parser)]
#[command(display_order = 1)]
pub struct Env {
    #[command(flatten)]
    pub xwin: XWinOptions,

    #[command(flatten)]
    pub cargo: cargo_options::CommonOptions,

    #[arg(long, value_name = "PATH", help_heading = cargo_options::heading::MANIFEST_OPTIONS)]
    pub manifest_path: Option<PathBuf>,
}

impl Env {
    /// Create a new env from manifest path
    #[allow(clippy::field_reassign_with_default)]
    pub fn new(manifest_path: Option<PathBuf>) -> Self {
        let mut build = Self::default();
        build.manifest_path = manifest_path;
        build
    }

    /// Print env
    pub fn execute(&self) -> Result<()> {
        let mut env = self.build_command()?;

        for target in &self.target {
            if target.contains("msvc") {
                if env::var_os("WINEDEBUG").is_none() {
                    env.env("WINEDEBUG", "-all");
                }
                let env_target = target.to_uppercase().replace('-', "_");
                let runner_env = format!("CARGO_TARGET_{}_RUNNER", env_target);
                if env::var_os(&runner_env).is_none() {
                    env.env(runner_env, "wine");
                }
            }
        }

        for (key, value) in env.get_envs() {
            println!(
                "export {}=\"{}\";",
                key.to_string_lossy(),
                value.unwrap_or_default().to_string_lossy()
            );
        }

        Ok(())
    }

    /// Generate cargo subcommand
    pub fn build_command(&self) -> Result<Command> {
        let mut build = Command::new("cargo");
        self.xwin
            .apply_command_env(self.manifest_path.as_deref(), &self.cargo, &mut build)?;
        Ok(build)
    }
}

impl Deref for Env {
    type Target = cargo_options::CommonOptions;

    fn deref(&self) -> &Self::Target {
        &self.cargo
    }
}

impl DerefMut for Env {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cargo
    }
}
