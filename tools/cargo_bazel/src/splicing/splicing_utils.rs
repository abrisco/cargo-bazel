use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use cargo_toml::{Dependency, Manifest};

pub fn default_cargo_workspace_manifest() -> cargo_toml::Manifest {
    // A manifest is generated with a fake workspace member so the [cargo_toml::Manifest::Workspace]
    // member is deseralized and is not `None`.
    let mut manifest = cargo_toml::Manifest::from_str(
        &toml::toml! {
            [workspace]
            members = ["TEMP"]
        }
        .to_string(),
    )
    .unwrap();

    // Drop the temp workspace member
    manifest.workspace.as_mut().unwrap().members.pop();

    manifest
}

pub fn default_cargo_package_manifest() -> cargo_toml::Manifest {
    // A manifest is generated with a fake workspace member so the [cargo_toml::Manifest::Workspace]
    // member is deseralized and is not `None`.
    let manifest = cargo_toml::Manifest::from_str(
        &toml::toml! {
            [package]
            name = "direct-cargo-bazel-deps"
            version = "0.0.1"
            edition = "2018"

            // A fake target used to satisfy requirements of Cargo.
            [lib]
            name = "direct_cargo_bazel_deps"
            path = ".direct_cargo_bazel_deps.rs"
        }
        .to_string(),
    )
    .unwrap();

    manifest
}

pub fn write_root_manifest(path: &Path, manifest: cargo_toml::Manifest) -> Result<()> {
    // Remove the file in case one exists already, preventing symlinked files
    // from having their contents overwritten.
    if path.exists() {
        fs::remove_file(path)?;
    }

    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // https://gitlab.com/crates.rs/cargo_toml/-/issues/3
    let value = toml::Value::try_from(&manifest)?;
    fs::write(path, toml::to_string(&value)?)
        .context(format!("Failed to write manifest to {}", path.display()))
}

/// Evaluates whether or not a manifest is considered a "workspace" manifest.
/// See [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html).
pub fn is_workspace(manifest: &Manifest) -> bool {
    // Anything with any workspace data is considered a workspace
    if manifest.workspace.is_some() {
        return true;
    }

    // Additionally, anything that contains path dependencies is also considered a workspace
    manifest.dependencies.iter().any(|(_, dep)| match dep {
        Dependency::Detailed(dep) => dep.path.is_some(),
        _ => false,
    })
}

/// Create a symlink file on unix systems
#[cfg(target_family = "unix")]
fn symlink(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    std::os::unix::fs::symlink(src, dest)
}

/// Create a symlink file on windows systems
#[cfg(target_family = "windows")]
fn symlink(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, dest)
    } else {
        std::os::windows::fs::symlink_file(src, dest)
    }
}

/// A list of files or directories to ignore when when symlinking
const IGNORE_LIST: &[&str] = &[".git", "bazel-bin", "bazel-out", ".svn"];

/// Symlinks the root contents of a source directory into a destination directory
pub fn symlink_roots(source: &Path, dest: &Path) -> Result<()> {
    // Ensure the source exists and is a directory
    if !source.is_dir() {
        bail!("Source path is not a directory: {}", source.display());
    }

    // Only check if the dest is a directory if it already exists
    if dest.exists() && !dest.is_dir() {
        bail!("Dest path is not a directory: {}", dest.display());
    }

    fs::create_dir_all(dest)?;

    // Link each directory entry from the source dir to the dest
    for entry in (source.read_dir()?).flatten() {
        let basename = entry.file_name();

        // Ignore certain directories that may lead to confusion
        if let Some(base_str) = basename.to_str() {
            if IGNORE_LIST.contains(&base_str) {
                continue;
            }
        }

        let link_src = source.join(&basename);
        let link_dest = dest.join(&basename);
        symlink(&link_src, &link_dest).context(format!(
            "Failed to create symlink: {} -> {}",
            link_src.display(),
            link_dest.display()
        ))?;
    }

    Ok(())
}

// Copies a file into place ensuring no symlink was present in it's place
pub fn install_file(src: &Path, dest: &Path) -> Result<()> {
    fs::remove_file(dest)?;
    fs::copy(src, dest)?;

    Ok(())
}
