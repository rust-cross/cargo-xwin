use anyhow::Context;
use cargo_options::Metadata;
use cargo_xwin::{Build, Run, Rustc, Test};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(version, name = "cargo-xwin")]
pub enum Cli {
    #[clap(subcommand, name = "xwin")]
    Opt(Opt),
    // flatten opt here so that `cargo-xwin build` also works
    #[clap(flatten)]
    Cargo(Opt),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
#[clap(version, global_setting(clap::AppSettings::DeriveDisplayOrder))]
pub enum Opt {
    #[clap(name = "build", alias = "b")]
    Build(Build),
    #[clap(name = "metadata")]
    Metadata(Metadata),
    #[clap(name = "run", alias = "r")]
    Run(Run),
    #[clap(name = "rustc")]
    Rustc(Rustc),
    #[clap(name = "test", alias = "t")]
    Test(Test),
}

fn main() -> anyhow::Result<()> {
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
        },
    }
    Ok(())
}
