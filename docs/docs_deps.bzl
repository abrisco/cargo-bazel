"""Dependencies required for generating documentation"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")

def docs_deps():
    """A macro defining dependencies used for generating documentation"""
    maybe(
        http_archive,
        name = "io_bazel_stardoc",
        sha256 = "c9794dcc8026a30ff67cf7cf91ebe245ca294b20b071845d12c192afe243ad72",
        urls = [
            "https://mirror.bazel.build/github.com/bazelbuild/stardoc/releases/download/0.5.0/stardoc-0.5.0.tar.gz",
            "https://github.com/bazelbuild/stardoc/releases/download/0.5.0/stardoc-0.5.0.tar.gz",
        ],
    )

    mdbook_version = "0.4.13"
    mdbook_components = [
        ("x86_64-apple-darwin", "c3a92c36700bd037b8eea27056e66f64ab279c21343dfee7b14e750795624f6c", "tar.gz"),
        ("x86_64-pc-windows-msvc", "093a5dcfccb0d5615d0b8ace4389d0c1af6d71ff25b9e86bfb0ffb7403e434e9", "zip"),
        ("x86_64-unknown-linux-gnu", "f040334cc62a3779c23a0df6eb648de445ca747d8f87956927a9b13c5bffff40", "tar.gz"),
    ]

    for (triple, sha256, archive) in mdbook_components:
        maybe(
            http_archive,
            name = "mdbook_{}".format(triple),
            sha256 = sha256,
            urls = [
                "https://github.com/rust-lang/mdBook/releases/download/v{version}/mdbook-v{version}-{triple}.{archive}".format(
                    version = mdbook_version,
                    triple = triple,
                    archive = archive,
                ),
            ],
            build_file_content = """exports_files(glob(["**"]), visibility = ["//visibility:public"])""",
        )

def mdbook_binary(name = "mdbook"):
    native.config_setting(
        name = "linux",
        constraint_values = ["@platforms//os:linux"],
    )

    native.config_setting(
        name = "macos",
        constraint_values = ["@platforms//os:macos"],
    )

    native.config_setting(
        name = "windows",
        constraint_values = ["@platforms//os:windows"],
    )

    native.alias(
        name = name,
        actual = select({
            ":linux": "@mdbook_x86_64-unknown-linux-gnu//:mdbook",
            ":macos": "@mdbook_x86_64-apple-darwin//:mdbook",
            ":windows": "@mdbook_x86_64-pc-windows-msvc//:mdbook.exe",
        }),
    )
