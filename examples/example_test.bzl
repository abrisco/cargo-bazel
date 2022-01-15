"""Cargo-bazel example test helpers"""

load("//tools/bazel_integration:bazel_integration.bzl", "bazel_integration_test")

def example_test(name):
    """Defines Bazel integration tests for cargo-bazel example workspaces.

    Args:
        name (str): The name of the test. This is expected to match a directory containing a Bazel workspace
    """
    cargo_bazel_bin = Label("//tools/cargo_bazel:bin")

    bazel_integration_test(
        name = name,
        workspace_srcs = native.glob(
            include = ["{}/**".format(name)],
            exclude = ["{}/bazel-*/**".format(name)],
        ),
        repository_archives = {Label("//distro"): "cargo_bazel"},
        env = {"CARGO_BAZEL_GENERATOR_URL": "file://${{pwd}}/$(rootpath {})".format(cargo_bazel_bin)},
        data = [cargo_bazel_bin],
        bazel_bin = "@bazel_integration",
        flaky = True,
        tags = ["requires-network"],
        size = "large",
    )
