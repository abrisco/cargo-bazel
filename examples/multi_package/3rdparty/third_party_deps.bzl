"""A helper module for loading 3rd party dependencies
The sources here originate from: https://github.com/bazelbuild/rules_foreign_cc/tree/0.6.0/examples/third_party/openssl
"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "maybe")

def third_party_deps(prefix = ""):
    """Definitions for 3rd party dependencies

    Args:
        prefix (str, optional): An optional prefix for all dependencies
    """
    pass
