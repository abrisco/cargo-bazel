use std::path::PathBuf;

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
