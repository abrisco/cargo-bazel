//! Collect and store information from Cargo metadata specific to Bazel's needs

pub mod dependency;

use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::path::PathBuf;

use anyhow::{bail, Result};
use cargo_metadata::{Node, Package, PackageId};
use hex::ToHex;
use serde::{Deserialize, Serialize};

use crate::config::{Commitish, Config, CrateExtras, CrateId};
use crate::splicing::{SourceInfo, WorkspaceMetadata};

use self::dependency::DependencySet;

pub type CargoMetadata = cargo_metadata::Metadata;
pub type CargoLockfile = cargo_lock::Lockfile;

#[derive(Debug, Serialize, Deserialize)]
pub struct CrateAnnotation {
    pub node: Node,
    pub deps: DependencySet,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MetadataAnnotation {
    pub packages: BTreeMap<PackageId, Package>,
    pub crates: BTreeMap<PackageId, CrateAnnotation>,
    pub workspace_members: BTreeSet<PackageId>,
    pub workspace_root: PathBuf,
    pub workspace_metadata: WorkspaceMetadata,
}

impl MetadataAnnotation {
    pub fn new(metadata: CargoMetadata) -> MetadataAnnotation {
        // UNWRAP: The workspace metadata should be written by a controlled process. This should not return a result
        let workspace_metadata = find_workspace_metadata(&metadata).unwrap_or_default();

        let resolve = metadata
            .resolve
            .as_ref()
            .expect("The metadata provided requires a resolve graph")
            .clone();

        let is_node_workspace_member = |node: &Node, metadata: &CargoMetadata| -> bool {
            metadata.workspace_members.iter().any(|pkg| pkg == &node.id)
        };

        let workspace_members: BTreeSet<PackageId> = resolve
            .nodes
            .iter()
            .filter(|node| is_node_workspace_member(node, &metadata))
            .map(|node| node.id.clone())
            .collect();

        let crates = resolve
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    Self::annotate_crate(node.clone(), &metadata),
                )
            })
            .collect();

        let packages = metadata
            .packages
            .into_iter()
            .map(|pkg| (pkg.id.clone(), pkg))
            .collect();

        MetadataAnnotation {
            packages,
            crates,
            workspace_members,
            workspace_root: PathBuf::from(metadata.workspace_root.as_std_path()),
            workspace_metadata,
        }
    }

    fn annotate_crate(node: Node, metadata: &CargoMetadata) -> CrateAnnotation {
        // Gather all dependencies
        let deps = DependencySet::new_for_node(&node, metadata);

        CrateAnnotation { node, deps }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SourceAnnotation {
    Git {
        remote: String,
        commitish: Commitish,
        shallow_since: Option<String>,
    },
    Http {
        url: String,
        sha256: Option<String>,
    },
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct LockfileAnnotation {
    pub crates: BTreeMap<PackageId, SourceAnnotation>,
}

impl LockfileAnnotation {
    pub fn new(lockfile: CargoLockfile, metadata: &CargoMetadata) -> Result<Self> {
        let workspace_metadata = find_workspace_metadata(metadata).unwrap_or_default();

        let nodes: Vec<&Node> = metadata
            .resolve
            .as_ref()
            .expect("Metadata is expected to have a resolve graph")
            .nodes
            .iter()
            .filter(|node| !is_workspace_member(&node.id, metadata))
            .collect();

        // Produce source annotations for each crate in the resolve graph
        let crates = nodes
            .iter()
            .map(|node| {
                Ok((
                    node.id.clone(),
                    Self::collect_source_annotations(
                        node,
                        metadata,
                        &lockfile,
                        &workspace_metadata,
                    )?,
                ))
            })
            .collect::<Result<BTreeMap<PackageId, SourceAnnotation>>>()?;

        Ok(Self { crates })
    }

    /// Resolve all URLs and checksum-like data for each package
    fn collect_source_annotations(
        node: &Node,
        metadata: &CargoMetadata,
        lockfile: &CargoLockfile,
        workspace_metadata: &WorkspaceMetadata,
    ) -> Result<SourceAnnotation> {
        let pkg = &metadata[&node.id];

        // Locate the matching lock package for the current crate
        let lock_pkg = match cargo_meta_pkg_to_locked_pkg(pkg, &lockfile.packages) {
            Some(lock_pkg) => lock_pkg,
            None => bail!(
                "Could not find lockfile entry matching metadata package '{}'",
                pkg.name
            ),
        };

        // Check for spliced information about a crate's network source.
        let spliced_source_info = Self::find_source_annotation(lock_pkg, workspace_metadata);

        // Parse it's source info. The check above should prevent a panic
        let source = match lock_pkg.source.as_ref() {
            Some(source) => source,
            None => match spliced_source_info {
                Some(info) => {
                    return Ok(SourceAnnotation::Http {
                        url: info.url,
                        sha256: Some(info.sha256),
                    })
                }
                None => bail!(
                    "The package '{:?} {:?}' has no source info so no annotation can be made",
                    lock_pkg.name,
                    lock_pkg.version
                ),
            },
        };

        // Handle any git repositories
        if let Some(git_ref) = source.git_reference() {
            return Ok(SourceAnnotation::Git {
                remote: source.url().to_string(),
                commitish: Commitish::from(git_ref.clone()),
                shallow_since: None,
            });
        }

        // One of the last things that should be checked is the spliced source information as
        // other sources may more accurately represent where a crate should be downloaded.
        if let Some(info) = spliced_source_info {
            return Ok(SourceAnnotation::Http {
                url: info.url,
                sha256: Some(info.sha256),
            });
        }

        // Finally, In the event that no spliced source information was included in the
        // metadata the raw source info is used for registry crates and `crates.io` is
        // assumed to be the source.
        if source.is_registry() {
            return Ok(SourceAnnotation::Http {
                url: format!(
                    "https://crates.io/api/v1/crates/{}/{}/download",
                    lock_pkg.name.to_string(),
                    lock_pkg.version.to_string()
                ),
                sha256: lock_pkg
                    .checksum
                    .as_ref()
                    .and_then(|sum| {
                        if sum.is_sha256() {
                            sum.as_sha256()
                        } else {
                            None
                        }
                    })
                    .map(|sum| sum.encode_hex::<String>()),
            });
        }

        bail!(
            "Unable to determine source annotation for '{:?} {:?}",
            lock_pkg.name,
            lock_pkg.version
        )
    }

    fn find_source_annotation(
        package: &cargo_lock::Package,
        metadata: &WorkspaceMetadata,
    ) -> Option<SourceInfo> {
        let crate_id = CrateId::new(package.name.to_string(), package.version.to_string());
        metadata.sources.get(&crate_id).cloned()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairredExtras {
    pub package_id: cargo_metadata::PackageId,
    pub crate_extra: CrateExtras,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Annotations {
    pub metadata: MetadataAnnotation,
    pub lockfile: LockfileAnnotation,
    pub config: Config,
    pub pairred_extras: BTreeMap<CrateId, PairredExtras>,
}

impl Annotations {
    pub fn new(
        cargo_metadata: CargoMetadata,
        cargo_lockfile: CargoLockfile,
        config: Config,
    ) -> Result<Self> {
        let lockfile_annotation = LockfileAnnotation::new(cargo_lockfile, &cargo_metadata)?;

        // Annotate the cargo metadata
        let metadata_annotation = MetadataAnnotation::new(cargo_metadata);

        // Ensure each override matches a particular package
        // TODO: There should probably be a warning here about 'extras'
        // that were not matched with anything
        let pairred_extras = metadata_annotation
            .packages
            .iter()
            .filter_map(|(pkg_id, pkg)| {
                let extras: Vec<CrateExtras> = config
                    .extras
                    .iter()
                    .filter(|(id, _)| id.matches(pkg))
                    .map(|(_, extra)| extra)
                    .cloned()
                    .collect();

                if !extras.is_empty() {
                    Some((
                        CrateId::new(pkg.name.clone(), pkg.version.to_string()),
                        PairredExtras {
                            package_id: pkg_id.clone(),
                            crate_extra: extras.into_iter().sum(),
                        },
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Annotate metadata
        Ok(Annotations {
            metadata: metadata_annotation,
            lockfile: lockfile_annotation,
            config,
            pairred_extras,
        })
    }
}

fn find_workspace_metadata(cargo_metadata: &CargoMetadata) -> Option<WorkspaceMetadata> {
    WorkspaceMetadata::try_from(cargo_metadata.workspace_metadata.clone()).ok()
}

/// Determines whether or not a package is a workspace member. This follows
/// the Cargo definition of a workspace memeber with one exception where
/// "extra workspace members" are *not* treated as workspace members
fn is_workspace_member(id: &PackageId, cargo_metadata: &CargoMetadata) -> bool {
    if cargo_metadata.workspace_members.contains(id) {
        if let Some(data) = find_workspace_metadata(cargo_metadata) {
            let pkg = &cargo_metadata[id];
            let crate_id = CrateId::new(pkg.name.clone(), pkg.version.to_string());

            !data.sources.contains_key(&crate_id)
        } else {
            true
        }
    } else {
        false
    }
}

/// Match a [cargo_metadata::Package] to a [cargo_lock::Package].
fn cargo_meta_pkg_to_locked_pkg<'a>(
    pkg: &Package,
    lock_packages: &'a [cargo_lock::Package],
) -> Option<&'a cargo_lock::Package> {
    lock_packages
        .iter()
        .find(|lock_pkg| lock_pkg.name.as_str() == pkg.name && lock_pkg.version == pkg.version)
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::test::*;

    #[test]
    fn test_cargo_meta_pkg_to_locked_pkg() {
        let pkg = mock_cargo_metadata_package();
        let lock_pkg = mock_cargo_lock_package();

        assert!(cargo_meta_pkg_to_locked_pkg(&pkg, &vec![lock_pkg]).is_some())
    }

    #[test]
    fn annotate_metadata_with_aliases() {
        let annotations = MetadataAnnotation::new(test::metadata::alias());
        let log_crates: BTreeMap<&PackageId, &CrateAnnotation> = annotations
            .crates
            .iter()
            .filter(|(id, _)| {
                let pkg = &annotations.packages[*id];
                pkg.name == "log"
            })
            .collect();

        assert_eq!(log_crates.len(), 2);
    }

    #[test]
    fn annotate_metadata_with_build_scripts() {
        MetadataAnnotation::new(test::metadata::build_scripts());
    }

    #[test]
    fn annotate_metadata_with_no_deps() {
        MetadataAnnotation::new(test::metadata::no_deps());
    }
}
