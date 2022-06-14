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
    #[clap(name = "build")]
    Build(Build),
    #[clap(name = "run")]
    Run(Run),
    #[clap(name = "rustc")]
    Rustc(Rustc),
    #[clap(name = "test")]
    Test(Test),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Opt(opt) | Cli::Cargo(opt) => match opt {
            Opt::Build(build) => build.execute()?,
            Opt::Run(run) => run.execute()?,
            Opt::Rustc(rustc) => rustc.execute()?,
            Opt::Test(test) => test.execute()?,
        },
    }
    Ok(())
}
