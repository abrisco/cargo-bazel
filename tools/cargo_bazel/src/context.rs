//! Convert annotated metadata into a renderable context

pub mod crate_context;
mod platforms;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::annotation::Annotations;
use crate::config::CrateId;
use crate::context::crate_context::{CrateContext, CrateDependency, Rule};
use crate::context::platforms::resolve_cfg_platforms;
use crate::digest::Digest;
use crate::utils::starlark::{Select, SelectList};

pub use self::crate_context::*;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Context {
    /// The collective checksum of all inputs to the context
    pub checksum: Option<Digest>,

    pub crates: BTreeMap<CrateId, CrateContext>,

    pub binary_crates: BTreeSet<CrateId>,

    pub workspace_members: BTreeMap<CrateId, String>,

    pub conditions: BTreeMap<String, BTreeSet<String>>,
}

impl Context {
    pub fn try_from_path<T: AsRef<Path>>(path: T) -> Result<Self> {
        let data = fs::read_to_string(path.as_ref())?;
        Ok(serde_json::from_str(&data)?)
    }

    pub fn new(annotations: Annotations, cargo_bin: &Path, rustc_bin: &Path) -> Result<Self> {
        // Build a map of crate contexts
        let crates: BTreeMap<CrateId, CrateContext> = annotations
            .metadata
            .crates
            .iter()
            // Convert the crate annotations into more renderable contexts
            .map(|(_, annotation)| {
                let context = CrateContext::new(
                    annotation,
                    &annotations.metadata.packages,
                    &annotations.lockfile.crates,
                    &annotations.pairred_extras,
                    annotations.config.generate_build_scripts,
                );
                let id = CrateId::new(context.name.clone(), context.version.clone());
                (id, context)
            })
            .collect();

        // Filter for any crate that contains a binary
        let binary_crates = crates
            .iter()
            .filter(|(_, ctx)| ctx.targets.iter().any(|t| matches!(t, Rule::Binary(..))))
            .filter(|(_, ctx)| ctx.repository.is_some())
            .map(|(id, _)| id.clone())
            .collect();

        // Given a list of all conditional dependencies, build a set of platform
        // triples which satsify the conditions.
        let conditions = resolve_cfg_platforms(
            crates.values().collect(),
            &annotations.config.supported_platform_triples,
        )?;

        // Generate a list of all workspace members
        let workspace_members = annotations
            .metadata
            .workspace_members
            .iter()
            .filter_map(|id| {
                let pkg = &annotations.metadata.packages[id];
                let package_path_id = match Self::get_package_path_id(
                    pkg,
                    &annotations.metadata.workspace_root,
                    &annotations.metadata.workspace_metadata.workspace_prefix,
                    &annotations.metadata.workspace_metadata.package_prefixes,
                ) {
                    Ok(id) => id,
                    Err(e) => return Some(Err(e)),
                };
                let crate_id = CrateId::new(pkg.name.clone(), pkg.version.to_string());

                // Crates that have repository information are not considered workspace members.
                // The assumpion is that they are "extra workspace members".
                match crates[&crate_id].repository {
                    Some(_) => None,
                    None => Some(Ok((crate_id, package_path_id))),
                }
            })
            .collect::<Result<BTreeMap<CrateId, String>>>()?;

        let checksum = Some(Digest::new(&annotations.config, cargo_bin, rustc_bin)?);

        Ok(Self {
            checksum,
            crates,
            binary_crates,
            workspace_members,
            conditions,
        })
    }

    // A helper function for locating the unique path in a workspace to a workspace member
    fn get_package_path_id(
        package: &cargo_metadata::Package,
        workspace_root: &Path,
        workspace_prefix: &Option<String>,
        package_prefixes: &BTreeMap<String, String>,
    ) -> Result<String> {
        // Locate the package's manifest directory
        let manifest_dir = package
            .manifest_path
            .parent()
            .expect("Every manifest should have a parent")
            .as_std_path();

        // Compare it with the root of the workspace
        let package_path_diff = pathdiff::diff_paths(manifest_dir, workspace_root)
            .expect("Every workspace member's manifest is a child of the workspace root");

        // Ensure the package paths are adjusted in the macros according to the splicing results
        let package_path = match package_prefixes.get(&package.name) {
            // Any package prefix should be absolute and therefore always applied
            Some(prefix) => PathBuf::from(prefix).join(package_path_diff),
            // If no package prefix is present, attempt to apply the workspace prefix
            // since workspace members would not have shown up with their own label
            None => match workspace_prefix {
                Some(prefix) => PathBuf::from(prefix).join(package_path_diff),
                None => package_path_diff,
            },
        };

        // Sanitize the path for increased consistency
        let package_path_id = package_path
            .display()
            .to_string()
            .replace('\\', "/")
            .trim_matches('/')
            .to_owned();

        Ok(package_path_id)
    }

    /// Filter a crate's dependencies to only ones with aliases
    pub fn crate_aliases(
        &self,
        crate_id: &CrateId,
        build: bool,
        include_dev: bool,
    ) -> SelectList<&CrateDependency> {
        let ctx = &self.crates[crate_id];
        let mut set = SelectList::default();

        // Return a set of aliases for build dependencies
        // vs normal dependencies when requested.
        if build {
            // Note that there may not be build dependencies so no dependencies
            // will be gathered in this case
            if let Some(attrs) = &ctx.build_script_attrs {
                let collection: Vec<(Option<String>, &CrateDependency)> = attrs
                    .deps
                    .configurations()
                    .into_iter()
                    .flat_map(move |conf| {
                        attrs
                            .deps
                            .get_iter(conf)
                            .expect("Iterating over known keys should never panic")
                            .filter(|dep| dep.alias.is_some())
                            .map(move |dep| (conf.cloned(), dep))
                    })
                    .chain(attrs.proc_macro_deps.configurations().into_iter().flat_map(
                        move |conf| {
                            attrs
                                .proc_macro_deps
                                .get_iter(conf)
                                .expect("Iterating over known keys should never panic")
                                .filter(|dep| dep.alias.is_some())
                                .map(move |dep| (conf.cloned(), dep))
                        },
                    ))
                    .collect();

                for (config, dep) in collection {
                    set.insert(dep, config);
                }
            }
        } else {
            let attrs = &ctx.common_attrs;
            let mut collection: Vec<(Option<String>, &CrateDependency)> =
                attrs
                    .deps
                    .configurations()
                    .into_iter()
                    .flat_map(move |conf| {
                        attrs
                            .deps
                            .get_iter(conf)
                            .expect("Iterating over known keys should never panic")
                            .filter(|dep| dep.alias.is_some())
                            .map(move |dep| (conf.cloned(), dep))
                    })
                    .chain(attrs.proc_macro_deps.configurations().into_iter().flat_map(
                        move |conf| {
                            attrs
                                .proc_macro_deps
                                .get_iter(conf)
                                .expect("Iterating over known keys should never panic")
                                .filter(|dep| dep.alias.is_some())
                                .map(move |dep| (conf.cloned(), dep))
                        },
                    ))
                    .collect();

            // Optionally include dev dependencies
            if include_dev {
                collection = collection
                    .into_iter()
                    .chain(
                        attrs
                            .deps_dev
                            .configurations()
                            .into_iter()
                            .flat_map(move |conf| {
                                attrs
                                    .deps_dev
                                    .get_iter(conf)
                                    .expect("Iterating over known keys should never panic")
                                    .filter(|dep| dep.alias.is_some())
                                    .map(move |dep| (conf.cloned(), dep))
                            }),
                    )
                    .chain(
                        attrs
                            .proc_macro_deps_dev
                            .configurations()
                            .into_iter()
                            .flat_map(move |conf| {
                                attrs
                                    .proc_macro_deps_dev
                                    .get_iter(conf)
                                    .expect("Iterating over known keys should never panic")
                                    .filter(|dep| dep.alias.is_some())
                                    .map(move |dep| (conf.cloned(), dep))
                            }),
                    )
                    .collect();
            }

            for (config, dep) in collection {
                set.insert(dep, config);
            }
        }

        set
    }

    pub fn flat_workspace_member_deps(&self) -> (Vec<CrateId>, BTreeMap<CrateId, String>) {
        let mut workspace_member_dependencies: Vec<CrateId> = self
            .workspace_members
            .iter()
            .map(|(id, _)| &self.crates[id])
            .flat_map(|ctx| {
                // Build an interator of all dependency CrateIds.
                // TODO: This expansion is horribly verbose and should be refactored but closures
                // were not playing nice when I tried it.
                ctx.common_attrs
                    .deps
                    .configurations()
                    .into_iter()
                    .flat_map(move |conf| {
                        ctx.common_attrs
                            .deps
                            .get_iter(conf)
                            .expect("Lookup should be guaranteed")
                    })
                    .chain(
                        ctx.common_attrs
                            .deps_dev
                            .configurations()
                            .into_iter()
                            .flat_map(move |conf| {
                                ctx.common_attrs
                                    .deps_dev
                                    .get_iter(conf)
                                    .expect("Lookup should be guaranteed")
                            }),
                    )
                    .chain(
                        ctx.common_attrs
                            .proc_macro_deps
                            .configurations()
                            .into_iter()
                            .flat_map(move |conf| {
                                ctx.common_attrs
                                    .proc_macro_deps
                                    .get_iter(conf)
                                    .expect("Lookup should be guaranteed")
                            }),
                    )
                    .chain(
                        ctx.common_attrs
                            .proc_macro_deps_dev
                            .configurations()
                            .into_iter()
                            .flat_map(move |conf| {
                                ctx.common_attrs
                                    .proc_macro_deps_dev
                                    .get_iter(conf)
                                    .expect("Lookup should be guaranteed")
                            }),
                    )
            })
            .map(|dep_set| &dep_set.id)
            .cloned()
            .collect();

        // Deduplicate entries in the set
        let mut uniques = HashSet::new();
        workspace_member_dependencies.retain(|e| uniques.insert(e.clone()));
        workspace_member_dependencies.sort();

        // Some dependencies appear multiple times in a workspace where two different crates have
        // pins for different versions. In order to correctly render all aliases, an additional
        // map is returned to indicate which crates are duplicates. The UX here is kinda undesirable
        // since the solution here writes `{crate_name}` as `{crate_name}-{crate_version}`. This means
        // users will be writing versions in their BUILD files which they'll need to change if they
        // update the pin __or__ remove one of the duplicates. Ideally users would use common pins
        // but at least this allows for this use case.
        let duplicate_deps: BTreeMap<CrateId, String> = workspace_member_dependencies
            .iter()
            .filter_map(|crate_id| {
                let is_duplicate = workspace_member_dependencies
                    .iter()
                    .filter(|id| id.name == crate_id.name)
                    .count()
                    > 1;
                if is_duplicate {
                    Some((
                        crate_id.clone(),
                        format!("{}-{}", &crate_id.name, &crate_id.version),
                    ))
                } else {
                    None
                }
            })
            .collect();

        (workspace_member_dependencies, duplicate_deps)
    }
}
