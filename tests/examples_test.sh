#!/bin/bash

set -euo pipefail

if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
    EXAMPLES_DIR="${BUILD_WORKSPACE_DIRECTORY}/examples"
else
    BUILD_WORKSPACE_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")/../" &>/dev/null && pwd)"
    EXAMPLES_DIR="${BUILD_WORKSPACE_DIRECTORY}/examples"
fi

export CARGO_BAZEL_GENERATOR_URL="file://$(pwd)/${CARGO_BAZEL_BIN}"
export CARGO_BAZEL_GENERATOR_SHA256="$(shasum -a 256 "$(pwd)/${CARGO_BAZEL_BIN}" | awk '{ print $1 }')"

if [[ -z "${BAZEL_STARTUP_FLAGS:-}" ]]; then
    export BAZEL_STARTUP_FLAGS=("")
fi

if [[ -z "${EXAMPLES_BAZEL_STARTUP_FLAGS:-}" ]]; then
    export EXAMPLES_BAZEL_STARTUP_FLAGS=("")
fi

set -x
pushd "${EXAMPLES_DIR}" &>/dev/null

bazel ${BAZEL_STARTUP_FLAGS[@]} ${EXAMPLES_BAZEL_STARTUP_FLAGS[@]} build //...
bazel ${BAZEL_STARTUP_FLAGS[@]} ${EXAMPLES_BAZEL_STARTUP_FLAGS[@]} test //...

export CARGO_BAZEL_REPIN=1
bazel ${BAZEL_STARTUP_FLAGS[@]} ${EXAMPLES_BAZEL_STARTUP_FLAGS[@]} build //...
bazel ${BAZEL_STARTUP_FLAGS[@]} ${EXAMPLES_BAZEL_STARTUP_FLAGS[@]} test //...

popd &>/dev/null
