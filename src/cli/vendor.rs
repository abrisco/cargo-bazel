//! The cli entrypoint for the `vendor` subcommand

use anyhow::Result;
use clap::Parser;

/// Command line options for the `vendor` subcommand
#[derive(Parser, Debug)]
#[clap(about, version)]
pub struct VendorOptions {}

/// Determine if the current lockfile needs to be re-pinned
pub fn vendor(_opt: VendorOptions) -> Result<()> {
    todo!("Implement support for vendoring either BUILD files with remote repository definitions or `cargo vendor` generated sources")
}
