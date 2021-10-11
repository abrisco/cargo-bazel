//! TODO

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Digest(String);

impl Digest {
    pub fn new(config: &Config, cargo_bin: &Path, rustc_bin: &Path) -> Result<Self> {
        let mut hasher = Sha256::new();

        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        hasher.update(b"\0");

        hasher.update(serde_json::to_string(config)?.as_bytes());
        hasher.update(b"\0");

        hasher.update(Self::bin_version(cargo_bin)?.as_bytes());
        hasher.update(b"\0");

        hasher.update(Self::bin_version(rustc_bin)?.as_bytes());
        hasher.update(b"\0");

        Ok(Self(hasher.finalize().encode_hex::<String>()))
    }

    fn bin_version(binary: &Path) -> Result<String> {
        let output = Command::new(binary).arg("--version").env_clear().output()?;

        if !output.status.success() {
            bail!("Failed to query cargo version")
        }

        let version = String::from_utf8(output.stdout)?;
        Ok(version)
    }
}

impl PartialEq<str> for Digest {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<String> for Digest {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}
