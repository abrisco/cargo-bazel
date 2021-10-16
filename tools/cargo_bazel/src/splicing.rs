//! This module is responsible for finding a Cargo workspace

mod splicer;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use cargo_toml::Manifest;
use serde::{Deserialize, Serialize};

use crate::config::CrateId;
use crate::metadata::LockGenerator;
use crate::utils::starlark::Label;

pub use self::splicer::*;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExtraManifestInfo {
    // The path to a Cargo Manifest
    pub manifest: PathBuf,

    // The URL where the manifest's package can be downloaded
    pub url: String,

    // The Sha256 checksum of the downloaded package located at `url`.
    pub sha256: String,
}

type DirectPackageManifest = BTreeMap<String, cargo_toml::DependencyDetail>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplicingManifest {
    /// A set of all packages directly written to the rule
    pub direct_packages: DirectPackageManifest,

    /// A collection of information required for reproducible "extra worksspace members".
    pub extra_manifest_infos: Vec<ExtraManifestInfo>,

    /// A mapping of manifest paths to the labels representing them
    pub manifests: BTreeMap<PathBuf, Label>,
}

impl FromStr for SplicingManifest {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SourceInfo {
    /// A url where to a `.crate` file.
    pub url: String,

    /// The `.crate` file's sha256 checksum.
    pub sha256: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    #[serde(serialize_with = "toml::ser::tables_last")]
    pub sources: BTreeMap<CrateId, SourceInfo>,

    pub workspace_prefix: Option<String>,

    #[serde(serialize_with = "toml::ser::tables_last")]
    pub package_prefixes: BTreeMap<String, String>,
}

impl WorkspaceMetadata {
    fn new(
        splicing_manifest: &SplicingManifest,
        injected_manifests: HashMap<&PathBuf, String>,
    ) -> Result<Self> {
        let mut sources = BTreeMap::new();

        for config in splicing_manifest.extra_manifest_infos.iter() {
            let package = match read_manifest(&config.manifest) {
                Ok(manifest) => match manifest.package {
                    Some(pkg) => pkg,
                    None => continue,
                },
                Err(e) => return Err(e),
            };

            let id = CrateId::new(package.name, package.version);
            let info = SourceInfo {
                url: config.url.clone(),
                sha256: config.sha256.clone(),
            };

            sources.insert(id, info);
        }

        let mut package_prefixes: BTreeMap<String, String> = injected_manifests
            .iter()
            .filter_map(|(original_manifest, cargo_pkg_name)| {
                let label = match splicing_manifest.manifests.get(*original_manifest) {
                    Some(v) => v,
                    None => return None,
                };

                let package = match &label.package {
                    Some(pkg) => PathBuf::from(pkg),
                    None => return None,
                };

                let prefix = package.to_string_lossy().to_string();

                Some((cargo_pkg_name.clone(), prefix))
            })
            .collect();

        // It is invald for toml maps to use empty strings as keys. In the case
        // the empty key is expected to be the root package. If the root package
        // has a prefix, then all other packages will as well (even if no other
        // manifest represents them). The value is then saved as a separate value
        let workspace_prefix = package_prefixes.remove("");

        let package_prefixes = package_prefixes
            .into_iter()
            .map(|(k, v)| {
                let prefix_path = PathBuf::from(v);
                let prefix = prefix_path.parent().unwrap();
                (k, prefix.to_string_lossy().to_string())
            })
            .collect();

        Ok(Self {
            sources,
            workspace_prefix,
            package_prefixes,
        })
    }

    fn should_skip_serializing(&self) -> bool {
        self.sources.is_empty()
            && self.workspace_prefix.is_none()
            && self.package_prefixes.is_empty()
    }

    fn inject_into(&self, manifest: &mut Manifest) -> Result<()> {
        // Do not bother rendering anything if the table is empty
        if self.should_skip_serializing() {
            return Ok(());
        }

        let metadata_value = toml::Value::try_from(self)?;
        let mut workspace = manifest.workspace.as_mut().unwrap();

        match &mut workspace.metadata {
            Some(data) => match data.as_table_mut() {
                Some(map) => {
                    map.insert("cargo-bazel".to_owned(), metadata_value);
                }
                None => bail!("The metadata field is always expected to be a table"),
            },
            None => {
                let mut table = toml::map::Map::new();
                table.insert("cargo-bazel".to_owned(), metadata_value);
                workspace.metadata = Some(toml::Value::Table(table))
            }
        }

        Ok(())
    }
}

pub enum SplicedManifest {
    Workspace(PathBuf),
    Package(PathBuf),
    MultiPackage(PathBuf),
}

impl SplicedManifest {
    pub fn as_path_buf(&self) -> &PathBuf {
        match self {
            SplicedManifest::Workspace(p) => p,
            SplicedManifest::Package(p) => p,
            SplicedManifest::MultiPackage(p) => p,
        }
    }
}

// Copies a file into place ensuring no symlink was present in it's place
pub fn install_file(src: &Path, dest: &Path) -> Result<()> {
    fs::remove_file(dest)?;
    fs::copy(src, dest)?;

    Ok(())
}

pub fn read_manifest(manifest: &Path) -> Result<Manifest> {
    let content = fs::read_to_string(manifest)?;
    cargo_toml::Manifest::from_str(content.as_str()).context("Failed to deserialize manifest")
}

pub fn generate_lockfile(
    manifest_path: &SplicedManifest,
    existing_lock: &Option<PathBuf>,
    cargo_bin: &Path,
    rustc_bin: &Path,
) -> Result<()> {
    let manifest_dir = manifest_path
        .as_path_buf()
        .parent()
        .expect("Every manifest should be contained in a parent directory");

    let root_lockfile_path = manifest_dir.join("Cargo.lock");

    // Optionally copy the given lockfile into place or install extra workspace members and
    // splice a new one. Note that it's invalid for an existing lockfile to be used with
    // extra workspace members.
    if let Some(lock) = existing_lock {
        install_file(lock, &root_lockfile_path)?;
        return Ok(());
    }

    // Remove the file so it's not overwitten if it happens to be a symlink.
    if root_lockfile_path.exists() {
        fs::remove_file(&root_lockfile_path)?;
    }

    // Generate the new lockfile
    LockGenerator::new(PathBuf::from(cargo_bin), PathBuf::from(rustc_bin))
        .generate(manifest_path.as_path_buf())?;

    // Write the lockfile to disk
    if !root_lockfile_path.exists() {
        bail!("Failed to generate Cargo.lock file")
    }

    Ok(())
}
