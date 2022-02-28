use cargo_xwinbuild::Build;
use clap::Parser;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Parser)]
#[clap(
    version,
    name = "cargo",
    global_setting(clap::AppSettings::DeriveDisplayOrder)
)]
pub enum Opt {
    #[clap(name = "xwinbuild")]
    Build(Build),
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    match opt {
        Opt::Build(build) => build.execute()?,
    }
    Ok(())
}
