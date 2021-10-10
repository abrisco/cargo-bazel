//! TODO

use std::fs;
use std::str::FromStr;

use crate::cli::opt::SpliceOptions;
use crate::cli::Result;
use crate::splicing::{Splicer, SplicingManifest};

/// Combine a set of disjoint manifests into a single workspace.
pub fn splice(opt: SpliceOptions) -> Result<()> {
    // Load the "Splicing manifest"
    let splicing_manifest = {
        let content = fs::read_to_string(opt.splicing_manifest)?;
        SplicingManifest::from_str(&content)?
    };

    // Generate a splicer for creating a Cargo workspace manifest
    let splicer = Splicer::new(
        opt.workspace_dir,
        splicing_manifest,
        opt.cargo_lockfile,
        opt.cargo,
        opt.rustc,
    )?;

    // Splice together the manifest
    splicer.splice_workspace()?;

    Ok(())
}
