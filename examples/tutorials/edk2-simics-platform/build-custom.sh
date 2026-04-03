#!/bin/bash

# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0

set -e

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
IMAGE_NAME="edk2-simics-asan"
DOCKERFILE="${SCRIPT_DIR}/Dockerfile-custom"
CONTAINER_UID=$(echo "${RANDOM}" | sha256sum | head -c 8)
CONTAINER_NAME="${IMAGE_NAME}-tmp-${CONTAINER_UID}"
EDK2_REPO_URL="https://github.com/cglosner/edk2.git"
EDK2_BRANCH="simics-sanitizer"
EDK2_PLATFORMS_REPO_URL="https://github.com/cglosner/edk2-platforms.git"
EDK2_PLATFORMS_BRANCH="simics-sanitizer"
EDK2_NON_OSI_HASH="1f4d784"
INTEL_FSP_HASH="8beacd5"

if [ ! -d "${SCRIPT_DIR}/workspace" ]; then
    mkdir -p "${SCRIPT_DIR}/workspace"
    git clone --branch "${EDK2_BRANCH}" --single-branch "${EDK2_REPO_URL}" "${SCRIPT_DIR}/workspace/edk2"
    git -C "${SCRIPT_DIR}/workspace/edk2" submodule update --init --depth 1
    git clone --branch "${EDK2_PLATFORMS_BRANCH}" --single-branch "${EDK2_PLATFORMS_REPO_URL}" "${SCRIPT_DIR}/workspace/edk2-platforms"
    git -C "${SCRIPT_DIR}/workspace/edk2-platforms" submodule update --init --depth 1
    git clone https://github.com/tianocore/edk2-non-osi.git "${SCRIPT_DIR}/workspace/edk2-non-osi"
    git -C "${SCRIPT_DIR}/workspace/edk2-non-osi" checkout "${EDK2_NON_OSI_HASH}"
    git -C "${SCRIPT_DIR}/workspace/edk2-non-osi" submodule update --init --depth 1
    git clone https://github.com/IntelFsp/FSP.git "${SCRIPT_DIR}/workspace/FSP"
    git -C "${SCRIPT_DIR}/workspace/FSP" checkout "${INTEL_FSP_HASH}"
    git -C "${SCRIPT_DIR}/workspace/FSP" submodule update --init --depth 1
fi

# The TSFFS logo harness is tutorial-local, so keep injecting its header into the
# cloned public tree before building the image in Docker.
cp "${SCRIPT_DIR}/../../../harness/tsffs.h" "${SCRIPT_DIR}/tsffs.h"

docker build -t "${IMAGE_NAME}" -f "${DOCKERFILE}" "${SCRIPT_DIR}"
docker create --name "${CONTAINER_NAME}" "${IMAGE_NAME}" bash
rm -rf "${SCRIPT_DIR}/BoardX58Ich10_CUSTOM"
docker cp "${CONTAINER_NAME}:/workspace/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_CLANGSAN/FV/" "${SCRIPT_DIR}/BoardX58Ich10_CUSTOM"
docker rm -f "${CONTAINER_NAME}"
mkdir -p "${SCRIPT_DIR}/project/targets/qsp-x86/images/"
cp "${SCRIPT_DIR}/BoardX58Ich10_CUSTOM/BOARDX58ICH10.fd" "${SCRIPT_DIR}/project/targets/qsp-x86/images/BOARDX58ICH10_CUSTOM.fd"
cp "${SCRIPT_DIR}/../../rsrc/minimal_boot_disk.craff" "${SCRIPT_DIR}/project/minimal_boot_disk.craff"
