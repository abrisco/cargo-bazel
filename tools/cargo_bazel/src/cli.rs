//! Command line interface entry points and utilities

mod generate;
mod opt;
mod query;
mod splice;

// Command line arguments
pub use opt::Options;

// Entrypoints
pub use generate::generate;
pub use query::query;
pub use splice::splice;

use structopt::StructOpt;

// Convenience wrappers to avoid dependencies in the binary
pub type Result<T> = anyhow::Result<T>;

pub fn parse_args() -> opt::Options {
    opt::Options::from_args()
}
