"""Transitive dependencies required by the `cargo-bazel`"""

load("//3rdparty:third_party_transitive_deps.bzl", "third_party_transitive_deps")

def cargo_bazel_deps_transitive():
    third_party_transitive_deps()
