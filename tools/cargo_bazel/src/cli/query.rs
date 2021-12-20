//!

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::config::Config;
use crate::context::Context;
use crate::lockfile::Digest;
use crate::splicing::SplicingManifest;

/// Command line options for the `query` subcommand
#[derive(StructOpt, Debug)]
pub struct QueryOptions {
    /// The lockfile path for reproducible Cargo->Bazel renderings
    #[structopt(long)]
    pub lockfile: PathBuf,

    /// The config file with information about the Bazel and Cargo workspace
    #[structopt(long)]
    pub config: PathBuf,

    /// A generated manifest of splicing inputs
    #[structopt(long)]
    pub splicing_manifest: PathBuf,

    /// The path to a Cargo binary to use for gathering metadata
    #[structopt(long, env = "CARGO")]
    pub cargo: PathBuf,

    /// The path to a rustc binary for use with Cargo
    #[structopt(long, env = "RUSTC")]
    pub rustc: PathBuf,
}

/// Determine if the current lockfile needs to be re-pinned
pub fn query(opt: QueryOptions) -> Result<()> {
    // Read the lockfile
    let content = match fs::read_to_string(&opt.lockfile) {
        Ok(c) => c,
        Err(_) => return announce_repin("Unable to read lockfile"),
    };

    // Deserialize it so we can easily compare it with
    let lockfile: Context = match serde_json::from_str(&content) {
        Ok(ctx) => ctx,
        Err(_) => return announce_repin("Could not load lockfile"),
    };

    // Check to see if a digest has been set
    let digest = match &lockfile.checksum {
        Some(d) => d.clone(),
        None => return announce_repin("No digest provided in lockfile"),
    };

    // Load the config file
    let config = Config::try_from_path(&opt.config)?;

    let splicing_manifest = SplicingManifest::try_from_path(&opt.splicing_manifest)?;

    // Generate a new digest so we can compare it with the one in the lockfile
    let expected = Digest::new(
        &lockfile,
        &config,
        &splicing_manifest,
        &opt.cargo,
        &opt.rustc,
    )?;
    if digest != expected {
        return announce_repin(&format!(
            "Digests do not match: {:?} != {:?}",
            digest, expected
        ));
    }

    // There is no need to repin
    Ok(())
}

fn announce_repin(reason: &str) -> Result<()> {
    eprintln!("{}", reason);
    println!("repin");
    Ok(())
}
