use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// Target CRT selection and flags that Cargo will not read again from target configuration.
pub struct CargoRustFlags {
    pub flags: Vec<String>,
    pub is_static_crt: bool,
    env_key: String,
    target: String,
}

impl CargoRustFlags {
    pub fn load(workdir: &Path, target: &str) -> Result<Self> {
        Self::load_with_env(workdir, target, env::vars_os().collect())
    }

    fn load_with_env(
        workdir: &Path,
        target: &str,
        mut vars: std::collections::BTreeMap<OsString, OsString>,
    ) -> Result<Self> {
        let env_key = format!(
            "CARGO_TARGET_{}_RUSTFLAGS",
            target.to_uppercase().replace('-', "_")
        );
        let config = cargo_config2::Config::load_with_options(
            workdir,
            cargo_config2::ResolveOptions::default().env(vars.clone()),
        )?;
        // apply() forwards RUSTFLAGS as target flags, which Cargo merges with
        // target configuration. Inspect the same combination for CRT selection.
        let global_flags = vars.remove(OsStr::new("RUSTFLAGS"));
        let has_global_flags =
            global_flags.is_some() || vars.contains_key(OsStr::new("CARGO_ENCODED_RUSTFLAGS"));
        if let Some(flags) = global_flags {
            vars.insert(env_key.clone().into(), flags);
        }
        // Resolve once with an empty build fallback to distinguish target flags
        // (including matching cfg tables) from build.rustflags. Adding our target
        // flags would otherwise suppress Cargo's build.rustflags fallback.
        vars.insert("CARGO_BUILD_RUSTFLAGS".into(), "".into());
        let target_config = cargo_config2::Config::load_with_options(
            workdir,
            cargo_config2::ResolveOptions::default().env(vars.clone()),
        )?;
        let target_flags = target_config.rustflags(target)?.unwrap_or_default();
        let mut flags = if has_global_flags || target_flags.flags.is_empty() {
            config.build.rustflags.unwrap_or_default()
        } else {
            // Cargo merges this environment variable with target config itself.
            // Re-exporting the resolved config would duplicate those flags.
            vars.get(OsStr::new(&env_key))
                .map(|value| {
                    value
                        .to_str()
                        .context("invalid target RUSTFLAGS")
                        .map(cargo_config2::Flags::from_space_separated)
                })
                .transpose()?
                .unwrap_or_default()
        };
        let resolved = if target_flags.flags.is_empty() {
            &flags.flags
        } else {
            &target_flags.flags
        };
        let is_static_crt = resolved.iter().any(|flag| flag.contains("+crt-static"));
        // Prefer libucrt for static CRT builds to resolve symbols such as
        // __stdio_common_vsscanf locally. Hybrid builds explicitly use the
        // system UCRT while keeping the rest of the CRT static.
        if is_static_crt && !requests_dynamic_ucrt(resolved) {
            flags.flags.extend([
                "-C".to_string(),
                "link-arg=-nodefaultlib:ucrt".to_string(),
                "-C".to_string(),
                "link-arg=-defaultlib:libucrt".to_string(),
            ]);
        }
        Ok(Self {
            flags: flags.flags,
            is_static_crt,
            env_key,
            target: target.to_owned(),
        })
    }

    pub fn apply(self, cmd: &mut Command) -> Result<()> {
        let mut flags = cargo_config2::Flags::default();
        flags.flags = self.flags;
        cmd.env_remove("RUSTFLAGS");
        match flags.encode_space_separated() {
            Ok(value) => {
                cmd.env(&self.env_key, value);
            }
            Err(_) => {
                // Cargo merges this array with target configuration, just like the
                // environment transport. Only export our additions, not resolved
                // target config, to avoid applying user flags twice.
                let target = toml::Value::String(self.target);
                let value =
                    toml::Value::Array(flags.flags.into_iter().map(toml::Value::String).collect());
                cmd.arg("--config")
                    .arg(format!("target.{target}.rustflags={value}"));
                cmd.env_remove(&self.env_key);
            }
        }
        Ok(())
    }
}

fn requests_dynamic_ucrt(flags: &[String]) -> bool {
    let mut flags = flags.iter().map(String::as_str);
    while let Some(flag) = flags.next() {
        let option = match flag {
            "-C" | "--codegen" => flags.next(),
            _ => flag
                .strip_prefix("-C")
                .or_else(|| flag.strip_prefix("--codegen=")),
        };
        match option.and_then(|option| option.split_once('=')) {
            Some(("link-arg" | "link_arg", arg)) if selects_dynamic_ucrt(arg) => return true,
            Some(("link-args" | "link_args", args))
                if args.split_whitespace().any(selects_dynamic_ucrt) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn selects_dynamic_ucrt(arg: &str) -> bool {
    let Some(arg) = arg.strip_prefix(['/', '-']) else {
        return false;
    };
    let arg = arg.to_ascii_lowercase();
    matches!(
        arg.strip_suffix(".lib").unwrap_or(&arg),
        "defaultlib:ucrt" | "nodefaultlib:libucrt"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_err as fs;
    use std::collections::BTreeMap;

    #[test]
    fn crt_selection() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join(".cargo"))?;
        let target = "x86_64-pc-windows-msvc";
        let static_crt = "-C target-feature=+crt-static";
        let hybrid = r#"[
            "-C", "link-args=/NODEFAULTLIB:libucrt.lib",
            "-C", "link-args=/DEFAULTLIB:ucrt.lib",
        ]"#;
        let import = r#"["-Clink-arg=-DefaultLib:UCRT"]"#;
        let exclude = r#"["--codegen=link-args=/DEBUG -NoDefaultLib:LibUcrt"]"#;
        let other = r#"["-Clink-arg=/defaultlib:ucrtd.lib"]"#;
        for (triple_flags, cfg_flags, global, static_runtime, static_ucrt) in [
            ("", "[]", None, false, false),
            ("", "[]", Some("-C opt-level=3"), false, false),
            ("", "[]", Some(static_crt), true, true),
            (static_crt, "[]", None, true, true),
            (static_crt, hybrid, None, true, false),
            ("", hybrid, Some(static_crt), true, false),
            (static_crt, import, None, true, false),
            (static_crt, exclude, None, true, false),
            (static_crt, other, None, true, true),
        ] {
            let config = format!(
                "[target.{target}]\nrustflags = '{triple_flags}'\n\
                 [target.'cfg(windows)']\nrustflags = {cfg_flags}\n"
            );
            fs::write(dir.path().join(".cargo/config.toml"), &config)?;
            let mut vars = BTreeMap::new();
            vars.insert(
                "CARGO_HOME".into(),
                dir.path().join("cargo-home").into_os_string(),
            );
            if let Some(global) = global {
                vars.insert("RUSTFLAGS".into(), global.into());
            }
            let flags = CargoRustFlags::load_with_env(dir.path(), target, vars)?;
            assert_eq!(flags.is_static_crt, static_runtime, "{config}");
            for arg in [
                "link-arg=-nodefaultlib:ucrt",
                "link-arg=-defaultlib:libucrt",
            ] {
                assert_eq!(
                    flags.flags.iter().any(|f| f == arg),
                    static_ucrt,
                    "{config}"
                );
            }
        }
        Ok(())
    }

    // Use real Cargo compilation to test its merge with our exported flags.
    // A host target and an empty crate avoid SDK, linker and nightly dependencies.
    #[test]
    fn cargo_rustflags_are_applied_once() -> Result<()> {
        let rustc = Command::new("rustc").arg("-vV").output()?;
        let version = String::from_utf8(rustc.stdout)?;
        let target = version
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .unwrap();
        let target_key = format!(
            "CARGO_TARGET_{}_RUSTFLAGS",
            target.to_uppercase().replace('-', "_")
        );
        let target_config = format!("[target.{target}]\nrustflags = [\"--cfg=from_target\"]\n");
        let build_config = "[build]\nrustflags = [\"--cfg=from_build\"]\n";
        let cases: Vec<(&str, String, Vec<(&str, &str)>, Vec<&str>)> = vec![
            ("array", target_config.clone(), vec![], vec!["from_target"]),
            (
                "string",
                format!("[target.{target}]\nrustflags = \"--cfg=from_target\"\n"),
                vec![],
                vec!["from_target"],
            ),
            (
                "cfg",
                "[target.'cfg(all())']\nrustflags = [\"--cfg=from_cfg\"]\n".into(),
                vec![],
                vec!["from_cfg"],
            ),
            ("build", build_config.into(), vec![], vec!["from_build"]),
            (
                "build_env",
                build_config.into(),
                vec![("CARGO_BUILD_RUSTFLAGS", "--cfg=from_env")],
                vec!["from_env"],
            ),
            (
                "target_over_build",
                format!("{target_config}{build_config}"),
                vec![],
                vec!["from_target"],
            ),
            (
                "target_env",
                target_config.clone(),
                vec![(&target_key, "--cfg=from_env")],
                vec!["from_target", "from_env"],
            ),
            (
                "global",
                target_config.clone(),
                vec![("RUSTFLAGS", "--cfg=from_global")],
                vec!["from_target", "from_global"],
            ),
            (
                "empty_global",
                target_config.clone(),
                vec![("RUSTFLAGS", "")],
                vec!["from_target"],
            ),
            (
                "encoded",
                target_config.clone(),
                vec![
                    ("RUSTFLAGS", "--cfg=from_global"),
                    ("CARGO_ENCODED_RUSTFLAGS", "--cfg=from_encoded"),
                ],
                vec!["from_encoded"],
            ),
            (
                "empty_encoded",
                target_config,
                vec![("CARGO_ENCODED_RUSTFLAGS", "")],
                vec![],
            ),
        ];
        for spaced in [false, true] {
            for (name, config, extra_env, expected) in cases.clone() {
                let dir = tempfile::tempdir()?;
                fs::create_dir_all(dir.path().join(".cargo"))?;
                fs::create_dir_all(dir.path().join("src"))?;
                fs::write(
                    dir.path().join("Cargo.toml"),
                    "[package]\nname = \"flags-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
                )?;
                fs::write(
                    dir.path().join("src/lib.rs"),
                    if !spaced || name.contains("encoded") {
                        ""
                    } else {
                        "#[cfg(not(cache_path = \"directory with spaces\"))] compile_error!(\"space-containing flag was lost\");"
                    },
                )?;
                fs::write(
                    dir.path().join("build.rs"),
                    "#[cfg(from_xwin)] compile_error!(\"target flags reached host build script\"); fn main() {}",
                )?;
                fs::write(dir.path().join(".cargo/config.toml"), config)?;
                let mut vars: BTreeMap<OsString, OsString> = env::vars_os()
                    .filter(|(key, _)| {
                        !key.to_string_lossy().starts_with("CARGO")
                            && !key.to_string_lossy().starts_with("RUST")
                    })
                    .collect();
                vars.insert(
                    "CARGO_HOME".into(),
                    dir.path().join("cargo-home").into_os_string(),
                );
                for (key, value) in extra_env {
                    vars.insert(key.into(), value.into());
                }
                let mut flags = CargoRustFlags::load_with_env(dir.path(), target, vars.clone())?;
                flags.flags.push("--cfg=from_xwin".into());
                if spaced {
                    flags
                        .flags
                        .push("--cfg=cache_path=\"directory with spaces\"".into());
                }
                let mut cmd = Command::new("cargo");
                cmd.env_clear().envs(vars).current_dir(dir.path()).args([
                    "check",
                    "--offline",
                    "--target",
                    target,
                    "-v",
                ]);
                flags.apply(&mut cmd)?;
                let output = cmd.output()?;
                let stderr = String::from_utf8_lossy(&output.stderr);
                if spaced && name == "string" {
                    // Cargo cannot merge an array CLI override with a string in a file.
                    assert!(
                        stderr.contains("expected string, but found array"),
                        "{stderr}"
                    );
                    continue;
                }
                assert!(output.status.success(), "{name}, spaced={spaced}: {stderr}");
                let invocation = stderr
                    .lines()
                    .find(|line| line.contains("--crate-name flags_probe"))
                    .unwrap();
                for flag in [
                    "from_target",
                    "from_cfg",
                    "from_build",
                    "from_env",
                    "from_global",
                    "from_encoded",
                    "from_xwin",
                ] {
                    let count = usize::from(
                        (flag == "from_xwin" && !name.contains("encoded"))
                            || expected.contains(&flag),
                    );
                    assert_eq!(
                        invocation.matches(&format!("--cfg={flag}")).count(),
                        count,
                        "{name}: {invocation}"
                    );
                }
            }
        }
        Ok(())
    }
}
