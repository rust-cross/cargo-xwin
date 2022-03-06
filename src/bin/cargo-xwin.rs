use cargo_xwin::{Build, Test};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(version, name = "cargo")]
pub enum Cli {
    #[clap(subcommand, name = "xwin")]
    Opt(Opt),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
#[clap(global_setting(clap::AppSettings::DeriveDisplayOrder))]
pub enum Opt {
    #[clap(name = "build")]
    Build(Build),
    #[clap(name = "test")]
    Test(Test),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Opt(opt) => match opt {
            Opt::Build(build) => build.execute()?,
            Opt::Test(test) => test.execute()?,
        },
    }
    Ok(())
}
