use std::env;
use std::ffi::OsString;
use std::process::Command;

use cargo_xwin::{Bench, Build, Cache, Check, Clippy, Doc, Env, Run, Rustc, Test};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    name = "cargo-xwin",
    styles = cargo_options::styles(),
)]
pub enum Cli {
    /// `cargo xwin` subcommand
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
    #[command(name = "bench")]
    Bench(Bench),
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
    #[command(name = "cache")]
    Cache(Cache),
}

fn cargo_global_arg_width(arg: &str) -> Option<usize> {
    let short_flags = arg
        .strip_prefix('-')
        .is_some_and(|flags| !flags.is_empty() && flags.chars().all(|c| matches!(c, 'q' | 'v')));
    let attached_value = arg.starts_with("--color=") || arg.starts_with("--config=");
    let attached_unstable = arg
        .strip_prefix("-Z")
        .is_some_and(|value| !value.is_empty());

    if matches!(arg, "--color" | "--config" | "-Z") {
        Some(2)
    } else if matches!(
        arg,
        "--quiet" | "--verbose" | "--locked" | "--offline" | "--frozen"
    ) || short_flags
        || attached_value
        || attached_unstable
    {
        Some(1)
    } else {
        None
    }
}

fn drain_global_cargo_args(
    args: &mut Vec<OsString>,
    index: usize,
    global_args: &mut Vec<OsString>,
) -> bool {
    loop {
        let Some(arg) = args.get(index) else {
            return true;
        };
        let Some(arg) = arg.to_str() else {
            return false;
        };
        let Some(width) = cargo_global_arg_width(arg) else {
            return true;
        };
        if index + width > args.len() {
            return false;
        }

        global_args.extend(args.drain(index..index + width));
    }
}

/// Cargo accepts global options on either side of its subcommand, while the
/// command-specific parsers used here accept them after the subcommand.
/// Normalize the former form into the latter before handing argv to clap.
fn normalize_global_cargo_args<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter().collect::<Vec<_>>();
    let original = args.clone();
    let mut global_args = Vec::new();

    if !drain_global_cargo_args(&mut args, 1, &mut global_args) {
        return original;
    }

    let mut command_index = 1;
    if args.get(command_index).and_then(|arg| arg.to_str()) == Some("xwin") {
        command_index += 1;
        if !drain_global_cargo_args(&mut args, command_index, &mut global_args) {
            return original;
        }
    }

    if global_args.is_empty() {
        return args;
    }

    let Some(command) = args.get(command_index).and_then(|arg| arg.to_str()) else {
        return original;
    };
    if command.starts_with('-') {
        return original;
    }

    args.splice(command_index + 1..command_index + 1, global_args);
    args
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse_from(normalize_global_cargo_args(env::args_os()));
    match cli {
        Cli::Opt(opt) | Cli::Cargo(opt) => match opt {
            Opt::Bench(bench) => bench.execute()?,
            Opt::Build(build) => build.execute()?,
            Opt::Run(run) => run.execute()?,
            Opt::Rustc(rustc) => rustc.execute()?,
            Opt::Test(test) => test.execute()?,
            Opt::Check(check) => check.execute()?,
            Opt::Clippy(clippy) => clippy.execute()?,
            Opt::Doc(doc) => doc.execute()?,
            Opt::Env(env) => env.execute()?,
            Opt::Cache(cache) => cache.execute()?,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(|value| OsString::from(*value)).collect()
    }

    #[test]
    fn normalize_cargo_global_options() {
        let cases: &[(&[&str], &[&str])] = &[
            (
                &["cargo-xwin", "--color=auto", "test", "--no-run"],
                &["cargo-xwin", "test", "--color=auto", "--no-run"],
            ),
            (
                &[
                    "cargo-xwin",
                    "--color",
                    "auto",
                    "--config",
                    "net.offline=true",
                    "-Z",
                    "unstable-options",
                    "-qv",
                    "check",
                ],
                &[
                    "cargo-xwin",
                    "check",
                    "--color",
                    "auto",
                    "--config",
                    "net.offline=true",
                    "-Z",
                    "unstable-options",
                    "-qv",
                ],
            ),
            (
                &["cargo-xwin", "xwin", "--offline", "build"],
                &["cargo-xwin", "xwin", "build", "--offline"],
            ),
            (
                &["cargo-xwin", "--locked", "xwin", "--color=always", "test"],
                &["cargo-xwin", "xwin", "test", "--locked", "--color=always"],
            ),
            (
                &["cargo-xwin", "test", "--color=always"],
                &["cargo-xwin", "test", "--color=always"],
            ),
            (
                &["cargo-xwin", "--target", "x86_64-pc-windows-msvc", "build"],
                &["cargo-xwin", "--target", "x86_64-pc-windows-msvc", "build"],
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(
                normalize_global_cargo_args(args(input)),
                args(expected),
                "input: {input:?}"
            );
        }
    }

    #[test]
    fn clap_accepts_global_options_before_cargo_subcommand() {
        let direct = normalize_global_cargo_args(args(&[
            "cargo-xwin",
            "--color=auto",
            "--offline",
            "test",
            "--no-run",
        ]));
        assert!(matches!(
            Cli::try_parse_from(direct),
            Ok(Cli::Cargo(Opt::Test(_)))
        ));

        let cargo_plugin = normalize_global_cargo_args(args(&[
            "cargo-xwin",
            "xwin",
            "--color=auto",
            "--offline",
            "test",
            "--no-run",
        ]));
        assert!(matches!(
            Cli::try_parse_from(cargo_plugin),
            Ok(Cli::Opt(Opt::Test(_)))
        ));
    }
}
