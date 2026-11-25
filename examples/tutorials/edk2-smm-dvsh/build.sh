#!/bin/bash

# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0

# Build one tutorial step:  ./build.sh <1|2|2u|3|4|5off|5poff|5|5p>
#
#   1   no SMM memory protections
#   2   SMM page-table protection (PcdCpuSmmRestrictedMemoryAccess)
#   2u  step 2 + UBSan in the handler
#   3   SMM Heap Guard (PcdHeapGuardPropertyMask) instead of step 2
#   4   steps 2 and 3 together (expected to fail, that is the point)
#
# Steps 5off, 5poff, 5 and 5p build the same board from FuzzUEr's forks (clang 15):
#   5off   DvshSmi not instrumented, no restricted SMM memory access (the control)
#   5poff  DvshSmi not instrumented, restricted SMM memory access
#   5      AddressSanitizer + UBSan on DvshSmi
#   5p     AddressSanitizer + UBSan on DvshSmi, restricted SMM memory access

set -e

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
STEP=${1:?usage: ./build.sh <1|2|2u|3|4|5off|5poff|5|5p>}

PT=FALSE
HG=FALSE
UB=FALSE
ASAN=FALSE
DOCKERFILE="${SCRIPT_DIR}/Dockerfile"
BUILD_ROOT=/build/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_GCC
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
5off | 5poff | 5 | 5p)
    DOCKERFILE="${SCRIPT_DIR}/fuzzuer/Dockerfile"
    BUILD_ROOT=/build/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_CLANGSAN
    case "${STEP}" in
    5poff) PT=TRUE ;;
    5) ASAN=TRUE ;;
    5p)
        ASAN=TRUE
        PT=TRUE
        ;;
    esac
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

if [ "${DOCKERFILE}" = "${SCRIPT_DIR}/fuzzuer/Dockerfile" ]; then
    SANARGS=(--build-arg "DVSH_PT_PROTECT=${PT}" --build-arg "DVSH_ASAN=${ASAN}")
else
    SANARGS=(--build-arg "DVSH_PT_PROTECT=${PT}" --build-arg "DVSH_HEAP_GUARD=${HG}" --build-arg "DVSH_UBSAN=${UB}")
fi

docker build -t "${IMAGE_NAME}" -f "${DOCKERFILE}" \
    "${SANARGS[@]}" \
    "${SCRIPT_DIR}"
docker create --name "${CONTAINER_NAME}" "${IMAGE_NAME}" bash
docker cp "${CONTAINER_NAME}:${BUILD_ROOT}/FV/BOARDX58ICH10.fd" \
    "${SCRIPT_DIR}/project/images/step${STEP}.fd"
docker cp "${CONTAINER_NAME}:${BUILD_ROOT}/X64/DamnVulnerableSmmPkg/DvshSmi/DvshSmi/DEBUG/DvshSmi.debug" \
    "${SCRIPT_DIR}/project/images/step${STEP}.DvshSmi.debug"
docker rm -f "${CONTAINER_NAME}"
