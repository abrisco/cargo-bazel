"""Bazel integration testing"""

def _get_integration_workspace_root(ctx):
    srcs = ctx.files.workspace_srcs
    for file in srcs:
        if file.basename in ("WORKSPACE", "WORKSPACE.bazel"):
            return file
    fail("Unable to locate a `WORKSPACE` or `WORKSPACE.bazel` file in `workspace_srcs` attribute")

def _generate_repo_archive_arg(repo_name, target):
    files = target[DefaultInfo].files.to_list()
    if len(files) != 1:
        fail("The target {} has an unexpected number of files".format(
            target.label,
        ))

    archive = files[0]

    return "--repo_archive={}={}".format(
        repo_name,
        archive.short_path,
    )

def _bazel_integration_test_impl(ctx):
    is_windows = ctx.executable._runner.basename.endswith(".exe")

    args_file = ctx.actions.declare_file(ctx.label.name + ".args.txt")

    data = ctx.attr.data
    env = {key: ctx.expand_location(val, data) for key, val in ctx.attr.env.items()}

    ctx.actions.write(
        output = args_file,
        content = "\n".join(
            [
                "integration",
                "--bazel_bin",
                ctx.executable.bazel_bin.short_path,
                "--workspace",
                _get_integration_workspace_root(ctx).short_path,
            ] +
            ["--env={}".format(var) for var in env.keys()] +
            [_generate_repo_archive_arg(repo_name, target) for target, repo_name in ctx.attr.repository_archives.items()],
        ),
    )

    runner = ctx.actions.declare_file(ctx.label.name + ".runner" + (".exe" if is_windows else ""))

    ctx.actions.symlink(
        output = runner,
        target_file = ctx.executable._runner,
        is_executable = True,
    )

    return [
        DefaultInfo(
            executable = runner,
            files = depset([runner, args_file]),
            runfiles = ctx.runfiles(files = [
                ctx.executable._runner,
                args_file,
                ctx.executable.bazel_bin,
            ] + ctx.files.data + ctx.files.workspace_srcs + ctx.files.repository_archives).merge(ctx.attr._runner[DefaultInfo].default_runfiles),
        ),
        testing.TestEnvironment(dict({
            "CARGO_BAZEL_INTEGRATION_TEST_ARGS_FILE": args_file.short_path,
        }.items() + env.items())),
    ]

bazel_integration_test = rule(
    implementation = _bazel_integration_test_impl,
    doc = "",
    attrs = {
        "bazel_bin": attr.label(
            doc = "",
            cfg = "exec",
            executable = True,
            mandatory = True,
            allow_files = True,
        ),
        "data": attr.label_list(
            doc = "",
            allow_files = True,
        ),
        "env": attr.string_dict(
            doc = "",
        ),
        "repository_archives": attr.label_keyed_string_dict(
            doc = "A mapping of archives to repository names for use during the test",
            allow_files = True,
        ),
        "workspace_srcs": attr.label_list(
            doc = "A filegroup of the workspace to test",
            allow_files = True,
            mandatory = True,
        ),
        "_runner": attr.label(
            doc = "",
            cfg = "exec",
            executable = True,
            default = Label("//tools/bazel_integration:integration_test_runner"),
        ),
    },
    test = True,
)

def _get_bazel_version(repository_ctx):
    if repository_ctx.attr.bazel_version_file:
        # Read the file
        bazel_version_file = repository_ctx.path(repository_ctx.attr.bazel_version_file)
        content = repository_ctx.read(bazel_version_file)

        # Strip comments
        bazel_version = "".join([line for line in content.splitlines() if "#" not in line])

        return bazel_version.strip(" ")

    return repository_ctx.attr.bazel_version

def _bazel_binary_repository_impl(repository_ctx):
    bazel_version = _get_bazel_version(repository_ctx)

    url = repository_ctx.attr.url_template
    if "win" in repository_ctx.os.name:
        system = "windows-x86_64"
        ext = ".exe"
    elif "mac" in repository_ctx.os.name:
        system = "darwin-x86_64"
        ext = ""
    else:
        system = "linux-x86_64"
        ext = ""

    url = repository_ctx.attr.url_template
    url = url.replace("{version}", bazel_version)
    url = url.replace("{system}", system)
    url = url.replace("{extension}", ext)

    path = "bazel{}".format(ext)
    bazel_path = repository_ctx.path(path)

    result = repository_ctx.download(
        url = url,
        output = bazel_path,
        sha256 = repository_ctx.attr.sha256,
        executable = True,
    )

    if not bazel_path.exists:
        fail("Failed to download Bazel from '{}'".format(url))

    repository_ctx.file("BUILD.bazel", "\n".join([
        """package(default_visibility = ["//visibility:public"])""",
        """exports_files(glob(["**"]))""",
        """alias(name = "{}", actual = ":{}")""".format(
            repository_ctx.name,
            path,
        ),
        "",
    ]))

    repository_ctx.file("WORKSPACE.bazel", """workspace(name = "{}")""".format(
        repository_ctx.name,
    ))

    return {
        "bazel_version": repository_ctx.attr.bazel_version,
        "bazel_version_file": repository_ctx.attr.bazel_version_file,
        "name": repository_ctx.name,
        "sha256": result.sha256,
        "url_template": repository_ctx.attr.url_template,
    }

bazel_binary_repository = repository_rule(
    implementation = _bazel_binary_repository_impl,
    doc = "",
    attrs = {
        "bazel_version": attr.string(
            doc = "",
            default = "4.0.0",
        ),
        "bazel_version_file": attr.label(
            allow_files = True,
            doc = "",
        ),
        "sha256": attr.string(
            doc = "",
        ),
        "url_template": attr.string(
            doc = "",
            default = "https://github.com/bazelbuild/bazel/releases/download/{version}/bazel-{version}-{system}{extension}",
        ),
    },
)

def write_deleted_packages(name):
    pass
