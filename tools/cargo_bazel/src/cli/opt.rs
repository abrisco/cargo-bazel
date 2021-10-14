//! A collection of command line options

use structopt::StructOpt;

use super::generate::GenerateOptions;
use super::query::QueryOptions;
use super::splice::SpliceOptions;

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
