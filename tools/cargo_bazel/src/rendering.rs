//! Tools for rendering and writing BUILD and other Starlark files

mod template_engine;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result};

use crate::config::RenderConfig;
use crate::context::Context;
use crate::rendering::template_engine::TemplateEngine;

pub struct Renderer {
    config: RenderConfig,
    engine: TemplateEngine,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        let engine = TemplateEngine::new(&config);
        Self { config, engine }
    }

    pub fn render(&self, context: &Context) -> Result<BTreeMap<PathBuf, String>> {
        let mut output = BTreeMap::new();

        output.extend(self.render_build_files(context)?);
        output.extend(self.render_crates_module(context)?);

        Ok(output)
    }

    fn render_crates_module(&self, context: &Context) -> Result<BTreeMap<PathBuf, String>> {
        let mut map = BTreeMap::new();
        map.insert(
            PathBuf::from("defs.bzl"),
            self.engine.render_module_bzl(context)?,
        );
        map.insert(
            PathBuf::from("BUILD.bazel"),
            self.engine.render_module_build_file(context)?,
        );

        Ok(map)
    }

    fn render_build_files(&self, context: &Context) -> Result<BTreeMap<PathBuf, String>> {
        Ok(self
            .engine
            .render_crate_build_files(context)?
            .into_iter()
            .map(|(id, content)| {
                let ctx = &context.crates[id];
                let filename = render_build_file_template(
                    &self.config.build_file_template,
                    &ctx.name,
                    &ctx.version,
                );
                (filename, content)
            })
            .collect())
    }
}

/// Write a set of [CrateContext][crate::context::CrateContext] to disk.
pub fn write_outputs(
    outputs: BTreeMap<PathBuf, String>,
    out_dir: &Path,
    dry_run: bool,
) -> Result<()> {
    let outputs: BTreeMap<PathBuf, String> = outputs
        .into_iter()
        .map(|(path, content)| (out_dir.join(path), content))
        .collect();

    if dry_run {
        println!("{:#?}", outputs);
    } else {
        // Ensure the output directory exists
        fs::create_dir_all(out_dir)?;

        for (path, content) in outputs {
            fs::write(&path, content.as_bytes())
                .context(format!("Failed to write file to disk: {}", path.display()))?;
        }
    }

    Ok(())
}

/// Render the Bazel label of a crate
pub fn render_crate_bazel_label(
    template: &str,
    repository_name: &str,
    name: &str,
    version: &str,
    target: &str,
) -> String {
    template
        .replace("{repository}", repository_name)
        .replace("{name}", name)
        .replace("{version}", version)
        .replace("{target}", target)
}

/// Render the Bazel label of a crate
pub fn render_crate_bazel_repository(
    template: &str,
    repository_name: &str,
    name: &str,
    version: &str,
) -> String {
    template
        .replace("{repository}", repository_name)
        .replace("{name}", name)
        .replace("{version}", version)
}

/// Render the Bazel label of a crate
pub fn render_crate_build_file(template: &str, name: &str, version: &str) -> String {
    template
        .replace("{name}", name)
        .replace("{version}", version)
}

/// Render the Bazel label of a platform triple
pub fn render_platform_constraint_label(template: &str, triple: &str) -> String {
    template.replace("{triple}", triple)
}

pub fn render_build_file_template(template: &str, name: &str, version: &str) -> PathBuf {
    PathBuf::from(
        template
            .replace("{name}", name)
            .replace("{version}", version),
    )
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::config::CrateId;
    use crate::context::crate_context::{CrateContext, Rule};
    use crate::context::{BuildScriptAttributes, Context, TargetAttributes};

    fn mock_render_config() -> RenderConfig {
        serde_json::from_value(serde_json::json!({
            "repository_name": "test_rendering"
        }))
        .unwrap()
    }

    fn mock_target_attributes() -> TargetAttributes {
        TargetAttributes {
            crate_name: "mock_crate".to_owned(),
            crate_root: Some("src/root.rs".to_owned()),
            ..TargetAttributes::default()
        }
    }

    #[test]
    fn render_rust_library() {
        let mut context = Context::default();
        let crate_id = CrateId::new("mock_crate".to_owned(), "0.1.0".to_owned());
        context.crates.insert(
            crate_id.clone(),
            CrateContext {
                name: crate_id.name,
                version: crate_id.version,
                targets: vec![Rule::Library(mock_target_attributes())],
                ..CrateContext::default()
            },
        );

        let renderer = Renderer::new(mock_render_config());
        let output = renderer.render(&context).unwrap();

        let build_file_content = output
            .get(&PathBuf::from("BUILD.mock_crate-0.1.0.bazel"))
            .unwrap();

        assert!(build_file_content.contains("rust_library("));
        assert!(build_file_content.contains("name = \"mock_crate\""));
    }

    #[test]
    fn render_cargo_build_script() {
        let mut context = Context::default();
        let crate_id = CrateId::new("mock_crate".to_owned(), "0.1.0".to_owned());
        context.crates.insert(
            crate_id.clone(),
            CrateContext {
                name: crate_id.name,
                version: crate_id.version,
                targets: vec![Rule::BuildScript(TargetAttributes {
                    crate_name: "build_script_build".to_owned(),
                    crate_root: Some("build.rs".to_owned()),
                    ..TargetAttributes::default()
                })],
                // Build script attributes are required.
                build_script_attrs: Some(BuildScriptAttributes::default()),
                ..CrateContext::default()
            },
        );

        let renderer = Renderer::new(mock_render_config());
        let output = renderer.render(&context).unwrap();

        let build_file_content = output
            .get(&PathBuf::from("BUILD.mock_crate-0.1.0.bazel"))
            .unwrap();

        assert!(build_file_content.contains("cargo_build_script("));
        assert!(build_file_content.contains("name = \"build_script_build\""));

        // Ensure `cargo_build_script` requirements are met
        assert!(build_file_content.contains("name = \"mock_crate_build_script\""));
    }

    #[test]
    fn render_proc_macro() {
        let mut context = Context::default();
        let crate_id = CrateId::new("mock_crate".to_owned(), "0.1.0".to_owned());
        context.crates.insert(
            crate_id.clone(),
            CrateContext {
                name: crate_id.name,
                version: crate_id.version,
                targets: vec![Rule::ProcMacro(mock_target_attributes())],
                ..CrateContext::default()
            },
        );

        let renderer = Renderer::new(mock_render_config());
        let output = renderer.render(&context).unwrap();

        let build_file_content = output
            .get(&PathBuf::from("BUILD.mock_crate-0.1.0.bazel"))
            .unwrap();

        assert!(build_file_content.contains("rust_proc_macro("));
        assert!(build_file_content.contains("name = \"mock_crate\""));
    }

    #[test]
    fn render_binary() {
        let mut context = Context::default();
        let crate_id = CrateId::new("mock_crate".to_owned(), "0.1.0".to_owned());
        context.crates.insert(
            crate_id.clone(),
            CrateContext {
                name: crate_id.name,
                version: crate_id.version,
                targets: vec![Rule::Binary(mock_target_attributes())],
                ..CrateContext::default()
            },
        );

        let renderer = Renderer::new(mock_render_config());
        let output = renderer.render(&context).unwrap();

        let build_file_content = output
            .get(&PathBuf::from("BUILD.mock_crate-0.1.0.bazel"))
            .unwrap();

        assert!(build_file_content.contains("rust_binary("));
        assert!(build_file_content.contains("name = \"mock_crate__bin\""));
    }

    #[test]
    fn render_additive_build_contents() {
        let mut context = Context::default();
        let crate_id = CrateId::new("mock_crate".to_owned(), "0.1.0".to_owned());
        context.crates.insert(
            crate_id.clone(),
            CrateContext {
                name: crate_id.name,
                version: crate_id.version,
                targets: vec![Rule::Binary(mock_target_attributes())],
                additive_build_file_content: Some(
                    "# Hello World from additive section!".to_owned(),
                ),
                ..CrateContext::default()
            },
        );

        let renderer = Renderer::new(mock_render_config());
        let output = renderer.render(&context).unwrap();

        let build_file_content = output
            .get(&PathBuf::from("BUILD.mock_crate-0.1.0.bazel"))
            .unwrap();

        assert!(build_file_content.contains("# Hello World from additive section!"));
    }
}
