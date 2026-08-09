#!/bin/bash

set -e

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
IMAGE_NAME="openbmc-ast2600"
DOCKERFILE="${SCRIPT_DIR}/Dockerfile"

mkdir -p "${SCRIPT_DIR}/project/deploy"

# Build only the "artifacts" stage (FROM scratch, holds just the final
# deploy/images output) and export it straight to a local directory via
# buildx's local exporter - this skips ever materializing the full builder
# stage (with its multi-hundred-GB Yocto build tree) as a docker image layer.
docker buildx build --target artifacts \
    -f "${DOCKERFILE}" \
    --build-arg "PROJECT=/home/builder/project" \
    --output "type=local,dest=${SCRIPT_DIR}/project/deploy" \
    "${SCRIPT_DIR}"
