"""Dependencies required by the `cargo-bazel` rules"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")
load("//3rdparty:third_party_deps.bzl", "third_party_deps")

def cargo_bazel_deps():
    maybe(
        http_archive,
        name = "rules_rust",
        sha256 = "69934a7953e5267ac18b751d6b109bc9e906d4a5d887f68cdd1940f172b64254",
        strip_prefix = "rules_rust-e7b1e5ab9afcccc8d301c2b8922cae815d20c714",
        urls = [
            # `main` branch as of 2021-11-15
            "https://github.com/bazelbuild/rules_rust/archive/e7b1e5ab9afcccc8d301c2b8922cae815d20c714.tar.gz",
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
