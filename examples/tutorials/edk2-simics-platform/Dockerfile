FROM ghcr.io/tianocore/containers/fedora-37-build:a0dd931@sha256:09843c8043412b49c36bd3daaa0660210b3bcd60a8417fe9f9e24f986cd345ff

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN dnf install -y clang llvm lld && \
     dnf clean all

ARG PROJECT=
ARG EDK2_REPO="https://github.com/cglosner/edk2.git"
ARG EDK2_REF="simics-sanitizer"
ARG EDK2_PLATFORMS_REPO_URL="https://github.com/cglosner/edk2-platforms.git"
ARG EDK2_PLATFORMS_BRANCH="simics-sanitizer"
ARG EDK2_NON_OSI_REPO_URL="https://github.com/cglosner/edk2-non-osi.git"
ARG EDK2_NON_OSI_BRANCH="simics-sanitizer"
ARG INTEL_FSP_REPO_URL="https://github.com/cglosner/FSP.git"
ARG INTEL_FSP_BRANCH="simics-sanitizer"

WORKDIR "$PROJECT"

RUN git clone "${EDK2_REPO}" edk2 && \
     git clone "${EDK2_PLATFORMS_REPO_URL}" edk2-platforms && \
     git clone "${EDK2_NON_OSI_REPO_URL}" edk2-non-osi && \
     git clone "${INTEL_FSP_REPO_URL}" FSP

ARG EDK2_HASH="${EDK2_REF}"
ARG EDK2_PLATFORMS_HASH="${EDK2_PLATFORMS_BRANCH}"
ARG EDK2_NON_OSI_HASH="${EDK2_NON_OSI_BRANCH}"
ARG INTEL_FSP_HASH="${INTEL_FSP_BRANCH}"

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
ENV CLANGSAN_BIN="/usr/bin/"

RUN source edk2/edksetup.sh && \
     make -C edk2/BaseTools/

WORKDIR "${PROJECT}/edk2-platforms/Platform/Intel"

# Tutorial-local TSFFS patch for DxeLogoLib is intentionally disabled for now.
# The ASAN platform branches are built as-is.

# Build SimicsOpenBoardPkg
RUN source "${PROJECT}/edk2/edksetup.sh" && \
     python build_bios.py -p BoardX58Ich10 -d -t CLANGSAN
