//! Command line interface entry points and utilities

mod generate;
mod query;
mod splice;

use structopt::StructOpt;

use self::generate::GenerateOptions;
use self::query::QueryOptions;
use self::splice::SpliceOptions;

// Entrypoints
pub use generate::generate;
pub use query::query;
pub use splice::splice;

#[derive(StructOpt, Debug)]
#[structopt(name = "cargo-bazel")]
pub enum Options {
    /// Generate Bazel Build files from a Cargo manifest.
    Generate(GenerateOptions),

    /// Splice together disjoint Cargo and Bazel info into a single Cargo workspace manifest.
    Splice(SpliceOptions),

    /// Query workspace info to determine whether or not a repin is needed.
    Query(QueryOptions),
}

// Convenience wrappers to avoid dependencies in the binary
pub type Result<T> = anyhow::Result<T>;

pub fn parse_args() -> Options {
    Options::from_args()
}
