#!/bin/bash

# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0

# Build one tutorial step:  ./build.sh <1|2|2u|3|4>
#
#   1   no SMM memory protections
#   2   SMM page-table protection (PcdCpuSmmRestrictedMemoryAccess)
#   2u  step 2 + UBSan in the handler
#   3   SMM Heap Guard (PcdHeapGuardPropertyMask) instead of step 2
#   4   steps 2 and 3 together (expected to fail, that is the point)

set -e

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
STEP=${1:?usage: ./build.sh <1|2|2u|3|4>}

PT=FALSE
HG=FALSE
UB=FALSE
case "${STEP}" in
1) ;;
2) PT=TRUE ;;
2u)
    PT=TRUE
    UB=TRUE
    ;;
3) HG=TRUE ;;
4)
    PT=TRUE
    HG=TRUE
    ;;
*)
    echo "unknown step ${STEP}" >&2
    exit 1
    ;;
esac

IMAGE_NAME="tsffs-tutorial-edk2-smm-dvsh-step${STEP}"
CONTAINER_NAME="${IMAGE_NAME}-tmp-$(echo "${RANDOM}" | sha256sum | head -c 8)"

mkdir -p "${SCRIPT_DIR}/project/images"
cp "${SCRIPT_DIR}/../../../harness/tsffs.h" "${SCRIPT_DIR}/DamnVulnerableSmmPkg/DvshHarness/tsffs.h"

docker build -t "${IMAGE_NAME}" -f "${SCRIPT_DIR}/Dockerfile" \
    --build-arg "DVSH_PT_PROTECT=${PT}" \
    --build-arg "DVSH_HEAP_GUARD=${HG}" \
    --build-arg "DVSH_UBSAN=${UB}" \
    "${SCRIPT_DIR}"
docker create --name "${CONTAINER_NAME}" "${IMAGE_NAME}" bash
docker cp "${CONTAINER_NAME}:/build/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_GCC/FV/BOARDX58ICH10.fd" \
    "${SCRIPT_DIR}/project/images/step${STEP}.fd"
docker cp "${CONTAINER_NAME}:/build/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_GCC/X64/DamnVulnerableSmmPkg/DvshSmi/DvshSmi/DEBUG/DvshSmi.debug" \
    "${SCRIPT_DIR}/project/images/step${STEP}.DvshSmi.debug"
docker rm -f "${CONTAINER_NAME}"
