//! A collection of command line options

use std::path::PathBuf;

use structopt::StructOpt;

use crate::lockfile::LockfileKind;

/// Command line options for the `generate` subcommand
#[derive(StructOpt, Debug)]
pub struct GenerateOptions {
    /// The path to a Cargo binary to use for gathering metadata
    #[structopt(long, env = "CARGO")]
    pub cargo: Option<PathBuf>,

    /// The path to a rustc binary for use with Cargo
    #[structopt(long, env = "RUSTC")]
    pub rustc: Option<PathBuf>,

    /// The config file with information about the Bazel and Cargo workspace
    #[structopt(long)]
    pub config: PathBuf,

    /// The path to either a Cargo or Bazel lockfile
    #[structopt(long)]
    pub lockfile: PathBuf,

    /// The type of lockfile
    #[structopt(long)]
    pub lockfile_kind: LockfileKind,

    /// The directory of the current repository rule
    #[structopt(long)]
    pub repository_dir: PathBuf,

    /// A [Cargo config](https://doc.rust-lang.org/cargo/reference/config.html#configuration)
    /// file to use when gathering metadata
    #[structopt(long)]
    pub cargo_config: Option<PathBuf>,

    /// Whether or not to ignore the provided lockfile and re-generate one
    #[structopt(long)]
    pub repin: bool,

    /// The root manifest used to generate metadata
    #[structopt(long)]
    pub manifest: Option<PathBuf>,

    /// If true, outputs will be printed instead of written to disk.
    #[structopt(long)]
    pub dry_run: bool,
}

/// Command line options for the `splice` subcommand
#[derive(StructOpt, Debug)]
pub struct SpliceOptions {
    /// A generated manifest of splicing inputs
    #[structopt(long)]
    pub splicing_manifest: PathBuf,

    /// A Cargo lockfile (Cargo.lock).
    #[structopt(long)]
    pub cargo_lockfile: Option<PathBuf>,

    /// The directory in which to build the workspace. A `Cargo.toml` file
    /// should always be produced within this directory.
    #[structopt(long)]
    pub workspace_dir: PathBuf,

    /// If true, outputs will be printed instead of written to disk.
    #[structopt(long)]
    pub dry_run: bool,

    /// The path to a Cargo binary to use for gathering metadata
    #[structopt(long, env = "CARGO")]
    pub cargo: PathBuf,

    /// The path to a rustc binary for use with Cargo
    #[structopt(long, env = "RUSTC")]
    pub rustc: PathBuf,
}

/// Command line options for the `query` subcommand
#[derive(StructOpt, Debug)]
pub struct QueryOptions {
    /// The lockfile path for reproducible Cargo->Bazel renderings
    #[structopt(long)]
    pub lockfile: PathBuf,

    /// The config file with information about the Bazel and Cargo workspace
    #[structopt(long)]
    pub config: PathBuf,

    /// The path to a Cargo binary to use for gathering metadata
    #[structopt(long, env = "CARGO")]
    pub cargo: PathBuf,

    /// The path to a rustc binary for use with Cargo
    #[structopt(long, env = "RUSTC")]
    pub rustc: PathBuf,
}

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
