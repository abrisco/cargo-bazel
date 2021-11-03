//! A module for configuration information

use std::collections::{BTreeMap, BTreeSet};
use std::convert::AsRef;
use std::path::Path;
use std::{fmt, fs};

use anyhow::Result;
use cargo_lock::package::source::GitReference;
use cargo_metadata::Package;
use semver::VersionReq;
use serde::de::Visitor;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Hash, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RenderConfig {
    /// The name of the repository being rendered
    pub repository_name: String,

    /// The pattern to use for BUILD file names.
    /// Eg. `BUILD.{name}-{version}.bazel`
    #[serde(default = "default_build_file_template")]
    pub build_file_template: String,

    /// The pattern to use for a crate target.
    /// Eg. `@{repository}__{name}-{version}//:{target}`
    #[serde(default = "default_crate_label_template")]
    pub crate_label_template: String,

    /// The pattern used for a crate's repository name.
    /// Eg. `{repository}__{name}-{version}`
    #[serde(default = "default_crate_repository_template")]
    pub crate_repository_template: String,

    /// The default of the `package_name` parameter to use for the module macros like `all_crate_deps`.
    /// In general, this should be be unset to allow the macros to do auto-detection in the analysis phase.
    pub default_package_name: Option<String>,

    /// The pattern to use for platform constraints.
    /// Eg. `@rules_rust//rust/platform:{triple}`.
    #[serde(default = "default_platforms_template")]
    pub platforms_template: String,
}

fn default_build_file_template() -> String {
    "BUILD.{name}-{version}.bazel".to_owned()
}

fn default_crate_label_template() -> String {
    "@{repository}__{name}-{version}//:{target}".to_owned()
}

fn default_crate_repository_template() -> String {
    "{repository}__{name}-{version}".to_owned()
}

fn default_platforms_template() -> String {
    "@rules_rust//rust/platform:{triple}".to_owned()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Commitish {
    /// From a tag.
    Tag(String),

    /// From the HEAD of a branch.
    Branch(String),

    /// From a specific revision.
    Rev(String),
}

impl From<GitReference> for Commitish {
    fn from(git_ref: GitReference) -> Self {
        match git_ref {
            GitReference::Tag(v) => Self::Tag(v),
            GitReference::Branch(v) => Self::Branch(v),
            GitReference::Rev(v) => Self::Rev(v),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Checksumish {
    Http {
        sha256: Option<String>,
    },
    Git {
        commitsh: Commitish,
        shallow_since: Option<String>,
    },
}

#[derive(Debug, Hash, Deserialize, Serialize, Clone)]
pub struct CrateExtras {
    /// Determins whether or not Cargo build scripts should be generated for the current package
    pub gen_build_script: Option<bool>,

    /// Additional data to pass to
    /// [deps](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-deps) attribute.
    pub deps: Option<BTreeSet<String>>,

    /// Additional data to pass to
    /// [proc_macro_deps](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-proc_macro_deps) attribute.
    pub proc_macro_deps: Option<BTreeSet<String>>,

    /// Additional data to pass to  the target's
    /// [crate_features](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-crate_features) attribute.
    pub crate_features: Option<BTreeSet<String>>,

    /// Additional data to pass to  the target's
    /// [data](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-data) attribute.
    pub data: Option<BTreeSet<String>>,

    /// An optional glob pattern to set on the
    /// [data](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-data) attribute.
    pub data_glob: Option<BTreeSet<String>>,

    /// Additional data to pass to
    /// [compile_data](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-compile_data) attribute.
    pub compile_data: Option<BTreeSet<String>>,

    /// An optional glob pattern to set on the
    /// [compile_data](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-compile_data) attribute.
    pub compile_data_glob: Option<BTreeSet<String>>,

    /// Additional data to pass to  the target's
    /// [rustc_env](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-rustc_env) attribute.
    pub rustc_env: Option<BTreeMap<String, String>>,

    /// Additional data to pass to  the target's
    /// [rustc_env_files](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-rustc_env_files) attribute.
    pub rustc_env_files: Option<BTreeSet<String>>,

    /// Additional data to pass to the target's
    /// [rustc_flags](https://bazelbuild.github.io/rules_rust/defs.html#rust_library-rustc_flags) attribute.
    pub rustc_flags: Option<Vec<String>>,

    /// Additional dependencies to pass to a build script's
    /// [deps](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-deps) attribute.
    pub build_script_deps: Option<BTreeSet<String>>,

    /// Additional data to pass to a build script's
    /// [proc_macro_deps](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-proc_macro_deps) attribute.
    pub build_script_proc_macro_deps: Option<BTreeSet<String>>,

    /// Additional data to pass to a build script's
    /// [build_script_data](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-data) attribute.
    pub build_script_data: Option<BTreeSet<String>>,

    /// Additional data to pass to a build script's
    /// [tools](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-tools) attribute.
    pub build_script_tools: Option<BTreeSet<String>>,

    /// An optional glob pattern to set on the
    /// [build_script_data](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-build_script_env) attribute.
    pub build_script_data_glob: Option<BTreeSet<String>>,

    /// Additional environment variables to pass to a build script's
    /// [build_script_env](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-rustc_env) attribute.
    pub build_script_env: Option<BTreeMap<String, String>>,

    /// Additional rustc_env flags to pass to a build script's
    /// [rustc_env](https://bazelbuild.github.io/rules_rust/cargo.html#cargo_build_script-rustc_env) attribute.
    pub build_script_rustc_env: Option<BTreeMap<String, String>>,

    /// A scratch pad used to write arbitrary text to target BUILD files.
    pub build_content: Option<String>,

    /// For git sourced crates, this is a the
    /// [git_repository::shallow_since](https://docs.bazel.build/versions/main/repo/git.html#new_git_repository-shallow_since) attribute.
    pub shallow_since: Option<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct CrateId {
    pub name: String,
    pub version: String,
}

impl CrateId {
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
    }

    pub fn matches(&self, package: &Package) -> bool {
        // If the package name does not match, it's obviously
        // not the right package
        if self.name != package.name {
            return false;
        }

        // First see if the package version matches exactly
        if package.version.to_string() == self.version {
            return true;
        }

        // Next, check to see if the version provided is a semver req and
        // check if the package matches the condition
        if let Ok(semver) = VersionReq::parse(&self.version) {
            if semver.matches(&package.version) {
                return true;
            }
        }

        false
    }
}

impl From<&Package> for CrateId {
    fn from(package: &Package) -> Self {
        Self {
            name: package.name.clone(),
            version: package.version.to_string(),
        }
    }
}

impl Serialize for CrateId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{} {}", self.name, self.version))
    }
}

struct CrateIdVisitor;
impl<'de> Visitor<'de> for CrateIdVisitor {
    type Value = CrateId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expected string value of `{name} {version}`.")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.rsplit_once(' ')
            .map(|(name, version)| CrateId {
                name: name.to_string(),
                version: version.to_string(),
            })
            .ok_or_else(|| {
                E::custom(format!(
                    "Expected string value of `{{name}} {{version}}`. Got '{}'",
                    v
                ))
            })
    }
}

impl<'de> Deserialize<'de> for CrateId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(CrateIdVisitor)
    }
}

impl std::fmt::Display for CrateId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&format!("{} {}", self.name, self.version), f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Whether or not to generate Cargo build scripts by default
    pub generate_build_scripts: bool,

    /// Additional settings to apply to generated crates
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extras: BTreeMap<CrateId, CrateExtras>,

    /// Settings used to determine various render info
    pub rendering: RenderConfig,

    /// The contents of a Cargo configuration file
    pub cargo_config: Option<toml::Value>,

    /// A set of platform triples to use in generated select statements
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub supported_platform_triples: BTreeSet<String>,
}

impl Config {
    pub fn try_from_path<T: AsRef<Path>>(path: T) -> Result<Self> {
        let data = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&data)?)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::test::*;

    #[test]
    fn test_crate_id_serde() {
        let id: CrateId = serde_json::from_str("\"crate 0.1.0\"").unwrap();
        assert_eq!(id, CrateId::new("crate".to_owned(), "0.1.0".to_owned()));
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"crate 0.1.0\"");
    }

    #[test]
    fn test_crate_id_serde_semver() {
        let semver_id: CrateId = serde_json::from_str("\"crate *\"").unwrap();
        assert_eq!(semver_id, CrateId::new("crate".to_owned(), "*".to_owned()));
        assert_eq!(serde_json::to_string(&semver_id).unwrap(), "\"crate *\"");
    }

    #[test]
    fn test_crate_id_matches() {
        let mut package = mock_cargo_metadata_package();
        let id = CrateId::new("mock-pkg".to_owned(), "0.1.0".to_owned());

        package.version = cargo_metadata::Version::new(0, 1, 0);
        assert!(id.matches(&package));

        package.version = cargo_metadata::Version::new(1, 0, 0);
        assert!(!id.matches(&package));
    }

    #[test]
    fn test_crate_id_semver_matches() {
        let mut package = mock_cargo_metadata_package();
        package.version = cargo_metadata::Version::new(1, 0, 0);
        let mut id = CrateId::new("mock-pkg".to_owned(), "0.1.0".to_owned());

        id.version = "*".to_owned();
        assert!(id.matches(&package));

        id.version = "<1".to_owned();
        assert!(!id.matches(&package));
    }
}
