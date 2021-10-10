"""Dependencies required for generating documentation"""

load("@io_bazel_stardoc//:setup.bzl", "stardoc_repositories")

def docs_deps_transitive():
    stardoc_repositories()
