# Cargo->Bazel

`cargo-bazel` is a Bazel repository rule for generating Rust targets using Cargo.

## Setup

```python
load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

# To get an accurate and up to date repository definition, See the releases page
# https://github.com/abrisco/cargo-bazel/releases
http_archive(
    name = "cargo_bazel",
    sha256 = "{sha256}",
    urls = ["https://github.com/abrisco/cargo-bazel/releases/download/{version}/cargo_bazel.tar.gz"],
)

load("@cargo_bazel//:deps.bzl", "cargo_bazel_deps")

cargo_bazel_deps()

# It's important to set a constant for desired Rust version so it
# can easily be passed to each `crates_repository` definition.
RUST_VERSION = "1.54.0"

load("@rules_rust//rust:repositories.bzl", "rust_repositories")

rust_repositories(version = RUST_VERSION)
```

## `crates_repository` Workflows

The [`crates_repository`][cr] rule (the primary repository rule of `cargo-bazel`) supports a number of different ways users
can express and organize their dependencies. The most common are listed below though there are more to be found in
the [./examples](https://github.com/abrisco/cargo-bazel/tree/main/examples) directory.

### Cargo Workspaces

One of the simpler ways to wire up dependencies would be to first structure your project into a [Cargo workspace][cw].
The `crates_repository` rule can ingest a the root `Cargo.toml` file and generate dependencies from there.

```python
load("@cargo_bazel//:defs.bzl", "crate", "crates_repository")

crates_repository(
    name = "crate_index",
    lockfile = "//:Cargo.Bazel.lock",
    manifests = ["//:Cargo.toml"],
)

load("@crate_index//:defs.bzl", "crate_repositories")

crate_repositories()
```

The generated `crates_repository` contains helper macros which make collecting dependencies for Bazel targets simpler.
Notably, the `all_crate_deps` and `aliases` macros commonly allow the `Cargo.toml` files to be the single source of
truth for dependencies. Since these macros come from the generated repository, the dependencies and alias definitions
they return will automatically update BUILD targets.

```python
load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "lib",
    aliases = aliases(),
    deps = all_crate_deps(
        normal = True,
    ),
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
)

rust_test(
    name = "unit_test",
    crate = ":lib",
    aliases = aliases(
        normal_dev = True,
        proc_macro_dev = True,
    ),
    deps = all_crate_deps(
        normal_dev = True,
    ),
    proc_macro_deps = all_crate_deps(
        proc_macro_dev = True,
    ),
)
```

### Direct Packages

In cases where Rust targets have heavy interractions with other Bazel targests ([Cc][cc], [Proto][proto], etc.),
maintaining `Cargo.toml` files may have deminishing returns as things like [rust-analyzer][ra] begin to be confused
about missing targets or environment variables defined only in Bazel. In workspaces like this, it may be desirable
to have a "Cargo free" setup. `crates_repository` supports this through the `packages` attribute.

```python
load("@cargo_bazel//:defs.bzl", "crate", "crates_repository", "render_config")

crates_repository(
    name = "crate_index",
    lockfile = "//:Cargo.Bazel.lock",
    packages = {
        "async-trait": crate.spec(
            version = "0.1.51",
        ),
        "mockall": crate.spec(
            version = "0.10.2",
        ),
        "tokio": crate.spec(
            version = "1.12.0",
        ),
    },
    # Setting the default package name to `""` forces the use of the macros defined in this repository
    # to always use the root package when looking for dependencies or aliases. This should be considered
    # optional as the repository also exposes alises for easy access to all dependencies.
    render_config = render_config(
        default_package_name = ""
    ),
)

load("@crate_index//:defs.bzl", "crate_repositories")

crate_repositories()
```

Consuming dependencies may be more ergonomic in this case through the aliases defined in the new repository.

```python
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "lib",
    deps = [
        "@crate_index//:tokio",
    ],
    proc_macro_deps = [
        "@crate_index//:async-trait",
    ],
)

rust_test(
    name = "unit_test",
    crate = ":lib",
    deps = [
        "@crate_index//:mockall",
    ],
)
```

[cw]: https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
[cr]: ./rules.md#crates_repository
[cc]: https://docs.bazel.build/versions/main/be/c-cpp.html
[proto]: https://rules-proto-grpc.com/en/latest/lang/rust.html
[ra]: https://rust-analyzer.github.io/
