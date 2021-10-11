"""Transitive third party dependencies of `cargo-bazel`"""

load("@rules_foreign_cc//foreign_cc:repositories.bzl", "rules_foreign_cc_dependencies")

def third_party_transitive_deps():
    # This sets up some common toolchains for building targets. For more details, please see
    # https://bazelbuild.github.io/rules_foreign_cc/0.6.0/flatten.html#rules_foreign_cc_dependencies
    rules_foreign_cc_dependencies()
