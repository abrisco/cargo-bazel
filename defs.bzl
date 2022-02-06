"""# Rules

- [crates_repository](#crates_repository)
- [crate.spec](#cratespec)
- [crate.workspace_member](#crateworkspace_member)
- [crate.annotation](#crateannotation)
- [render_config](#render_config)
- [splicing_config](#splicing_config)

"""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("@rules_rust//rust/platform:triple_mappings.bzl", "SUPPORTED_PLATFORM_TRIPLES")
load("//private:common_utils.bzl", "get_host_triple", "get_rust_tools")
load(
    "//private:generate_utils.bzl",
    "CRATES_REPOSITORY_ENVIRON",
    "determine_repin",
    "execute_generator",
    "generate_config",
    "get_generator",
    "get_lockfile",
    _render_config = "render_config",
)
load(
    "//private:splicing_utils.bzl",
    "create_splicing_manifest",
    "splice_workspace_manifest",
    _splicing_config = "splicing_config",
)
load("//private:urls.bzl", "CARGO_BAZEL_SHA256S", "CARGO_BAZEL_URLS")

# Reexport this symbol so users can easiliy access it from this file.
render_config = _render_config
splicing_config = _splicing_config

def _crates_repository_impl(repository_ctx):
    # Determine the current host's platform triple
    host_triple = get_host_triple(repository_ctx)

    # Locate the generator to use
    generator = get_generator(repository_ctx, host_triple.triple)

    # Generate a config file for all settings
    config = generate_config(repository_ctx)

    # Locate the lockfile
    lockfile = get_lockfile(repository_ctx)

    # Locate Rust tools (cargo, rustc)
    tools = get_rust_tools(repository_ctx, host_triple)
    cargo_path = repository_ctx.path(tools.cargo)
    rustc_path = repository_ctx.path(tools.rustc)

    # Create a manifest of all dependency inputs
    splicing_manifest = create_splicing_manifest(repository_ctx)

    # Determine whether or not to repin depednencies
    repin = determine_repin(
        repository_ctx = repository_ctx,
        generator = generator,
        lockfile_path = lockfile.path,
        lockfile_kind = lockfile.kind,
        config = config.path,
        splicing_manifest = splicing_manifest,
        cargo = cargo_path,
        rustc = rustc_path,
    )

    # If re-pinning is enabled, gather additional inputs for the generator
    kwargs = dict()
    if repin or lockfile.kind == "cargo":
        # Generate a top level Cargo workspace and manifest for use in generation
        metadata_path = splice_workspace_manifest(
            repository_ctx = repository_ctx,
            generator = generator,
            lockfile = lockfile,
            splicing_manifest = splicing_manifest,
            cargo = cargo_path,
            rustc = rustc_path,
        )

        kwargs.update({
            "metadata": metadata_path,
            "repin": True,
        })

    # Run the generator
    execute_generator(
        repository_ctx = repository_ctx,
        generator = generator,
        config = config.path,
        splicing_manifest = splicing_manifest,
        lockfile_path = lockfile.path,
        lockfile_kind = lockfile.kind,
        repository_dir = repository_ctx.path("."),
        cargo = cargo_path,
        rustc = rustc_path,
        # sysroot = tools.sysroot,
        **kwargs
    )

crates_repository = repository_rule(
    doc = """\
A rule for defining and downloading Rust dependencies (crates).

Environment Variables:

| variable | usage |
| --- | --- |
| `CARGO_BAZEL_GENERATOR_SHA256` | The sha256 checksum of the file located at `CARGO_BAZEL_GENERATOR_URL` |
| `CARGO_BAZEL_GENERATOR_URL` | The URL of a cargo-bazel binary. This variable takes precedence over attributes and can use `file://` for local paths |
| `CARGO_BAZEL_ISOLATED` | An authorative flag as to whether or not the `CARGO_HOME` environment variable should be isolated from the host configuration |
| `CARGO_BAZEL_REPIN` | An indicator that the dependencies represented by the rule should be regenerated. `REPIN` may also be used. |

""",
    implementation = _crates_repository_impl,
    attrs = {
        "annotations": attr.string_list_dict(
            doc = "Extra settings to apply to crates. See [crate.annotations](#crateannotations).",
        ),
        "cargo_config": attr.label(
            doc = "A [Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html) file",
        ),
        "extra_workspace_member_url_template": attr.string(
            doc = "The registry url to use when fetching extra workspace members",
            default = "https://crates.io/api/v1/crates/{name}/{version}/download",
        ),
        "extra_workspace_members": attr.string_dict(
            doc = (
                "Additional crates to download and include as a workspace member. This is unfortunately required in " +
                "order to add information about \"binary-only\" crates so that a `rust_binary` may be generated for " +
                "it. [rust-lang/cargo#9096](https://github.com/rust-lang/cargo/issues/9096) tracks an RFC which may " +
                "solve for this."
            ),
        ),
        "generate_build_scripts": attr.bool(
            doc = (
                "Whether or not to generate " +
                "[cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) by default."
            ),
            default = True,
        ),
        "generator": attr.string(
            doc = (
                "The absolute label of a generator. Eg. `@cargo_bazel_bootstrap//:cargo-bazel`. " +
                "This is typically used when bootstrapping"
            ),
        ),
        "generator_sha256s": attr.string_dict(
            doc = "Dictionary of `host_triple` -> `sha256` for a `cargo-bazel` binary.",
            default = CARGO_BAZEL_SHA256S,
        ),
        "generator_urls": attr.string_dict(
            doc = (
                "URL template from which to download the `cargo-bazel` binary. `{host_triple}` and will be " +
                "filled in according to the host platform."
            ),
            default = CARGO_BAZEL_URLS,
        ),
        "isolated": attr.bool(
            doc = (
                "If true, `CARGO_HOME` will be overwritten to a directory within the generated repository in " +
                "order to prevent other uses of Cargo from impacting having any effect on the generated targets " +
                "produced by this rule. For users who either have multiple `crate_repository` definitions in a " +
                "WORKSPACE or rapidly re-pin dependencies, setting this to false may improve build times. This " +
                "variable is also controled by `CARGO_BAZEL_ISOLATED` environment variable."
            ),
            default = True,
        ),
        "lockfile": attr.label(
            doc = (
                "The path to a file to use for reproducible renderings. Two kinds of lock files are supported, " +
                "Cargo (`Cargo.lock` files) and Bazel (custom files generated by this rule, naming is irrelevant). " +
                "Bazel lockfiles should be the prefered kind as they're desigend with Bazel's notions of " +
                "reporducibility in mind. Cargo lockfiles can be used in cases where it's intended to be the " +
                "source of truth, but more work will need to be done to generate BUILD files which are not " +
                "guaranteed to be determinsitic."
            ),
            mandatory = True,
        ),
        "lockfile_kind": attr.string(
            doc = (
                "Two different kinds of lockfiles are supported, the custom \"Bazel\" lockfile, which is generated " +
                "by this rule, and Cargo lockfiles (`Cargo.lock`). This attribute allows for explicitly defining " +
                "the type in cases where it may not be auto-detectable."
            ),
            values = [
                "auto",
                "bazel",
                "cargo",
            ],
            default = "auto",
        ),
        "manifests": attr.label_list(
            doc = "A list of Cargo manifests (`Cargo.toml` files).",
        ),
        "packages": attr.string_dict(
            doc = "A set of crates (packages) specifications to depend on. See [crate.spec](#crate.spec).",
        ),
        "quiet": attr.bool(
            doc = "If stdout and stderr should not be printed to the terminal.",
            default = True,
        ),
        "render_config": attr.string(
            doc = (
                "The configuration flags to use for rendering. Use `@cargo_bazel//:defs.bzl\\%render_config` to " +
                "generate the value for this field. If unset, the defaults defined there will be used."
            ),
        ),
        "rust_toolchain_cargo_template": attr.string(
            doc = (
                "The template to use for finding the host `cargo` binary. `{version}` (eg. '1.53.0'), " +
                "`{triple}` (eg. 'x86_64-unknown-linux-gnu'), `{arch}` (eg. 'aarch64'), `{vendor}` (eg. 'unknown'), " +
                "`{system}` (eg. 'darwin'), `{cfg}` (eg. 'exec'), and `{tool}` (eg. 'rustc.exe') will be replaced in " +
                "the string if present."
            ),
            default = "@rust_{system}_{arch}//:bin/{tool}",
        ),
        "rust_toolchain_rustc_template": attr.string(
            doc = (
                "The template to use for finding the host `rustc` binary. `{version}` (eg. '1.53.0'), " +
                "`{triple}` (eg. 'x86_64-unknown-linux-gnu'), `{arch}` (eg. 'aarch64'), `{vendor}` (eg. 'unknown'), " +
                "`{system}` (eg. 'darwin'), `{cfg}` (eg. 'exec'), and `{tool}` (eg. 'cargo.exe') will be replaced in " +
                "the string if present."
            ),
            default = "@rust_{system}_{arch}//:bin/{tool}",
        ),
        "rust_version": attr.string(
            doc = "The version of Rust the currently registered toolchain is using. Eg. `1.56.0`, or `nightly-2021-09-08`",
            default = rust_common.default_version,
        ),
        "splicing_config": attr.string(
            doc = (
                "The configuration flags to use for splicing Cargo maniests. Use `@cargo_bazel//:defs.bzl\\%rsplicing_config` to " +
                "generate the value for this field. If unset, the defaults defined there will be used."
            ),
        ),
        "supported_platform_triples": attr.string_list(
            doc = "A set of all platform triples to consider when generating dependencies.",
            default = SUPPORTED_PLATFORM_TRIPLES,
        ),
    },
    environ = CRATES_REPOSITORY_ENVIRON,
)

def _workspace_member(version, sha256 = None):
    """Define information for extra workspace members

    Args:
        version (str): The semver of the crate to download. Must be an exact version.
        sha256 (str, optional): The sha256 checksum of the `.crate` file.

    Returns:
        string: A json encoded string of all inputs
    """
    return json.encode(struct(
        version = version,
        sha256 = sha256,
    ))

def _spec(
        package = None,
        version = None,
        default_features = True,
        features = [],
        git = None,
        rev = None):
    """A constructor for a crate dependency.

    See [specifying dependencies][sd] in the Cargo book for more details.

    [sd]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html

    Args:
        package (str, optional): The explicit name of the package (used when attempting to alias a crate).
        version (str, optional): The exact version of the crate. Cannot be used with `git`.
        default_features (bool, optional): Maps to the `default-features` flag.
        features (list, optional): A list of features to use for the crate
        git (str, optional): The Git url to use for the crate. Cannot be used with `version`.
        rev (str, optional): The git revision of the remote crate. Tied with the `git` param.

    Returns:
        string: A json encoded string of all inputs
    """
    return json.encode(struct(
        package = package,
        default_features = default_features,
        features = features,
        version = version,
        git = git,
        rev = rev,
    ))

def _assert_absolute(label):
    """Ensure a given label is an absolute label

    Args:
        label (Label): The label to check
    """
    label_str = str(label)
    if not label.startswith("@"):
        fail("The labels must be absolute. Please update '{}'".format(
            label_str,
        ))

def _annotation(
        version = "*",
        additive_build_file = None,
        additive_build_file_content = None,
        build_script_data = None,
        build_script_data_glob = None,
        build_script_deps = None,
        build_script_env = None,
        build_script_proc_macro_deps = None,
        build_script_rustc_env = None,
        compile_data = None,
        compile_data_glob = None,
        crate_features = None,
        data = None,
        data_glob = None,
        deps = None,
        gen_build_script = None,
        patch_args = None,
        patch_tool = None,
        patches = None,
        proc_macro_deps = None,
        rustc_env = None,
        rustc_env_files = None,
        rustc_flags = None,
        shallow_since = None):
    """A collection of extra attributes and settings for a particular crate

    Args:
        version (str, optional): The version or semver-conditions to match with a crate.
        additive_build_file_content (str, optional): Extra contents to write to the bottom of generated BUILD files.
        additive_build_file (str, optional): A file containing extra contents to write to the bottom of
            generated BUILD files.
        build_script_data (list, optional): A list of labels to add to a crate's `cargo_build_script::data` attribute.
        build_script_data_glob (list, optional): A list of glob patterns to add to a crate's `cargo_build_script::data`
            attribute.
        build_script_deps (list, optional): A list of labels to add to a crate's `cargo_build_script::deps` attribute.
        build_script_env (dict, optional): Additional environment variables to set on a crate's
            `cargo_build_script::env` attribute.
        build_script_proc_macro_deps (list, optional): A list of labels to add to a crate's
            `cargo_build_script::proc_macro_deps` attribute.
        build_script_rustc_env (dict, optional): Additional environment variables to set on a crate's
            `cargo_build_script::env` attribute.
        compile_data (list, optional): A list of labels to add to a crate's `rust_library::compile_data` attribute.
        compile_data_glob (list, optional): A list of glob patterns to add to a crate's `rust_library::compile_data`
            attribute.
        crate_features (list, optional): A list of strings to add to a crate's `rust_library::crate_features`
            attribute.
        data (list, optional): A list of labels to add to a crate's `rust_library::data` attribute.
        data_glob (list, optional): A list of glob patterns to add to a crate's `rust_library::data` attribute.
        deps (list, optional): A list of labels to add to a crate's `rust_library::deps` attribute.
        gen_build_script (bool, optional): An authorative flag to determine whether or not to produce
            `cargo_build_script` targets for the current crate.
        patch_args (list, optional): The `patch_args` attribute of a Bazel repository rule. See
            [http_archive.patch_args](https://docs.bazel.build/versions/main/repo/http.html#http_archive-patch_args)
        patch_tool (list, optional): The `patch_tool` attribute of a Bazel repository rule. See
            [http_archive.patch_tool](https://docs.bazel.build/versions/main/repo/http.html#http_archive-patch_tool)
        patches (list, optional): The `patches` attribute of a Bazel repository rule. See
            [http_archive.patches](https://docs.bazel.build/versions/main/repo/http.html#http_archive-patches)
        proc_macro_deps (list, optional): A list of labels to add to a crate's `rust_library::proc_macro_deps`
            attribute.
        rustc_env (dict, optional): Additional variables to set on a crate's `rust_library::rustc_env` attribute.
        rustc_env_files (list, optional): A list of labels to set on a crate's `rust_library::rustc_env_files`
            attribute.
        rustc_flags (list, optional): A list of strings to set on a crate's `rust_library::rustc_flags` attribute.
        shallow_since (str, optional): An optional timestamp used for crates originating from a git repository
            instead of a crate registry. This flag optimizes fetching the source code.

    Returns:
        string: A json encoded string containing the specified version and separately all other inputs.
    """
    if additive_build_file:
        _assert_absolute(additive_build_file)
    if patches:
        for patch in patches:
            _assert_absolute(patch)

    return json.encode((
        version,
        struct(
            additive_build_file = additive_build_file,
            additive_build_file_content = additive_build_file_content,
            build_script_data = build_script_data,
            build_script_data_glob = build_script_data_glob,
            build_script_deps = build_script_deps,
            build_script_env = build_script_env,
            build_script_proc_macro_deps = build_script_proc_macro_deps,
            build_script_rustc_env = build_script_rustc_env,
            compile_data = compile_data,
            compile_data_glob = compile_data_glob,
            crate_features = crate_features,
            data = data,
            data_glob = data_glob,
            deps = deps,
            gen_build_script = gen_build_script,
            patch_args = patch_args,
            patch_tool = patch_tool,
            patches = patches,
            proc_macro_deps = proc_macro_deps,
            rustc_env = rustc_env,
            rustc_env_files = rustc_env_files,
            rustc_flags = rustc_flags,
            shallow_since = shallow_since,
        ),
    ))

crate = struct(
    spec = _spec,
    annotation = _annotation,
    workspace_member = _workspace_member,
)
