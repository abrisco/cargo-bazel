//! TODO

use anyhow::{bail, Result};

use crate::annotation::Annotations;
use crate::cli::opt::GenerateOptions;
use crate::config::Config;
use crate::context::Context;
use crate::lockfile::{is_cargo_lockfile, write_lockfile, LockfileKind};
use crate::metadata::{Generator, MetadataGenerator};
use crate::rendering::{write_outputs, Renderer};

pub fn generate(opt: GenerateOptions) -> Result<()> {
    // Load the config
    let config = Config::try_from_path(&opt.config)?;

    // Determine if the dependencies need to be repinned.
    let mut should_repin = opt.repin;

    // Cargo lockfiles must always be repinned.
    if is_cargo_lockfile(&opt.lockfile, &opt.lockfile_kind) {
        should_repin = true;
    }

    // Go straight to rendering if there is no need to repin
    if !should_repin {
        let context = Context::try_from_path(opt.lockfile)?;

        // Render build files
        let outputs = Renderer::new(config.rendering).render(&context)?;

        // Write the outputs to disk
        write_outputs(outputs, &opt.repository_dir, opt.dry_run)?;

        return Ok(());
    }

    // Ensure Cargo and Rustc are available for use during generation.
    let cargo_bin = match &opt.cargo {
        Some(bin) => bin,
        None => bail!("The `--cargo` argument is required when generating unpinned content"),
    };
    let rustc_bin = match &opt.rustc {
        Some(bin) => bin,
        None => bail!("The `--rustc` argument is required when generating unpinned content"),
    };

    // Generate Metadata
    let mut metadata_generator = Generator::new()
        .with_cargo(cargo_bin.clone())
        .with_rustc(rustc_bin.clone());

    // Optionally use the Cargo lockfile if one was provided
    if is_cargo_lockfile(&opt.lockfile, &opt.lockfile_kind) {
        metadata_generator = metadata_generator.with_cargo_lockfile(&opt.lockfile)?;
    }

    let (cargo_metadata, cargo_lockfile) = metadata_generator.generate(match &opt.manifest {
        Some(path) => path,
        None => bail!("The `--manifest` argument is required when repinning dependencies"),
    })?;

    // Copy the rendering config for later use
    let render_config = config.rendering.clone();

    // Annotate metadata
    let annotations = Annotations::new(cargo_metadata, cargo_lockfile, config)?;

    // Generate renderable contexts for earch package
    let context = Context::new(annotations, cargo_bin, rustc_bin)?;

    // Render build files
    let outputs = Renderer::new(render_config).render(&context)?;

    // Write outputs
    write_outputs(outputs, &opt.repository_dir, opt.dry_run)?;

    // Ensure Bazel lockfiles are written to disk so future generations can be short-circuted.
    if matches!(opt.lockfile_kind, LockfileKind::Bazel) {
        write_lockfile(context, &opt.lockfile, opt.dry_run)?;
    }

    Ok(())
}
