"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "e264d59cfa9b8591751dcabb54472111425c9631275c7b165ded26f5156553c5",
        strip_prefix = "rules_rust-69a738a701fde54686f1220d5968f048c0fa89f8",
        urls = [
            # `main` branch as of 2022-01-06
            "https://github.com/bazelbuild/rules_rust/archive/69a738a701fde54686f1220d5968f048c0fa89f8.tar.gz",
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
