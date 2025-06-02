use std::env;
use std::ffi::OsString;
use std::process::Command;

use cargo_xwin::{Build, Check, Clippy, Doc, Env, Run, Rustc, Test};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    name = "cargo-xwin",
    styles = cargo_options::styles(),
)]
pub enum Cli {
    #[command(subcommand, name = "xwin")]
    Opt(Opt),
    // flatten opt here so that `cargo-xwin build` also works
    #[command(flatten)]
    Cargo(Opt),
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
#[command(version, display_order = 1)]
pub enum Opt {
    #[command(name = "build", alias = "b")]
    Build(Build),
    Check(Check),
    Clippy(Clippy),
    Doc(Doc),
    #[command(name = "run", alias = "r")]
    Run(Run),
    #[command(name = "rustc")]
    Rustc(Rustc),
    #[command(name = "test", alias = "t")]
    Test(Test),
    #[command(name = "env")]
    Env(Env),
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli {
        Cli::Opt(opt) | Cli::Cargo(opt) => match opt {
            Opt::Build(build) => build.execute()?,
            Opt::Run(run) => run.execute()?,
            Opt::Rustc(rustc) => rustc.execute()?,
            Opt::Test(test) => test.execute()?,
            Opt::Check(check) => check.execute()?,
            Opt::Clippy(clippy) => clippy.execute()?,
            Opt::Doc(doc) => doc.execute()?,
            Opt::Env(env) => env.execute()?,
        },
        Cli::External(args) => {
            let mut child = Command::new(env::var_os("CARGO").unwrap_or("cargo".into()))
                .args(args)
                .env_remove("CARGO")
                .spawn()?;
            let status = child.wait().expect("Failed to wait on cargo process");
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
    }
    Ok(())
}
