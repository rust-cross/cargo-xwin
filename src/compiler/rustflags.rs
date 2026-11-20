use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// Flags that Cargo will not read again from target configuration.
pub struct CargoRustFlags {
    pub flags: Vec<String>,
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
        // Preserve the existing target-scoped transport of explicit global
        // flags so Windows linker arguments do not reach other artifact targets.
        if vars.contains_key(OsStr::new("RUSTFLAGS"))
            || vars.contains_key(OsStr::new("CARGO_ENCODED_RUSTFLAGS"))
        {
            return Ok(Self {
                flags: config.rustflags(target)?.unwrap_or_default().flags,
                env_key,
                target: target.to_owned(),
            });
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
        let flags = if target_flags.flags.is_empty() {
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
        Ok(Self {
            flags: flags.flags,
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

#[cfg(test)]
mod tests {
    use super::*;
    use fs_err as fs;
    use std::collections::BTreeMap;

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
