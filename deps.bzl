"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "1a919f80faf6a5e3ee1d0fccf84775c5f6f6ee062eb413dd9f7560b6b02008bb",
        strip_prefix = "rules_rust-1cb3c446b263c16b373e259e988f00c5f1e3f175",
        urls = [
            # `main` branch as of 2022-01-27
            "https://github.com/bazelbuild/rules_rust/archive/1cb3c446b263c16b373e259e988f00c5f1e3f175.tar.gz",
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

    third_party_deps()
