use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use cargo_toml::{Dependency, Manifest};

use crate::splicing::{SplicedManifest, SplicingManifest};

use super::{read_manifest, DirectPackageManifest, ExtraManifestInfo, WorkspaceMetadata};

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

/// A list of files or directories to ignore when when symlinking
const IGNORE_LIST: &[&str] = &[".git", "bazel-bin", "bazel-out", ".svn"];

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
        symlink_roots(manifest_dir, workspace_dir, Some(IGNORE_LIST))?;

        // Optionally install the cargo config after contents have been symlinked
        Self::setup_cargo_config(&splicing_manifest.cargo_config, workspace_dir)?;

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
        symlink_roots(manifest_dir, workspace_dir, Some(IGNORE_LIST))?;

        // Optionally install the cargo config after contents have been symlinked
        Self::setup_cargo_config(&splicing_manifest.cargo_config, workspace_dir)?;

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

        // Optionally install a cargo config file into the workspace root.
        Self::setup_cargo_config(&splicing_manifest.cargo_config, workspace_dir)?;

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

    /// A helper for installing Cargo config files into the spliced workspace while also
    /// ensuring no other linked config file is available
    fn setup_cargo_config(cargo_config_path: &Option<PathBuf>, workspace_dir: &Path) -> Result<()> {
        // Make sure no other config files exist
        for config in vec![
            workspace_dir.join("config"),
            workspace_dir.join("config.toml"),
        ] {
            if config.exists() {
                fs::remove_file(&config).with_context(|| {
                    format!(
                        "Failed to delete existing cargo config: {}",
                        config.display()
                    )
                })?;
            }
        }

        // If the `.cargo` dir is a symlink, we'll need to relink it and ensure
        // a Cargo config file is omitted
        let dot_cargo_dir = workspace_dir.join(".cargo");
        if dot_cargo_dir.exists() {
            let is_symlink = dot_cargo_dir
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if is_symlink {
                let real_path = dot_cargo_dir.canonicalize()?;
                remove_symlink(&dot_cargo_dir).with_context(|| {
                    format!(
                        "Failed to remove existing symlink {}",
                        dot_cargo_dir.display()
                    )
                })?;
                fs::create_dir(&dot_cargo_dir)?;
                symlink_roots(&real_path, &dot_cargo_dir, Some(&["config", "config.toml"]))?;
            } else {
                for config in vec![
                    dot_cargo_dir.join("config"),
                    dot_cargo_dir.join("config.toml"),
                ] {
                    if config.exists() {
                        fs::remove_file(&config)?;
                    }
                }
            }
        }

        // Install the new config file after having removed all others
        if let Some(cargo_config_path) = cargo_config_path {
            let install_path = workspace_dir.join(".cargo").join("config.toml");
            if !install_path.parent().unwrap().exists() {
                fs::create_dir_all(&install_path.parent().unwrap())?;
            }

            fs::copy(cargo_config_path, &install_path)?;
        }

        Ok(())
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

                match symlink_roots(manifest_dir, &dest_package_dir, Some(IGNORE_LIST)) {
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

/// Create a symlink file on unix systems
#[cfg(target_family = "unix")]
fn remove_symlink(path: &Path) -> Result<(), std::io::Error> {
    fs::remove_file(path)
}

/// Create a symlink file on windows systems
#[cfg(target_family = "windows")]
fn remove_symlink(path: &Path) -> Result<(), std::io::Error> {
    if path.is_dir() {
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

/// Symlinks the root contents of a source directory into a destination directory
pub fn symlink_roots(source: &Path, dest: &Path, ignore_list: Option<&[&str]>) -> Result<()> {
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
            if let Some(list) = ignore_list {
                if list.contains(&base_str) {
                    continue;
                }
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

#[cfg(test)]
mod test {
    use super::*;

    use std::fs;
    use std::str::FromStr;

    use cargo_metadata::{MetadataCommand, PackageId};

    use crate::splicing::ExtraManifestInfo;
    use crate::test::*;
    use crate::utils::starlark::Label;

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
    fn mock_workspace_metadata(include_extra_member: bool) -> serde_json::Value {
        if include_extra_member {
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
        } else {
            serde_json::json!({
                "cargo-bazel": {
                    "package_prefixes": {},
                    "sources": {}
                }
            })
        }
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
        assert_eq!(metadata.workspace_metadata, mock_workspace_metadata(false));

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
        assert_eq!(metadata.workspace_metadata, mock_workspace_metadata(false));

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
        assert_eq!(metadata.workspace_metadata, mock_workspace_metadata(false));

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
        assert_eq!(metadata.workspace_metadata, mock_workspace_metadata(true));

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
        assert_eq!(metadata.workspace_metadata, mock_workspace_metadata(true));

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
        assert_eq!(metadata.workspace_metadata, mock_workspace_metadata(true));

        // Ensure lockfile was successfully spliced
        cargo_lock::Lockfile::load(workspace_root.as_ref().join("Cargo.lock")).unwrap();
    }
}
