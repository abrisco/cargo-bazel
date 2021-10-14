//! TODO

use std::collections::{BTreeMap, BTreeSet};

use cargo_metadata::{Node, Package, PackageId};
use serde::{Deserialize, Serialize};

use crate::annotation::dependency::Dependency;
use crate::annotation::{CrateAnnotation, PairredExtras, SourceAnnotation};
use crate::config::CrateId;
use crate::utils::sanitize_module_name;
use crate::utils::starlark::{Glob, SelectList, SelectMap, SelectStringDict, SelectStringList};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone)]
pub struct CrateDependency {
    /// The [CrateId] of the dependency
    pub id: CrateId,

    /// The target name of the dependency. Note this may differ from the
    /// dependency's package name in cases such as build scripts.
    pub target: String,

    /// Some dependencies are assigned aliases. This is tracked here
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetAttributes {
    /// The module name of the crate (notably, not the package name).
    pub crate_name: String,

    /// The path to the crate's root source file, relative to the manifest.
    pub crate_root: Option<String>,

    /// A glob pattern of all source files required by the target
    pub srcs: Glob,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rule {
    /// `cargo_build_script`
    BuildScript(TargetAttributes),

    /// `rust_proc_macro`
    ProcMacro(TargetAttributes),

    /// `rust_library`
    Library(TargetAttributes),

    /// `rust_binary`
    Binary(TargetAttributes),
}

/// A set of attributes common to most `rust_library`, `rust_proc_macro`, and other
/// [core rules of `rules_rust`](https://bazelbuild.github.io/rules_rust/defs.html).
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(default)]
pub struct CommonAttributes {
    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub compile_data: SelectStringList,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub compile_data_glob: BTreeSet<String>,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub crate_features: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub data: SelectStringList,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub data_glob: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectList::should_skip_serializing")]
    pub deps: SelectList<CrateDependency>,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub extra_deps: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectList::should_skip_serializing")]
    pub deps_dev: SelectList<CrateDependency>,

    pub edition: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub linker_script: Option<String>,

    #[serde(skip_serializing_if = "SelectList::should_skip_serializing")]
    pub proc_macro_deps: SelectList<CrateDependency>,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub extra_proc_macro_deps: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectList::should_skip_serializing")]
    pub proc_macro_deps_dev: SelectList<CrateDependency>,

    #[serde(skip_serializing_if = "SelectStringDict::should_skip_serializing")]
    pub rustc_env: SelectStringDict,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub rustc_env_files: SelectStringList,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub rustc_flags: SelectStringList,

    pub version: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

// Build script attributes. See
// https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildScriptAttributes {
    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub compile_data: SelectStringList,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub data: SelectStringList,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub data_glob: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectList::should_skip_serializing")]
    pub deps: SelectList<CrateDependency>,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub extra_deps: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectStringDict::should_skip_serializing")]
    pub build_script_env: SelectStringDict,

    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub extra_proc_macro_deps: BTreeSet<String>,

    #[serde(skip_serializing_if = "SelectList::should_skip_serializing")]
    pub proc_macro_deps: SelectList<CrateDependency>,

    #[serde(skip_serializing_if = "SelectStringDict::should_skip_serializing")]
    pub rustc_env: SelectStringDict,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub rustc_flags: SelectStringList,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub rustc_env_files: SelectStringList,

    #[serde(skip_serializing_if = "SelectStringList::should_skip_serializing")]
    pub tools: SelectStringList,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(default)]
pub struct CrateContext {
    /// The package name of the current crate
    pub name: String,

    /// The full version of the current crate
    pub version: String,

    /// Optional source annotations if they were discoverable in the
    /// lockfile. Workspace Members will not have source annotations and
    /// potentially others.
    pub repository: Option<SourceAnnotation>,

    /// A list of all targets (lib, proc-macro, bin) associated with this package
    pub targets: Vec<Rule>,

    /// The name of the crate's root library target. This is the target that a dependent
    /// would get if they were to depend on `{crate_name}`.
    pub library_target_name: Option<String>,

    /// A set of attributes common to most [Rule] types or target types.
    pub common_attrs: CommonAttributes,

    /// Optional attributes for build scripts. This field is only populated if
    /// a build script (`custom-build`) target is defined for the crate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_script_attrs: Option<BuildScriptAttributes>,

    /// The license used by the crate
    pub license: Option<String>,

    /// Additional text to add to the generated BUILD file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_build_contents: Option<String>,
}

impl CrateContext {
    pub fn new(
        annotation: &CrateAnnotation,
        packages: &BTreeMap<PackageId, Package>,
        source_annotations: &BTreeMap<PackageId, SourceAnnotation>,
        extras: &BTreeMap<CrateId, PairredExtras>,
        include_build_scripts: bool,
    ) -> Self {
        let package: &Package = &packages[&annotation.node.id];
        let current_crate_id = CrateId::new(package.name.clone(), package.version.to_string());

        let new_crate_dep = |dep: Dependency| -> CrateDependency {
            let pkg = &packages[&dep.package_id];

            // Unfortunately, The package graph and resolve graph of cargo metadata have different representations
            // for the crate names (resolve graph sanitizes names to match module names) so to get the rest of this
            // content to align when rendering, the dependency target needs to be explicitly sanitized.
            let target = sanitize_module_name(&dep.target_name);

            CrateDependency {
                id: CrateId::new(pkg.name.clone(), pkg.version.to_string()),
                target,
                alias: dep.alias,
            }
        };

        // Convert the dependencies into renderable strings
        let deps = annotation.deps.normal_deps.clone().map(new_crate_dep);
        let deps_dev = annotation.deps.normal_dev_deps.clone().map(new_crate_dep);
        let proc_macro_deps = annotation.deps.proc_macro_deps.clone().map(new_crate_dep);
        let proc_macro_deps_dev = annotation
            .deps
            .proc_macro_dev_deps
            .clone()
            .map(new_crate_dep);

        // Gather all "common" attributes
        let mut common_attrs = CommonAttributes {
            crate_features: annotation.node.features.iter().cloned().collect(),
            deps,
            deps_dev,
            edition: package.edition.clone(),
            proc_macro_deps,
            proc_macro_deps_dev,
            version: package.version.to_string(),
            ..Default::default()
        };

        // Iterate over each target and produce a Bazel target for all supported "kinds"
        let targets = Self::collect_targets(
            &annotation.node,
            packages,
            Self::crate_includes_build_script(package, extras, include_build_scripts),
        );

        // Parse the library crate name from the set of included targets
        let library_target_name = {
            let lib_targets: Vec<&TargetAttributes> = targets
                .iter()
                .filter_map(|t| match t {
                    Rule::ProcMacro(attrs) => Some(attrs),
                    Rule::Library(attrs) => Some(attrs),
                    _ => None,
                })
                .collect();

            // TODO: There should only be at most 1 library target. This case
            // should be handled in a more intelligent way.
            assert!(lib_targets.len() <= 1);
            lib_targets
                .iter()
                .last()
                .map(|attr| attr.crate_name.clone())
        };

        // Gather any build-script related attributes
        let build_script_target = targets.iter().find_map(|r| match r {
            Rule::BuildScript(attr) => Some(attr),
            _ => None,
        });

        let build_script_attrs = if let Some(target) = build_script_target {
            // Track the build script dependency
            common_attrs.deps.insert(
                CrateDependency {
                    id: current_crate_id,
                    target: target.crate_name.clone(),
                    alias: None,
                },
                None,
            );

            let build_deps = annotation.deps.build_deps.clone().map(new_crate_dep);
            let build_proc_macro_deps = annotation
                .deps
                .build_proc_macro_deps
                .clone()
                .map(new_crate_dep);

            Some(BuildScriptAttributes {
                deps: build_deps,
                proc_macro_deps: build_proc_macro_deps,
                links: package.links.clone(),
                ..Default::default()
            })
        } else {
            None
        };

        // Save the repository information for the current crate
        let repository = source_annotations.get(&package.id).cloned();

        // Identify the license type
        let license = package.license.clone();

        // Create the crate's context and apply extra settings
        CrateContext {
            name: package.name.clone(),
            version: package.version.to_string(),
            repository,
            targets,
            library_target_name,
            common_attrs,
            build_script_attrs,
            license,
            extra_build_contents: None,
        }
        .with_overrides(extras)
    }

    fn with_overrides(mut self, extras: &BTreeMap<CrateId, PairredExtras>) -> Self {
        let id = CrateId::new(self.name.clone(), self.version.clone());

        // Insert all overrides/extras
        if let Some(pairred_override) = extras.get(&id) {
            let crate_extra = &pairred_override.crate_extra;

            // Deps
            if let Some(extra) = &crate_extra.deps {
                self.common_attrs.extra_deps = extra.clone();
            }

            // Proc macro deps
            if let Some(extra) = &crate_extra.proc_macro_deps {
                self.common_attrs.extra_proc_macro_deps = extra.clone();
            }

            // Compile data
            if let Some(extra) = &crate_extra.compile_data {
                for data in extra.iter() {
                    self.common_attrs.compile_data.insert(data.clone(), None);
                }
            }

            // Compile data glob
            if let Some(extra) = &crate_extra.compile_data_glob {
                self.common_attrs.compile_data_glob.extend(extra.clone());
            }

            // Crate features
            if let Some(extra) = &crate_extra.crate_features {
                for data in extra.iter() {
                    self.common_attrs.crate_features.insert(data.clone());
                }
            }

            // Data
            if let Some(extra) = &crate_extra.data {
                for data in extra.iter() {
                    self.common_attrs.data.insert(data.clone(), None);
                }
            }

            // Data glob
            if let Some(extra) = &crate_extra.data_glob {
                self.common_attrs.data_glob.extend(extra.clone());
            }

            // Rustc flags
            // TODO: SelectList is currently backed by `BTreeSet` which is generally incorrect
            // for rustc flags. Should SelectList be refactored?
            if let Some(extra) = &crate_extra.rustc_flags {
                for data in extra.iter() {
                    self.common_attrs.rustc_flags.insert(data.clone(), None);
                }
            }

            // Rustc env
            if let Some(extra) = &crate_extra.rustc_env {
                self.common_attrs.rustc_env.insert(extra.clone(), None);
            }

            // Rustc env files
            if let Some(extra) = &crate_extra.rustc_env_files {
                for data in extra.iter() {
                    self.common_attrs.rustc_env_files.insert(data.clone(), None);
                }
            }

            // Build script Attributes
            if let Some(attrs) = &mut self.build_script_attrs {
                // Deps
                if let Some(extra) = &crate_extra.build_script_deps {
                    attrs.extra_deps = extra.clone();
                }

                // Proc macro deps
                if let Some(extra) = &crate_extra.build_script_proc_macro_deps {
                    attrs.extra_proc_macro_deps = extra.clone();
                }

                // Data
                if let Some(extra) = &crate_extra.build_script_data {
                    for data in extra {
                        attrs.data.insert(data.clone(), None);
                    }
                }

                // Data glob
                if let Some(extra) = &crate_extra.build_script_data_glob {
                    attrs.data_glob.extend(extra.clone());
                }

                // Rustc env
                if let Some(extra) = &crate_extra.build_script_rustc_env {
                    attrs.rustc_env.insert(extra.clone(), None);
                }

                // Build script env
                if let Some(extra) = &crate_extra.build_script_env {
                    attrs.build_script_env.insert(extra.clone(), None);
                }
            }

            // Extra build contents
            self.extra_build_contents = crate_extra.build_content.as_ref().map(|content| {
                // For prettier rendering, dedent the build contents
                textwrap::dedent(content)
            });

            // Git shallow_since
            if let Some(SourceAnnotation::Git { shallow_since, .. }) = &mut self.repository {
                *shallow_since = crate_extra.shallow_since.clone()
            }
        }

        self
    }

    /// Determine whether or not a crate __should__ include a build script
    /// (build.rs) if it happens to have one.
    fn crate_includes_build_script(
        package: &Package,
        overrides: &BTreeMap<CrateId, PairredExtras>,
        default_generate_build_script: bool,
    ) -> bool {
        match overrides
            .iter()
            .find(|(_, settings)| settings.package_id == package.id)
            .map(|(_, settings)| settings)
        {
            // The settings will awlays take precedence if provided
            Some(settings) => match settings.crate_extra.gen_build_script {
                Some(gen_build_script) => gen_build_script,
                None => default_generate_build_script,
            },
            None => default_generate_build_script,
        }
    }

    /// Collect all Bazel targets that should be generated for a particular Package
    fn collect_targets(
        node: &Node,
        packages: &BTreeMap<PackageId, Package>,
        include_build_scripts: bool,
    ) -> Vec<Rule> {
        let package = &packages[&node.id];

        let package_root = package
            .manifest_path
            .as_std_path()
            .parent()
            .expect("Every manifest should have a parent directory");

        package
            .targets
            .iter()
            .flat_map(|target| {
                target
                    .kind
                    .iter()
                    .filter_map(|kind| {
                        // Unfortunately, The package graph and resolve graph of cargo metadata have different representations
                        // for the crate names (resolve graph sanitizes names to match module names) so to get the rest of this
                        // content to align when rendering, the package target names are always sanitized.
                        let crate_name = sanitize_module_name(&target.name);

                        // Locate the crate's root source file relative to the package root normalized for unix
                        let crate_root =
                            pathdiff::diff_paths(target.src_path.to_string(), package_root).map(
                                // Normalize the path so that it always renders the same regardless of platform
                                |root| root.to_string_lossy().replace("\\", "/"),
                            );

                        // Conditionally check to see if the dependencies is a build-script target
                        if include_build_scripts && kind == "custom-build" {
                            return Some(Rule::BuildScript(TargetAttributes {
                                crate_name,
                                crate_root,
                                srcs: Glob::new_rust_srcs(),
                            }));
                        }

                        // Check to see if the dependencies is a proc-macro target
                        if kind == "proc-macro" {
                            return Some(Rule::ProcMacro(TargetAttributes {
                                crate_name,
                                crate_root,
                                srcs: Glob::new_rust_srcs(),
                            }));
                        }

                        // Check to see if the dependencies is a library target
                        if kind == "lib" {
                            return Some(Rule::Library(TargetAttributes {
                                crate_name,
                                crate_root,
                                srcs: Glob::new_rust_srcs(),
                            }));
                        }

                        // Check to see if the dependencies is a library target
                        if kind == "bin" {
                            return Some(Rule::Binary(TargetAttributes {
                                crate_name: target.name.clone(),
                                crate_root,
                                srcs: Glob::new_rust_srcs(),
                            }));
                        }

                        None
                    })
                    .collect::<Vec<Rule>>()
            })
            .collect()
    }
}