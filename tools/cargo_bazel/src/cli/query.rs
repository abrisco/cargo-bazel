//!

use std::fs;

use anyhow::Result;
use serde::Deserialize;

use crate::cli::opt::QueryOptions;
use crate::config::Config;
use crate::digest::Digest;

#[derive(Debug, Deserialize)]
struct Lockfile {
    pub checksum: Option<Digest>,
}

/// Determine if the current lockfile needs to be re-pinned
pub fn query(opt: QueryOptions) -> Result<()> {
    // Read the lockfile
    let content = match fs::read_to_string(&opt.lockfile) {
        Ok(c) => c,
        Err(_) => return announce_repin("Unable to read lockfile"),
    };

    // Deserialize it so we can easily compare it with
    let lockfile: Lockfile = match serde_json::from_str(&content) {
        Ok(ctx) => ctx,
        Err(_) => return announce_repin("Could not load lockfile"),
    };

    // Check to see if a digest has been set
    let digest = match lockfile.checksum {
        Some(d) => d,
        None => return announce_repin("No digest provided in lockfile"),
    };

    // Load the config file
    let config = Config::try_from_path(&opt.config)?;

    // Generate a new digest so we can compare it with the one in the lockfile
    let expected = Digest::new(&config, &opt.cargo, &opt.rustc)?;
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
