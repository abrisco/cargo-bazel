//! Tools for gathering various kinds of metadata (Cargo.lock, Cargo metadata, Crate Index info).

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use cargo_lock::Lockfile as CargoLockfile;
use cargo_metadata::{Metadata as CargoMetadata, MetadataCommand};

// TODO: This should also return a set of [crate-index::IndexConfig]s for packages in metadata.packages
pub trait MetadataGenerator {
    fn generate<T: AsRef<Path>>(&self, manifest_path: T) -> Result<(CargoMetadata, CargoLockfile)>;
}

pub struct Generator {
    cargo_bin: PathBuf,
    rustc_bin: PathBuf,
    existing_lockfile: Option<CargoLockfile>,
}

impl Generator {
    pub fn new() -> Self {
        Generator {
            cargo_bin: PathBuf::from(env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())),
            rustc_bin: PathBuf::from(env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string())),
            existing_lockfile: None,
        }
    }

    pub fn with_cargo(mut self, cargo_bin: PathBuf) -> Self {
        self.cargo_bin = cargo_bin;
        self
    }

    pub fn with_rustc(mut self, rustc_bin: PathBuf) -> Self {
        self.rustc_bin = rustc_bin;
        self
    }

    pub fn with_cargo_lockfile<T: AsRef<Path>>(mut self, lockfile: &T) -> Result<Self> {
        self.existing_lockfile = Some(CargoLockfile::load(lockfile.as_ref())?);
        Ok(self)
    }
}

impl MetadataGenerator for Generator {
    fn generate<T: AsRef<Path>>(&self, manifest_path: T) -> Result<(CargoMetadata, CargoLockfile)> {
        let lockfile = match &self.existing_lockfile {
            Some(lock) => lock.clone(),
            None => {
                let manifest_dir = manifest_path
                    .as_ref()
                    .parent()
                    .expect("The manifest should have a parent directory");
                let lock_path = manifest_dir.join("Cargo.lock");
                if !lock_path.exists() {
                    bail!("No `Cargo.lock` file was found with the given manifest")
                }
                cargo_lock::Lockfile::load(lock_path)?
            }
        };

        let metadata = MetadataCommand::new()
            .cargo_path(&self.cargo_bin)
            .manifest_path(manifest_path.as_ref())
            .other_options(["--offline".to_owned(), "--locked".to_owned()])
            .exec()?;

        Ok((metadata, lockfile))
    }
}

pub struct LockGenerator {
    cargo_bin: PathBuf,
    rustc_bin: PathBuf,
}

impl LockGenerator {
    pub fn new(cargo_bin: PathBuf, rustc_bin: PathBuf) -> Self {
        Self {
            cargo_bin,
            rustc_bin,
        }
    }

    pub fn generate(&self, manifest_path: &Path) -> Result<cargo_lock::Lockfile> {
        let output = Command::new(&self.cargo_bin)
            .arg("generate-lockfile")
            .arg("--manifest-path")
            .arg(manifest_path)
            .env("RUSTC", &self.rustc_bin)
            .output()
            .context(format!(
                "Error running cargo to generate lockfile '{}'",
                manifest_path.display()
            ))?;

        if !output.status.success() {
            bail!(format!("Failed to generate lockfile: {:?}", output))
        }

        let manifest_dir = manifest_path.parent().unwrap();
        let generated_lockfile_path = manifest_dir.join("Cargo.lock");

        cargo_lock::Lockfile::load(&generated_lockfile_path).context(format!(
            "Failed to load lockfile: {}",
            generated_lockfile_path.display()
        ))
    }
}
