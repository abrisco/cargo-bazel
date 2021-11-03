"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "d6a8bc37502f252ef190b37945c922e0d0104dc2250940a3ea5f9c42e7a0dc10",
        strip_prefix = "rules_rust-e2f0fccda912daac686b533ad77c5bc5d2f2ddb7",
        urls = [
            # `main` branch as of 2021-11-04
            "https://github.com/UebelAndre/rules_rust/archive/e2f0fccda912daac686b533ad77c5bc5d2f2ddb7.tar.gz",
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
