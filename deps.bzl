"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "9aa5373aabd6fb523c5237b1dd8677ea028bf9d90561e16d57c1313e19c4d9a7",
        strip_prefix = "rules_rust-0c53d0e6d36689e22a4f7ebd0fd227b690d0f6e9",
        urls = [
            # `main` branch as of 2021-10-27
            "https://github.com/bazelbuild/rules_rust/archive/0c53d0e6d36689e22a4f7ebd0fd227b690d0f6e9.tar.gz",
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
