//! The Bazel integration test runner

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::str::FromStr;
use std::{env, fs, io, process};

use clap::Parser;
use flate2::read::GzDecoder;
use tar::Archive;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RepositoryArchive {
    /// The name of the repository.
    pub name: String,

    /// A path to an archive containing the rules.
    pub archive: PathBuf,
}

impl FromStr for RepositoryArchive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split("=").collect();

        if parts.len() != 2 {
            return Err(format!("Unexpected value: {}", s));
        }

        Ok(Self {
            name: String::from(parts[0]),
            archive: PathBuf::from(parts[1]),
        })
    }
}

/// Generate bazelrc files defining --deleted_package flags
#[derive(Parser, Debug)]
#[clap(rename_all = "snake_case")]
struct DeletePackagesOpts {
    /// The directory to begin deleting packages in.
    #[clap(long)]
    pub directory: PathBuf,

    /// The path to the output `.bazelrc` file.
    #[clap(long)]
    pub output: PathBuf,
}

/// Perform Bazel integration tests
#[derive(Parser, Debug)]
#[clap(rename_all = "snake_case")]
struct IntegrationTestOpts {
    /// The path to a WORKSPACE file, indicating the root of a Bazel workspace.
    #[clap(long)]
    pub workspace: PathBuf,

    /// The path to a Bazel binary
    #[clap(long)]
    pub bazel_bin: PathBuf,

    /// A mapping of repository names to archives to use as overrides
    #[clap(long = "repo_archive")]
    pub repo_archives: Vec<RepositoryArchive>,

    /// A list of test environment variables
    #[clap(long = "env")]
    pub envs: Vec<String>,
}

#[derive(Parser, Debug)]
enum Options {
    DeletePackages(DeletePackagesOpts),
    Integration(IntegrationTestOpts),
}

fn parse_args() -> Options {
    let mut options = match env::var("CARGO_BAZEL_INTEGRATION_TEST_ARGS_FILE") {
        Ok(var) => {
            // Prepare a list of arguments
            let mut argv: Vec<String> = Vec::new();

            // Parser argv0 (the exectutable's path)
            let exe_path = PathBuf::from(std::env::args().next().expect("arg 0 was not set"));
            argv.push(exe_path.file_name().unwrap().to_string_lossy().to_string());

            // Read the file found at the variable to an array
            let file = fs::File::open(var).unwrap();
            let reader = BufReader::new(file);
            argv.extend(reader.lines().filter_map(io::Result::ok));

            // Parse arguments
            Options::parse_from(argv)
        }
        Err(_) => Options::parse(),
    };

    match &mut options {
        Options::DeletePackages(_) => {}
        Options::Integration(opts) => {
            opts.envs.dedup();

            // Handle duplicates of repo archives
            let mut deduped = opts.repo_archives.clone();
            deduped.dedup_by(|a, b| a.name == b.name);
            if deduped.len() != opts.repo_archives.len() {
                eprintln!(
                    "A naming conflict was found in `--repo_archive` arguments. Please provide unique repository names: {:#?}",
                    opts.repo_archives
                );
                process::exit(1);
            }
        }
    };

    options
}

/// Generate a `.bazelrc` file which is needed to support integration tests
fn deleted_packages(opts: DeletePackagesOpts) {
    // Walk the given directory, looking for BUILD/BUILD.bazel files

    // Generate deleted packages content

    // Write the content to the requested location on disk
}

/// Perform a Bazel integration test
fn integration(opts: IntegrationTestOpts) {
    // Create an optional temp directory
    let tempdir = tempfile::tempdir().unwrap();
    let (test_workspace, test_home) = match env::var("TEST_TMPDIR") {
        Ok(var) => (
            PathBuf::from(&var).join("bazel_integration_test"),
            PathBuf::from(&var).join("bazel_integration_home"),
        ),
        Err(_) => (
            tempdir.as_ref().join("bazel_integration_test"),
            tempdir.as_ref().join("bazel_integration_home"),
        ),
    };

    // Make all subdirectories
    fs::create_dir_all(&test_workspace).unwrap();
    fs::create_dir_all(&test_home).unwrap();

    // Copy over the workspace files to the test directory
    let workspace_root = opts
        .workspace
        .parent()
        .expect("The WORKSPCE file should always have a parent");
    WalkDir::new(workspace_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| !entry.path().is_dir())
        .into_iter()
        .for_each(|entry| {
            let install_path =
                test_workspace.join(pathdiff::diff_paths(entry.path(), workspace_root).unwrap());
            fs::create_dir_all(install_path.parent().unwrap()).unwrap();
            fs::copy(entry.path(), install_path).unwrap();
        });

    // Extract repository archives and generate `override_repository` commands
    let override_commands = opts
        .repo_archives
        .iter()
        .map(|repo_archive| {
            let repo_path = test_home.join(&repo_archive.name);
            fs::create_dir_all(&repo_path).unwrap();

            // Extract the archive
            let tar_file = File::open(&repo_archive.archive).unwrap();
            let tar = GzDecoder::new(tar_file);
            let mut archive = Archive::new(tar);
            archive.unpack(&repo_path).unwrap();

            // Generate the override command
            format!(
                "common --override_repository={}='{}'",
                repo_archive.name,
                repo_path.to_string_lossy(),
            )
        })
        .collect();

    // Write bazelrc with overrides and handy flags
    let bazel_rc_content: Vec<String> = vec![
        override_commands,
        vec![
            "".to_owned(),
            "build --curses=no".to_owned(),
            "common --announce_rc".to_owned(),
            "startup --max_idle_secs=1".to_owned(),
            "test --test_output=errors".to_owned(),
            "".to_owned(),
        ],
        // TODO: These should be set by arguments
        vec![
            "build --aspects=@rules_rust//rust:defs.bzl%rustfmt_aspect".to_owned(),
            "build --output_groups=+rustfmt_checks".to_owned(),
            "build --aspects=@rules_rust//rust:defs.bzl%rust_clippy_aspect".to_owned(),
            "build --output_groups=+clippy_checks".to_owned(),
            "".to_owned(),
        ],
    ]
    .into_iter()
    .flatten()
    .collect();
    fs::write(&test_home.join(".bazelrc"), bazel_rc_content.join("\n")).unwrap();

    // Generate a map of environment variables
    let current_dir = env::current_dir().unwrap();
    let envs: HashMap<String, String> = opts
        .envs
        .iter()
        .map(|var| {
            (
                var.clone(),
                env::var(var)
                    .unwrap()
                    // Allow users to inject the current working directory into variables
                    .replace("${pwd}", &current_dir.to_string_lossy().to_string()),
            )
        })
        .collect();

    // Run Bazel test //...
    let status = process::Command::new(current_dir.join(opts.bazel_bin))
        .current_dir(test_workspace)
        .arg("test")
        .arg("//...")
        .env("HOME", test_home.to_string_lossy().to_string())
        .envs(envs)
        .status()
        .unwrap();

    process::exit(status.code().unwrap())
}

fn main() {
    match parse_args() {
        Options::DeletePackages(opts) => deleted_packages(opts),
        Options::Integration(opts) => integration(opts),
    }
}
