//! Tools for parsing [Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html) files

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use serde::Deserialize;

/// The [`[registry]`](https://doc.rust-lang.org/cargo/reference/config.html#registry)
/// table controls the default registry used when one is not specified.
#[derive(Debug, Deserialize)]
pub struct Registry {
    /// name of the default registry
    pub default: String,

    /// authentication token for crates.io
    pub token: Option<String>,
}

/// The [`[source]`](https://doc.rust-lang.org/cargo/reference/config.html#source)
/// table defines the registry sources available.
#[derive(Debug, Deserialize)]
pub struct Source {
    /// replace this source with the given named source
    #[serde(rename = "replace-with")]
    pub replace_with: Option<String>,

    /// URL to a registry source
    #[serde(default = "default_registry_url")]
    pub registry: String,
}

/// This is the default registry url per what's defined by Cargo.
fn default_registry_url() -> String {
    "https://github.com/rust-lang/crates.io-index".to_owned()
}

#[derive(Debug, Deserialize)]
/// registries other than crates.io
pub struct AdditionalRegistry {
    /// URL of the registry index
    pub index: String,

    /// authentication token for the registry
    pub token: Option<String>,
}

/// A subset of a Cargo configuration file. The schema here is only what
/// is required for parsing registry information.
/// See [cargo docs](https://doc.rust-lang.org/cargo/reference/config.html#configuration-format)
/// for more details.
#[derive(Debug, Deserialize)]
pub struct CargoConfig {
    /// registries other than crates.io
    #[serde(default = "default_registries")]
    pub registries: HashMap<String, AdditionalRegistry>,

    #[serde(default = "default_registry")]
    pub registry: Registry,

    /// source definition and replacement
    #[serde(default = "HashMap::new")]
    pub source: HashMap<String, Source>,
}

/// Each Cargo config is expected to have a default `crates-io` registry.
fn default_registries() -> HashMap<String, AdditionalRegistry> {
    let mut registries = HashMap::new();
    registries.insert(
        "crates-io".to_owned(),
        AdditionalRegistry {
            index: default_registry_url(),
            token: None,
        },
    );
    registries
}

/// Each Cargo config has a default registry for `crates.io`.
fn default_registry() -> Registry {
    Registry {
        default: "crates-io".to_owned(),
        token: None,
    }
}

impl Default for CargoConfig {
    fn default() -> Self {
        let registries = default_registries();
        let registry = default_registry();
        let source = Default::default();

        Self {
            registries,
            registry,
            source,
        }
    }
}

impl FromStr for CargoConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let incoming: CargoConfig = toml::from_str(s)?;
        let mut config = Self::default();
        config.registries.extend(incoming.registries);
        config.source.extend(incoming.source);
        config.registry = incoming.registry;
        Ok(config)
    }
}

impl CargoConfig {
    /// Load a Cargo conig from a path to a file on disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Look up a reigstry [Source] by it's url.
    pub fn get_source_from_url(&self, url: &str) -> Option<&Source> {
        self.source.values().find(|v| v.registry == url)
    }
}
