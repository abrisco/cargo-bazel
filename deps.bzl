"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "82b30cbb46c61a9014de0a8d0443a45e6eb6bd9add35ab421cfb1943dc3271f5",
        strip_prefix = "rules_rust-e589105b4e8181dd1d0d8ccaa0cf3267efb06e86",
        urls = [
            # `main` branch as of 2021-09-21
            "https://github.com/bazelbuild/rules_rust/archive/e589105b4e8181dd1d0d8ccaa0cf3267efb06e86.tar.gz",
        ],
    )

    maybe(
        http_archive,
        name = "bazel_skylib",
        urls = [
            "https://github.com/bazelbuild/bazel-skylib/releases/download/1.1.1/bazel-skylib-1.1.1.tar.gz",
            "https://mirror.bazel.build/github.com/bazelbuild/bazel-skylib/releases/download/1.1.1/bazel-skylib-1.1.1.tar.gz",
        ],
        sha256 = "c6966ec828da198c5d9adbaa94c05e3a1c7f21bd012a0b29ba8ddbccb2c93b0d",
    )
