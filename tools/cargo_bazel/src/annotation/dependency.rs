///! Gathering dependencies is the largest part of annotating.
use cargo_metadata::{Metadata as CargoMetadata, Node, NodeDep, Package, PackageId};
use serde::{Deserialize, Serialize};

use crate::utils::sanitize_module_name;
use crate::utils::starlark::{Select, SelectList};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Dependency {
    /// The PackageId of the target
    pub package_id: PackageId,

    /// The name of the dependncy as seen in [cargo_metadata::NodeDep].
    pub target_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencySet {
    pub normal_deps: SelectList<Dependency>,
    pub normal_dev_deps: SelectList<Dependency>,
    pub proc_macro_deps: SelectList<Dependency>,
    pub proc_macro_dev_deps: SelectList<Dependency>,
    pub build_deps: SelectList<Dependency>,
    pub build_proc_macro_deps: SelectList<Dependency>,
}

impl DependencySet {
    /// Collect all dependencies for a given node in the resolve graph.
    pub fn new_for_node(node: &Node, metadata: &CargoMetadata) -> Self {
        let (normal_dev_deps, normal_deps) = {
            let (dev, normal) = node
                .deps
                .iter()
                // Do not track workspace members as dependencies. Users are expected to maintain those connections
                .filter(|dep| !is_workspace_member(dep, metadata))
                .filter(|dep| is_lib_package(&metadata[&dep.pkg]))
                .filter(|dep| !is_build_dependency(dep))
                .partition(|dep| is_dev_dependency(dep));

            (
                collect_deps_selectable(dev),
                collect_deps_selectable(normal),
            )
        };

        let (proc_macro_dev_deps, proc_macro_deps) = {
            let (dev, normal) = node
                .deps
                .iter()
                // Do not track workspace members as dependencies. Users are expected to maintain those connections
                .filter(|dep| !is_workspace_member(dep, metadata))
                .filter(|dep| is_proc_macro_package(&metadata[&dep.pkg]))
                .filter(|dep| !is_build_dependency(dep))
                .partition(|dep| is_dev_dependency(dep));

            (
                collect_deps_selectable(dev),
                collect_deps_selectable(normal),
            )
        };

        let (build_proc_macro_deps, mut build_deps) = {
            let (proc_macro, normal) = node
                .deps
                .iter()
                // Do not track workspace members as dependencies. Users are expected to maintain those connections
                .filter(|dep| !is_workspace_member(dep, metadata))
                .filter(|dep| is_build_dependency(dep))
                .filter(|dep| !is_dev_dependency(dep))
                .partition(|dep| is_proc_macro_package(&metadata[&dep.pkg]));

            (
                collect_deps_selectable(proc_macro),
                collect_deps_selectable(normal),
            )
        };

        // `*-sys` packages follow slightly different rules than other dependencies. These
        // packages seem to provide some environment variables required to build the top level
        // package and are expected to be avialable to other build scripts. If a target depends
        // on a `*-sys` crate for itself, so would it's build script. Hopefully this is correct.
        // https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key
        // https://doc.rust-lang.org/cargo/reference/build-scripts.html#-sys-packages
        let sys_name = format!("{}-sys", &metadata[&node.id].name);
        normal_deps.configurations().into_iter().for_each(|config| {
            normal_deps
                .get_iter(config)
                // Iterating over known key should be safe
                .unwrap()
                // Add any normal dependency to build dependencies that are associated `*-sys` crates
                .for_each(|dep| {
                    let dep_pkg_name = &metadata[&dep.package_id].name;
                    if *dep_pkg_name == sys_name {
                        let mut dep = dep.clone();
                        dep.target_name = sanitize_module_name(dep_pkg_name);
                        build_deps.insert(dep, config.cloned())
                    }
                });
        });

        Self {
            normal_deps,
            normal_dev_deps,
            proc_macro_deps,
            proc_macro_dev_deps,
            build_deps,
            build_proc_macro_deps,
        }
    }
}

fn collect_deps_selectable(deps: Vec<&NodeDep>) -> SelectList<Dependency> {
    let mut selectable = SelectList::default();

    for dep in deps.into_iter() {
        let kind_info = dep
            .dep_kinds
            .first()
            .expect("Each dependency should have at least 1 kind");

        selectable.insert(
            Dependency {
                package_id: dep.pkg.clone(),
                target_name: dep.name.clone(),
            },
            kind_info
                .target
                .as_ref()
                .map(|platform| platform.to_string()),
        );
    }

    selectable
}

fn is_lib_package(package: &Package) -> bool {
    package
        .targets
        .iter()
        .any(|target| target.crate_types.iter().any(|t| t == "lib"))
}

fn is_proc_macro_package(package: &Package) -> bool {
    package
        .targets
        .iter()
        .any(|target| target.crate_types.iter().any(|t| t == "proc-macro"))
}

fn is_dev_dependency(node_dep: &NodeDep) -> bool {
    node_dep
        .dep_kinds
        .iter()
        .any(|k| matches!(k.kind, cargo_metadata::DependencyKind::Development))
}

fn is_build_dependency(node_dep: &NodeDep) -> bool {
    node_dep
        .dep_kinds
        .iter()
        .any(|k| matches!(k.kind, cargo_metadata::DependencyKind::Build))
}

fn is_workspace_member(node_dep: &NodeDep, metadata: &CargoMetadata) -> bool {
    metadata
        .workspace_members
        .iter()
        .any(|id| id == &node_dep.pkg)
}
