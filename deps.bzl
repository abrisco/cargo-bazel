"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "8e190ea711500bf076f8de6c4c2729ac0d676a992a3d8aefb409f1e786a3f080",
        strip_prefix = "rules_rust-c435cf4478fc6e097edc5dba0e71de6608ab77d8",
        urls = [
            # `main` branch as of 2022-02-10
            "https://github.com/bazelbuild/rules_rust/archive/c435cf4478fc6e097edc5dba0e71de6608ab77d8.tar.gz",
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
