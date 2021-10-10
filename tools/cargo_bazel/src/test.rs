//! A module containing common test helpers

pub fn mock_cargo_metadata_package() -> cargo_metadata::Package {
    serde_json::from_value(serde_json::json!({
        "name": "mock-pkg",
        "version": "3.3.3",
        "id": "mock-pkg 3.3.3 (registry+https://github.com/rust-lang/crates.io-index)",
        "license": "Unlicense/MIT",
        "license_file": null,
        "description": "Fast multiple substring searching.",
        "source": "registry+https://github.com/rust-lang/crates.io-index",
        "dependencies": [],
        "targets": [],
        "features": {},
        "manifest_path": "/tmp/mock-pkg-3.3.3/Cargo.toml",
        "metadata": null,
        "publish": null,
        "authors": [],
        "categories": [],
        "keywords": [],
        "readme": "README.md",
        "repository": "",
        "homepage": "",
        "documentation": null,
        "edition": "2021",
        "links": null,
        "default_run": null
    }))
    .unwrap()
}

pub fn mock_cargo_lock_package() -> cargo_lock::Package {
    toml::from_str(&textwrap::dedent(
        r#"
        name = "mock-pkg"
        version = "3.3.3"
        source = "registry+https://github.com/rust-lang/crates.io-index"
        checksum = "ee49baf6cb617b853aa8d93bf420db2383fab46d314482ca2803b40d5fde979b"
        dependencies = []
        "#,
    ))
    .unwrap()
}

/// Clone and compare two items after calling `.sort()` on them.
macro_rules! assert_sort_eq {
    ($left:expr, $right:expr $(,)?) => {
        let mut left = $left.clone();
        left.sort();
        let mut right = $right.clone();
        right.sort();
        assert_eq!(left, right);
    };
}
pub(crate) use assert_sort_eq;
