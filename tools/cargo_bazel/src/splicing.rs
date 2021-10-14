//! This module is responsible for finding a Cargo workspace

pub mod splicing_utils;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use cargo_toml::Manifest;
use serde::{Deserialize, Serialize};

use crate::config::CrateId;
use crate::metadata::LockGenerator;
use crate::splicing::splicing_utils::*;
use crate::utils::starlark::Label;

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

/// The core splicer implementation. Each style of Bazel workspace should be represented
/// here and a splicing implementation defined.
pub enum SplicerKind<'a> {
    /// Splice a manifest which is represented by a Cargo workspace
    Workspace {
        path: &'a PathBuf,
        manifest: &'a Manifest,
        splicing_manifest: &'a SplicingManifest,
    },
    /// Splice a manifest for a single package. This includes cases where
    /// were defined directly in Bazel.
    Package {
        path: &'a PathBuf,
        manifest: &'a Manifest,
        splicing_manifest: &'a SplicingManifest,
    },
    /// Splice a manifest from multiple disjoint Cargo manifests.
    MultiPackage {
        manifests: &'a HashMap<PathBuf, Manifest>,
        splicing_manifest: &'a SplicingManifest,
    },
}

impl<'a> SplicerKind<'a> {
    pub fn new(
        manifests: &'a HashMap<PathBuf, Manifest>,
        splicing_manifest: &'a SplicingManifest,
    ) -> Result<Self> {
        // First check for any workspaces in the provided manifests
        let mut workspaces: HashMap<&PathBuf, &Manifest> = manifests
            .iter()
            .filter(|(_, manifest)| is_workspace(manifest))
            .collect();

        // Filter out any invalid manifest combinations
        if workspaces.len() > 1 {
            bail!("When splicing manifests, there can only be 1 workspace manifest");
        }
        if !workspaces.is_empty() && manifests.len() > 1 {
            bail!("Workspace manifests can not be used with any other manifests")
        }

        if workspaces.len() == 1 {
            let (path, manifest) = workspaces.drain().last().unwrap();

            Ok(Self::Workspace {
                path,
                manifest,
                splicing_manifest,
            })
        } else if manifests.len() == 1 {
            let (path, manifest) = manifests.iter().last().unwrap();
            Ok(Self::Package {
                path,
                manifest,
                splicing_manifest,
            })
        } else {
            Ok(Self::MultiPackage {
                manifests,
                splicing_manifest,
            })
        }
    }

    /// Performs splicing based on the current variant.
    pub fn splice(&self, workspace_dir: &Path) -> Result<SplicedManifest> {
        match self {
            SplicerKind::Workspace {
                path,
                manifest,
                splicing_manifest,
            } => Self::splice_workspace(workspace_dir, path, manifest, splicing_manifest),
            SplicerKind::Package {
                path,
                manifest,
                splicing_manifest,
            } => Self::splice_package(workspace_dir, path, manifest, splicing_manifest),
            SplicerKind::MultiPackage {
                manifests,
                splicing_manifest,
            } => Self::splice_multi_package(workspace_dir, manifests, splicing_manifest),
        }
    }

    fn splice_workspace(
        workspace_dir: &Path,
        path: &&PathBuf,
        manifest: &&Manifest,
        splicing_manifest: &&SplicingManifest,
    ) -> Result<SplicedManifest> {
        let mut manifest = (*manifest).clone();
        let manifest_dir = path
            .parent()
            .expect("Every manifest should havee a parent directory");

        let extra_workspace_manifests =
            Self::get_extra_workspace_manifests(&splicing_manifest.extra_manifest_infos)?;

        // Link the sources of the root manifest into the new workspace
        symlink_roots(manifest_dir, workspace_dir)?;

        // Add additional workspace members to the new manifest
        let mut installations = Self::inject_workspace_members(
            &mut manifest,
            &extra_workspace_manifests,
            workspace_dir,
        )?;

        // Add any additional depeendencies to the root package
        Self::inject_direct_packages(&mut manifest, &splicing_manifest.direct_packages)?;

        let root_manifest_path = workspace_dir.join("Cargo.toml");
        installations.insert(path, String::new());

        // Write the generated metadata to the manifest
        let workspace_metadata = WorkspaceMetadata::new(splicing_manifest, installations)?;
        workspace_metadata.inject_into(&mut manifest)?;

        // Write the root manifest
        write_root_manifest(&root_manifest_path, manifest)?;

        Ok(SplicedManifest::Workspace(root_manifest_path))
    }

    fn splice_package(
        workspace_dir: &Path,
        path: &&PathBuf,
        manifest: &&Manifest,
        splicing_manifest: &&SplicingManifest,
    ) -> Result<SplicedManifest> {
        let manifest_dir = path
            .parent()
            .expect("Every manifest should havee a parent directory");

        let extra_workspace_manifests =
            Self::get_extra_workspace_manifests(&splicing_manifest.extra_manifest_infos)?;

        // Link the sources of the root manifest into the new workspace
        symlink_roots(manifest_dir, workspace_dir)?;

        // Ensure the root package manifest has a populated `workspace` member
        let mut manifest = (*manifest).clone();
        if manifest.workspace.is_none() {
            manifest.workspace = default_cargo_workspace_manifest().workspace
        }

        // Add additional workspace members to the new manifest
        let mut installations = Self::inject_workspace_members(
            &mut manifest,
            &extra_workspace_manifests,
            workspace_dir,
        )?;

        // Add any additional depeendencies to the root package
        Self::inject_direct_packages(&mut manifest, &splicing_manifest.direct_packages)?;

        let root_manifest_path = workspace_dir.join("Cargo.toml");
        installations.insert(path, String::new());

        // Write the generated metadata to the manifest
        let workspace_metadata = WorkspaceMetadata::new(splicing_manifest, installations)?;
        workspace_metadata.inject_into(&mut manifest)?;

        // Write the root manifest
        write_root_manifest(&root_manifest_path, manifest)?;

        Ok(SplicedManifest::Package(root_manifest_path))
    }

    fn splice_multi_package(
        workspace_dir: &Path,
        manifests: &&HashMap<PathBuf, Manifest>,
        splicing_manifest: &&SplicingManifest,
    ) -> Result<SplicedManifest> {
        let mut manifest = default_cargo_workspace_manifest();

        let extra_workspace_manifests =
            Self::get_extra_workspace_manifests(&splicing_manifest.extra_manifest_infos)?;

        let manifests: HashMap<PathBuf, Manifest> = manifests
            .iter()
            .map(|(p, m)| (p.to_owned(), m.to_owned()))
            .collect();

        let all_manifests = manifests
            .iter()
            .chain(extra_workspace_manifests.iter())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let installations =
            Self::inject_workspace_members(&mut manifest, &all_manifests, workspace_dir)?;

        // Write the generated metadata to the manifest
        let workspace_metadata = WorkspaceMetadata::new(splicing_manifest, installations)?;
        workspace_metadata.inject_into(&mut manifest)?;

        // Add any additional depeendencies to the root package
        Self::inject_direct_packages(&mut manifest, &splicing_manifest.direct_packages)?;

        // Write the root manifest
        let root_manifest_path = workspace_dir.join("Cargo.toml");
        write_root_manifest(&root_manifest_path, manifest)?;

        Ok(SplicedManifest::MultiPackage(root_manifest_path))
    }

    /// Extract the set of extra workspace member manifests such that it matches
    /// how other manifests are passed when creating a new [SplicerKind].
    fn get_extra_workspace_manifests(
        extra_manifests: &[ExtraManifestInfo],
    ) -> Result<HashMap<PathBuf, Manifest>> {
        extra_manifests
            .iter()
            .map(|config| match read_manifest(&config.manifest) {
                Ok(manifest) => Ok((config.manifest.clone(), manifest)),
                Err(err) => Err(err),
            })
            .collect()
    }

    /// Update the newly generated manifest to include additional packages as
    /// Cargo workspace members.
    fn inject_workspace_members<'b>(
        root_manifest: &mut Manifest,
        manifests: &'b HashMap<PathBuf, Manifest>,
        workspace_dir: &Path,
    ) -> Result<HashMap<&'b PathBuf, String>> {
        manifests
            .iter()
            .map(|(path, manifest)| {
                let package_name = &manifest
                    .package
                    .as_ref()
                    .expect("Each manifest should have a root package")
                    .name;

                root_manifest
                    .workspace
                    .as_mut()
                    .expect("The root manifest is expected to always have a workspace")
                    .members
                    .push(package_name.clone());

                let manifest_dir = path
                    .parent()
                    .expect("Every manifest should havee a parent directory");

                let dest_package_dir = workspace_dir.join(package_name);

                match symlink_roots(manifest_dir, &dest_package_dir) {
                    Ok(_) => Ok((path, package_name.clone())),
                    Err(e) => Err(e),
                }
            })
            .collect()
    }

    fn inject_direct_packages(
        manifest: &mut Manifest,
        direct_packages_manifest: &DirectPackageManifest,
    ) -> Result<()> {
        // Ensure there's a root package to satisfy Cargo requirements
        if manifest.package.is_none() {
            let new_manifest = default_cargo_package_manifest();
            manifest.package = new_manifest.package;
            if manifest.lib.is_none() {
                manifest.lib = new_manifest.lib;
            }
        }

        // Check for any duplicates
        let duplicates: Vec<&String> = manifest
            .dependencies
            .keys()
            .filter(|k| direct_packages_manifest.contains_key(*k))
            .collect();
        if !duplicates.is_empty() {
            bail!(
                "Duplications detected between manifest dependencies and direct dependencies: {:?}",
                duplicates
            )
        }

        // Add the dependencies
        for (name, details) in direct_packages_manifest.iter() {
            manifest.dependencies.insert(
                name.clone(),
                cargo_toml::Dependency::Detailed(details.clone()),
            );
        }

        Ok(())
    }
}

pub struct Splicer {
    workspace_dir: PathBuf,
    manifests: HashMap<PathBuf, Manifest>,
    splicing_manifest: SplicingManifest,
}

impl Splicer {
    pub fn new(workspace_dir: PathBuf, splicing_manifest: SplicingManifest) -> Result<Self> {
        // Load all manifests
        let manifests = splicing_manifest
            .manifests
            .iter()
            .map(|(path, _)| {
                let m = read_manifest(path)?;
                Ok((path.clone(), m))
            })
            .collect::<Result<HashMap<PathBuf, Manifest>>>()?;

        Ok(Self {
            workspace_dir,
            manifests,
            splicing_manifest,
        })
    }

    /// Build a new workspace root
    pub fn splice_workspace(&self) -> Result<SplicedManifest> {
        SplicerKind::new(&self.manifests, &self.splicing_manifest)?.splice(&self.workspace_dir)
    }
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

#[cfg(test)]
mod test {
    use super::*;

    use crate::test::*;

    use cargo_metadata::{MetadataCommand, PackageId};

    fn generate_metadata(manifest_path: &Path) -> cargo_metadata::Metadata {
        MetadataCommand::new()
            .manifest_path(manifest_path)
            .other_options(["--offline".to_owned()])
            .exec()
            .unwrap()
    }

    fn mock_cargo_toml(path: &Path, name: &str) -> cargo_toml::Manifest {
        let manifest = cargo_toml::Manifest::from_str(&textwrap::dedent(&format!(
            r#"
            [package]
            name = "{}"
            version = "0.0.1"

            [lib]
            path = "lib.rs"
            "#,
            name
        )))
        .unwrap();

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, toml::to_string(&manifest).unwrap()).unwrap();

        manifest
    }

    fn mock_extra_manifest_digest(cache_dir: &Path) -> Vec<ExtraManifestInfo> {
        vec![{
            let manifest_path = cache_dir.join("extra_pkg").join("Cargo.toml");
            mock_cargo_toml(&manifest_path, "extra_pkg");

            ExtraManifestInfo {
                manifest: manifest_path,
                url: "https://crates.io/".to_owned(),
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_owned(),
            }
        }]
    }

    /// This json object is tightly coupled to [mock_extra_manifest_digest]
    fn mock_extra_workspace_metadata() -> serde_json::Value {
        serde_json::json!({
            "cargo-bazel": {
            "package_prefixes": {},
            "sources": {
                "extra_pkg 0.0.1": {
                "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "url": "https://crates.io/"
                }
            }
            }
        })
    }

    fn mock_splicing_manifest_with_workspace() -> (SplicingManifest, tempfile::TempDir) {
        let mut splicing_manifest = SplicingManifest::default();
        let cache_dir = tempfile::tempdir().unwrap();

        // Write workspace members
        for pkg in &["sub_pkg_a", "sub_pkg_b"] {
            let manifest_path = cache_dir
                .as_ref()
                .join("root_pkg")
                .join(pkg)
                .join("Cargo.toml");
            mock_cargo_toml(&manifest_path, pkg);
        }

        // Create the root package with a workspace definition
        let manifest: cargo_toml::Manifest = toml::toml! {
            [workspace]
            members = [
                "sub_pkg_a",
                "sub_pkg_b",
            ]
            [package]
            name = "root_pkg"
            version = "0.0.1"

            [lib]
            path = "lib.rs"
        }
        .try_into()
        .unwrap();

        let manifest_path = cache_dir.as_ref().join("root_pkg").join("Cargo.toml");
        fs::create_dir_all(&manifest_path.parent().unwrap()).unwrap();
        fs::write(&manifest_path, toml::to_string(&manifest).unwrap()).unwrap();

        splicing_manifest
            .manifests
            .insert(manifest_path, Label::from_str("//:Cargo.toml").unwrap());

        (splicing_manifest, cache_dir)
    }

    fn mock_splicing_manifest_with_package() -> (SplicingManifest, tempfile::TempDir) {
        let mut splicing_manifest = SplicingManifest::default();
        let cache_dir = tempfile::tempdir().unwrap();

        // Add an additional package
        let manifest_path = cache_dir.as_ref().join("root_pkg").join("Cargo.toml");
        mock_cargo_toml(&manifest_path, "root_pkg");
        splicing_manifest
            .manifests
            .insert(manifest_path, Label::from_str("//:Cargo.toml").unwrap());

        (splicing_manifest, cache_dir)
    }

    fn mock_splicing_manifest_with_multi_package() -> (SplicingManifest, tempfile::TempDir) {
        let mut splicing_manifest = SplicingManifest::default();
        let cache_dir = tempfile::tempdir().unwrap();

        // Add an additional package
        for pkg in &["pkg_a", "pkg_b", "pkg_c"] {
            let manifest_path = cache_dir.as_ref().join(pkg).join("Cargo.toml");
            mock_cargo_toml(&manifest_path, pkg);
            splicing_manifest
                .manifests
                .insert(manifest_path, Label::from_str("//:Cargo.toml").unwrap());
        }

        (splicing_manifest, cache_dir)
    }

    fn new_package_id(name: &str, workspace_root: &Path, is_root: bool) -> PackageId {
        if is_root {
            PackageId {
                repr: format!("{} 0.0.1 (path+file://{})", name, workspace_root.display()),
            }
        } else {
            PackageId {
                repr: format!(
                    "{} 0.0.1 (path+file://{}/{})",
                    name,
                    workspace_root.display(),
                    name,
                ),
            }
        }
    }

    #[test]
    fn splice_workspace() {
        let (splicing_manifest, _cache_dir) = mock_splicing_manifest_with_workspace();

        // Splice the workspace
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace_manifest =
            Splicer::new(workspace_root.as_ref().to_path_buf(), splicing_manifest)
                .unwrap()
                .splice_workspace()
                .unwrap();

        // Ensure metadata is valid
        let metadata = generate_metadata(workspace_manifest.as_path_buf());
        assert_sort_eq!(
            metadata.workspace_members,
            vec![
                new_package_id("sub_pkg_a", workspace_root.as_ref(), false),
                new_package_id("sub_pkg_b", workspace_root.as_ref(), false),
                new_package_id("root_pkg", workspace_root.as_ref(), true),
            ]
        );

        // Ensure the workspace metadata annotations are populated
        assert_eq!(metadata.workspace_metadata, serde_json::Value::Null,);

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }

    #[test]
    fn splice_package() {
        let (splicing_manifest, _cache_dir) = mock_splicing_manifest_with_package();

        // Splice the workspace
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace_manifest =
            Splicer::new(workspace_root.as_ref().to_path_buf(), splicing_manifest)
                .unwrap()
                .splice_workspace()
                .unwrap();

        // Ensure metadata is valid
        let metadata = generate_metadata(workspace_manifest.as_path_buf());
        assert_sort_eq!(
            metadata.workspace_members,
            vec![new_package_id("root_pkg", workspace_root.as_ref(), true)]
        );

        // Ensure the workspace metadata annotations are not populated
        assert_eq!(metadata.workspace_metadata, serde_json::Value::Null);

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }

    #[test]
    fn splice_multi_package() {
        let (splicing_manifest, _cache_dir) = mock_splicing_manifest_with_multi_package();

        // Splice the workspace
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace_manifest =
            Splicer::new(workspace_root.as_ref().to_path_buf(), splicing_manifest)
                .unwrap()
                .splice_workspace()
                .unwrap();

        // Ensure metadata is valid
        let metadata = generate_metadata(workspace_manifest.as_path_buf());
        assert_sort_eq!(
            metadata.workspace_members,
            vec![
                new_package_id("pkg_a", workspace_root.as_ref(), false),
                new_package_id("pkg_b", workspace_root.as_ref(), false),
                new_package_id("pkg_c", workspace_root.as_ref(), false),
                // Multi package renderings always add a root package
                new_package_id("direct-cargo-bazel-deps", workspace_root.as_ref(), true),
            ]
        );

        // Ensure the workspace metadata annotations are populated
        assert_eq!(metadata.workspace_metadata, serde_json::Value::Null);

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }

    #[test]
    fn extra_workspace_member_with_package() {
        let (mut splicing_manifest, cache_dir) = mock_splicing_manifest_with_package();

        // Add the extra workspace member
        splicing_manifest.extra_manifest_infos = mock_extra_manifest_digest(cache_dir.as_ref());

        // Splice the workspace
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace_manifest =
            Splicer::new(workspace_root.as_ref().to_path_buf(), splicing_manifest)
                .unwrap()
                .splice_workspace()
                .unwrap();

        // Ensure metadata is valid
        let metadata = generate_metadata(workspace_manifest.as_path_buf());
        assert_sort_eq!(
            metadata.workspace_members,
            vec![
                new_package_id("extra_pkg", workspace_root.as_ref(), false),
                new_package_id("root_pkg", workspace_root.as_ref(), true),
            ]
        );

        // Ensure the workspace metadata annotations are populated
        assert_eq!(metadata.workspace_metadata, mock_extra_workspace_metadata());

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }

    #[test]
    fn extra_workspace_member_with_workspace() {
        let (mut splicing_manifest, cache_dir) = mock_splicing_manifest_with_workspace();

        // Add the extra workspace member
        splicing_manifest.extra_manifest_infos = mock_extra_manifest_digest(cache_dir.as_ref());

        // Splice the workspace
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace_manifest =
            Splicer::new(workspace_root.as_ref().to_path_buf(), splicing_manifest)
                .unwrap()
                .splice_workspace()
                .unwrap();

        // Ensure metadata is valid
        let metadata = generate_metadata(workspace_manifest.as_path_buf());
        assert_sort_eq!(
            metadata.workspace_members,
            vec![
                new_package_id("sub_pkg_a", workspace_root.as_ref(), false),
                new_package_id("sub_pkg_b", workspace_root.as_ref(), false),
                new_package_id("extra_pkg", workspace_root.as_ref(), false),
                new_package_id("root_pkg", workspace_root.as_ref(), true),
            ]
        );

        // Ensure the workspace metadata annotations are populated
        assert_eq!(metadata.workspace_metadata, mock_extra_workspace_metadata());

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }

    #[test]
    fn extra_workspace_member_with_multi_package() {
        let (mut splicing_manifest, cache_dir) = mock_splicing_manifest_with_multi_package();

        // Add the extra workspace member
        splicing_manifest.extra_manifest_infos = mock_extra_manifest_digest(cache_dir.as_ref());

        // Splice the workspace
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace_manifest =
            Splicer::new(workspace_root.as_ref().to_path_buf(), splicing_manifest)
                .unwrap()
                .splice_workspace()
                .unwrap();

        // Ensure metadata is valid
        let metadata = generate_metadata(workspace_manifest.as_path_buf());
        assert_sort_eq!(
            metadata.workspace_members,
            vec![
                new_package_id("pkg_a", workspace_root.as_ref(), false),
                new_package_id("pkg_b", workspace_root.as_ref(), false),
                new_package_id("pkg_c", workspace_root.as_ref(), false),
                new_package_id("extra_pkg", workspace_root.as_ref(), false),
                // Multi package renderings always add a root package
                new_package_id("direct-cargo-bazel-deps", workspace_root.as_ref(), true),
            ]
        );

        // Ensure the workspace metadata annotations are populated
        assert_eq!(metadata.workspace_metadata, mock_extra_workspace_metadata());

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }
}
