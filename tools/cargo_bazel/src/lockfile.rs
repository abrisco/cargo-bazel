///! Utility module for interracting with different kinds of lock files
use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::{bail, Context as AnyhowContext, Result};

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

        if serde_json::from_str::<Context>(&content).is_ok() {
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
        LockfileKind::Auto => {}
        LockfileKind::Bazel => return false,
        LockfileKind::Cargo => return true,
    };

    let kind = match LockfileKind::detect(path) {
        Ok(kind) => kind,
        Err(_) => return false,
    };

    matches!(kind, LockfileKind::Cargo)
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
