//! A small test binary for ensuring the version of the rules matches the binary version

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

fn main() {
    // Parse the version field from the `cargo-bazel` Cargo.toml file
    let cargo_version = {
        let cargo_path = PathBuf::from(env!("CARGO_TOML"));
        let file = File::open(cargo_path).expect("Failed to load Cargo.toml file");
        BufReader::new(file)
            .lines()
            .flatten()
            .find(|line| line.contains("version = "))
            .map(|line| {
                line.trim()
                    .replace("version = ", "")
                    .trim_matches('\"')
                    .to_owned()
            })
            .expect("The version.bzl file should have a line with `version = `")
    };

    // Parse the version global from the Bazel module
    let bazel_version = {
        let bazel_path = PathBuf::from(env!("VERSION_BZL"));
        let file = File::open(bazel_path).expect("Failed to load versions.bzl file");
        BufReader::new(file)
            .lines()
            .flatten()
            .find(|line| line.contains("VERSION = "))
            .map(|line| {
                line.trim()
                    .replace("VERSION = ", "")
                    .trim_matches('\"')
                    .to_owned()
            })
            .expect("The version.bzl file should have a line with `VERSION = `")
    };

    eprintln!("If this test fails, make sure `//:version.bzl` and `//:Cargo.toml` have matching versions");
    assert_eq!(cargo_version, bazel_version)
}
