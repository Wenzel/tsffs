FROM ghcr.io/tianocore/containers/fedora-37-build:a0dd931@sha256:09843c8043412b49c36bd3daaa0660210b3bcd60a8417fe9f9e24f986cd345ff

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG PROJECT=

WORKDIR "$PROJECT"

RUN git clone https://github.com/tianocore/edk2.git && \
     git clone https://github.com/tianocore/edk2-platforms.git && \
     git clone https://github.com/tianocore/edk2-non-osi.git && \
     git clone https://github.com/IntelFsp/FSP.git

ARG EDK2_HASH="edk2-stable202602"
ARG EDK2_PLATFORMS_HASH="e758c82cfb4b8f3a283b8f1f2e9f8dcd45a86b71"
ARG EDK2_NON_OSI_HASH="1f4d784"
ARG INTEL_FSP_HASH="8beacd5"

RUN git -C edk2 checkout "${EDK2_HASH}" && \
     git -C edk2 submodule update --init --depth 1 && \
     git -C edk2-platforms checkout "${EDK2_PLATFORMS_HASH}" && \
     git -C edk2-platforms submodule update --init --depth 1 && \
     git -C edk2-non-osi checkout "${EDK2_NON_OSI_HASH}" && \
     git -C edk2-non-osi submodule update --init --depth 1 && \
     git -C FSP checkout "${INTEL_FSP_HASH}" && \
     git -C FSP submodule update --init --depth 1

ENV EDK_TOOLS_PATH="${PROJECT}/edk2/BaseTools/"
ENV PACKAGES_PATH="${PROJECT}/edk2:${PROJECT}/edk2-platforms:${PROJECT}/edk2-non-osi"
ENV WORKSPACE="${PROJECT}"

RUN source edk2/edksetup.sh && \
     make -C edk2/BaseTools/

WORKDIR "${PROJECT}/edk2-platforms/Platform/Intel"

COPY edk2-platforms.patch /tmp/edk2-platforms.patch
COPY tsffs.h "${PROJECT}/edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/Library/DxeLogoLib/tsffs.h"

RUN git -C "${PROJECT}/edk2-platforms" apply --whitespace fix /tmp/edk2-platforms.patch

# Build SimicsOpenBoardPkg
RUN source "${PROJECT}/edk2/edksetup.sh" && \
     python build_bios.py -p BoardX58Ich10X64 -d -t GCC
