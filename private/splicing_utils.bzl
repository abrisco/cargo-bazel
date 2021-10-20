"""Utilities directly related to the `splicing` step of `cargo-bazel`."""

load(":common_utils.bzl", "execute")

def download_extra_workspace_members(repository_ctx, cache_dir, render_template_registry_url):
    """Download additional workspace members for use in splicing.

    Args:
        repository_ctx (repository_ctx): The rule's context object.
        cache_dir (path): A directory in which to download and extract extra workspace members
        render_template_registry_url (str): The base template to use for determining the crate's registry URL.

    Returns:
        list: A list of information related to the downloaded crates
            - manifest: The path of the manifest.
            - url: The url the manifest came from.
            - sha256: The sha256 checksum of the new manifest.
    """
    manifests = []
    extra_workspace_members = repository_ctx.attr.extra_workspace_members
    if extra_workspace_members:
        repository_ctx.report_progress("Downloading extra workspace members.")

    for name, spec in repository_ctx.attr.extra_workspace_members.items():
        spec = struct(**json.decode(spec))

        url = render_template_registry_url
        url = url.replace("{name}", name)
        url = url.replace("{version}", spec.version)

        if spec.sha256:
            result = repository_ctx.download_and_extract(
                output = cache_dir,
                url = url,
                sha256 = spec.sha256,
                type = "tar.gz",
            )
        else:
            result = repository_ctx.download_and_extract(
                output = cache_dir,
                url = url,
                type = "tar.gz",
            )

        manifest = repository_ctx.path("{}/{}-{}/Cargo.toml".format(
            cache_dir,
            name,
            spec.version,
        ))

        if not manifest.exists:
            fail("Extra workspace member '{}' has no root Cargo.toml file".format(name))

        manifests.append(struct(
            manifest = str(manifest),
            url = url,
            sha256 = result.sha256,
        ))

    return manifests

def splice_workspace_manifest(repository_ctx, generator, lockfile, cargo, rustc):
    """Splice together a Cargo workspace from various other manifests and package definitions

    Args:
        repository_ctx (repository_ctx): The rule's context object.
        generator (path): The `cargo-bazel` binary.
        lockfile (path): The path to a "lock" file for reproducible `cargo-bazel` renderings.
        cargo (path): The path to a Cargo binary.
        rustc (path): The Path to a Rustc binary.

    Returns:
        path: The path to a Cargo metadata json file found in the spliced workspace root.
    """
    repository_ctx.report_progress("Splicing Cargo workspace.")
    repo_dir = repository_ctx.path(".")

    # Download extra workspace members
    crates_cache_dir = repository_ctx.path("{}/.crates_cache".format(repo_dir))
    extra_manifest_info = download_extra_workspace_members(
        repository_ctx = repository_ctx,
        cache_dir = crates_cache_dir,
        render_template_registry_url = repository_ctx.attr.extra_workspace_member_url_template,
    )

    # Deserialize information about direct packges
    direct_packages_info = {
        pkg: json.decode(data)
        for (pkg, data) in repository_ctx.attr.packages.items()
    }

    manifests = {str(repository_ctx.path(m)): str(m) for m in repository_ctx.attr.manifests}

    # Serialize information required for splicing
    splicing_manifest = repository_ctx.path("{}/splicing_manifest.json".format(repo_dir))
    repository_ctx.file(
        splicing_manifest,
        json.encode_indent(struct(
            direct_packages = direct_packages_info,
            extra_manifest_infos = extra_manifest_info,
            manifests = manifests,
        ), indent = " " * 4),
    )

    cargo_workspace = repository_ctx.path("{}/cargo-bazel-splicing".format(repo_dir))

    # Generate a workspace root which contains all workspace members
    arguments = [
        generator,
        "splice",
        "--workspace-dir",
        cargo_workspace,
        "--splicing-manifest",
        splicing_manifest,
        "--cargo",
        cargo,
        "--rustc",
        rustc,
    ]

    # Splicing accepts a Cargo.lock file in some scenarios. Ensure it's passed
    # if the lockfile is a actually a Cargo lockfile.
    if lockfile.kind == "cargo":
        arguments.extend([
            "--cargo-lockfile",
            lockfile.path,
        ])

    result = execute(
        repository_ctx = repository_ctx,
        args = arguments,
        env = {
            "CARGO": str(cargo),
            "RUSTC": str(rustc),
            "RUST_BACKTRACE": "full",
        },
    )

    root_manifest = repository_ctx.path("{}/Cargo.toml".format(cargo_workspace))
    if not root_manifest.exists:
        fail("Root manifest does not exist: {}".format(root_manifest))

    # This file must match the one generated in splicing
    metadata_path = repository_ctx.path("{}/cargo-bazel-spliced-metadata.json".format(cargo_workspace))
    if not metadata_path.exists:
        fail("Root metadata file does not exist: {}".format(metadata_path))

    return metadata_path
