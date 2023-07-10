use anyhow::Context;
use cargo_options::Metadata;
use cargo_xwin::{Build, Check, Clippy, Run, Rustc, Test};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, name = "cargo-xwin")]
pub enum Cli {
    #[command(subcommand, name = "xwin")]
    Opt(Opt),
    // flatten opt here so that `cargo-xwin build` also works
    #[command(flatten)]
    Cargo(Opt),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
#[command(version, display_order = 1)]
pub enum Opt {
    #[command(name = "build", alias = "b")]
    Build(Build),
    Check(Check),
    Clippy(Clippy),
    #[command(name = "metadata")]
    Metadata(Metadata),
    #[command(name = "run", alias = "r")]
    Run(Run),
    #[command(name = "rustc")]
    Rustc(Rustc),
    #[command(name = "test", alias = "t")]
    Test(Test),
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli {
        Cli::Opt(opt) | Cli::Cargo(opt) => match opt {
            Opt::Build(build) => build.execute()?,
            Opt::Metadata(metadata) => {
                let mut cmd = metadata.command();
                let mut child = cmd.spawn().context("Failed to run cargo metadata")?;
                let status = child
                    .wait()
                    .expect("Failed to wait on cargo metadata process");
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            Opt::Run(run) => run.execute()?,
            Opt::Rustc(rustc) => rustc.execute()?,
            Opt::Test(test) => test.execute()?,
            Opt::Check(check) => check.execute()?,
            Opt::Clippy(clippy) => clippy.execute()?,
        },
    }
    Ok(())
}
