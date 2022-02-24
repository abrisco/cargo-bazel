"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "7240a4865b11427cc58cd00b3e89c805825bfd3cc4c225b7e992a58622bec859",
        strip_prefix = "rules_rust-a619e1a30bb274639b6d2ccb76c820a02b9f94be",
        urls = [
            # `main` branch as of 2022-02-23
            "https://github.com/bazelbuild/rules_rust/archive/a619e1a30bb274639b6d2ccb76c820a02b9f94be.tar.gz",
        ],
    )

    maybe(
        http_archive,
        name = "bazel_skylib",
        urls = [
            "https://mirror.bazel.build/github.com/bazelbuild/bazel-skylib/releases/download/1.2.0/bazel-skylib-1.2.0.tar.gz",
            "https://github.com/bazelbuild/bazel-skylib/releases/download/1.2.0/bazel-skylib-1.2.0.tar.gz",
        ],
        sha256 = "af87959afe497dc8dfd4c6cb66e1279cb98ccc84284619ebfec27d9c09a903de",
    )

    third_party_deps()
