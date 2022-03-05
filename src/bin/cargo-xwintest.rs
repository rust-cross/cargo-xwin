use cargo_xwinbuild::Test;
use clap::Parser;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Parser)]
#[clap(
    version,
    name = "cargo",
    global_setting(clap::AppSettings::DeriveDisplayOrder)
)]
pub enum Opt {
    #[clap(name = "xwintest")]
    Test(Test),
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    match opt {
        Opt::Test(test) => test.execute()?,
    }
    Ok(())
}
