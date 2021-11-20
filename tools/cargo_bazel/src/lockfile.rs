//! Utility module for interracting with different kinds of lock files

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use anyhow::{bail, Context as AnyhowContext, Result};
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};

use crate::config::Config;
use crate::context::Context;

#[derive(Debug)]
pub enum LockfileKind {
    Auto,
    Bazel,
    Cargo,
}

impl LockfileKind {
    pub fn detect(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;

        if serde_json::from_str::<Context>(&content).is_ok() {
            return Ok(Self::Bazel);
        }

        if cargo_lock::Lockfile::from_str(&content).is_ok() {
            return Ok(Self::Cargo);
        }

        bail!("Unknown Lockfile kind for {}", path.display())
    }
}

impl FromStr for LockfileKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        if lower == "auto" {
            return Ok(Self::Auto);
        }

        if lower == "bazel" {
            return Ok(Self::Bazel);
        }

        if lower == "cargo" {
            return Ok(Self::Cargo);
        }

        bail!("Unknown LockfileKind: '{}'", s)
    }
}

pub fn is_cargo_lockfile(path: &Path, kind: &LockfileKind) -> bool {
    match kind {
        LockfileKind::Auto => match LockfileKind::detect(path) {
            Ok(kind) => matches!(kind, LockfileKind::Cargo),
            Err(_) => false,
        },
        LockfileKind::Bazel => false,
        LockfileKind::Cargo => true,
    }
}

/// Write a [crate::planning::PlannedContext] to disk
pub fn write_lockfile(context: Context, path: &Path, dry_run: bool) -> Result<()> {
    let content = serde_json::to_string_pretty(&context)?;

    if dry_run {
        println!("{:#?}", content);
    } else {
        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
            .context(format!("Failed to write file to disk: {}", path.display()))?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Digest(String);

impl Digest {
    pub fn new(config: &Config, cargo_bin: &Path, rustc_bin: &Path) -> Result<Self> {
        let mut hasher = Sha256::new();

        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        hasher.update(b"\0");

        hasher.update(serde_json::to_string(config)?.as_bytes());
        hasher.update(b"\0");

        hasher.update(
            Self::bin_version(cargo_bin)
                .context("Failed to get Cargo version")?
                .as_bytes(),
        );
        hasher.update(b"\0");

        hasher.update(
            Self::bin_version(rustc_bin)
                .context("Failed to get Rustc version")?
                .as_bytes(),
        );
        hasher.update(b"\0");

        Ok(Self(hasher.finalize().encode_hex::<String>()))
    }

    fn bin_version(binary: &Path) -> Result<String> {
        let safe_vars = [OsStr::new("HOMEDRIVE"), OsStr::new("PATHEXT")];
        let env = std::env::vars_os().filter(|(var, _)| safe_vars.contains(&var.as_os_str()));

        let output = Command::new(binary)
            .arg("--version")
            .env_clear()
            .envs(env)
            .output()?;

        if !output.status.success() {
            bail!("Failed to query cargo version")
        }

        let version = String::from_utf8(output.stdout)?;
        Ok(version)
    }
}

impl PartialEq<str> for Digest {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<String> for Digest {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::fs;

    #[test]
    fn detect_bazel_lockfile() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lockfile = temp_dir.as_ref().join("lockfile");
        fs::write(
            &lockfile,
            serde_json::to_string(&crate::context::Context::default()).unwrap(),
        )
        .unwrap();

        let kind = LockfileKind::detect(&lockfile).unwrap();
        assert!(matches!(kind, LockfileKind::Bazel));
    }

    #[test]
    fn detect_cargo_lockfile() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lockfile = temp_dir.as_ref().join("lockfile");
        fs::write(
            &lockfile,
            textwrap::dedent(
                r#"
                version = 3

                [[package]]
                name = "detect"
                version = "0.1.0"
                "#,
            ),
        )
        .unwrap();

        let kind = LockfileKind::detect(&lockfile).unwrap();
        assert!(matches!(kind, LockfileKind::Cargo));
    }

    #[test]
    fn detect_invalid_lockfile() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lockfile = temp_dir.as_ref().join("lockfile");
        fs::write(&lockfile, "]} invalid {[").unwrap();

        assert!(LockfileKind::detect(&lockfile).is_err());
    }

    #[test]
    fn detect_missing_lockfile() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lockfile = temp_dir.as_ref().join("lockfile");
        assert!(LockfileKind::detect(&lockfile).is_err());
    }
}
