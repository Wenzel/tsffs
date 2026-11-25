FROM ghcr.io/tianocore/containers/fedora-37-build:a0dd931

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

WORKDIR /build

RUN git clone --quiet https://github.com/tianocore/edk2.git edk2 && \
    git clone --quiet https://github.com/tianocore/edk2-platforms.git edk2-platforms && \
    git clone --quiet https://github.com/tianocore/edk2-non-osi.git edk2-non-osi && \
    git clone --quiet https://github.com/IntelFsp/FSP.git FSP

ARG EDK2_HASH="eccdab6"
ARG EDK2_PLATFORMS_HASH="f446fff"
ARG EDK2_NON_OSI_HASH="1f4d784"
ARG INTEL_FSP_HASH="8beacd5"

# UnitTestFrameworkPkg's subhook submodule is a dead upstream repo; it is only
# needed for edk2's own unit tests, so skip it.
RUN git -C edk2 checkout "${EDK2_HASH}" && \
    git -C edk2 submodule deinit -f UnitTestFrameworkPkg/Library/SubhookLib/subhook && \
    git -C edk2 config --file .gitmodules --get-regexp path | awk '{print $2}' | grep -v '^UnitTestFrameworkPkg/Library/SubhookLib/subhook$' > /tmp/submodule-paths.txt && \
    cd edk2 && git submodule update --init -- $(cat /tmp/submodule-paths.txt) && cd .. && \
    git -C edk2-platforms checkout "${EDK2_PLATFORMS_HASH}" && \
    git -C edk2-platforms submodule update --init && \
    git -C edk2-non-osi checkout "${EDK2_NON_OSI_HASH}" && \
    git -C edk2-non-osi submodule update --init && \
    git -C FSP checkout "${INTEL_FSP_HASH}" && \
    git -C FSP submodule update --init

ENV EDK_TOOLS_PATH="/build/edk2/BaseTools/"
ENV PACKAGES_PATH="/build/edk2:/build/edk2-platforms:/build/edk2-non-osi"
ENV WORKSPACE="/build"

RUN source edk2/edksetup.sh && make -C edk2/BaseTools/

# DVSH, its TSFFS harness, and the board changes (SMRR PCD + DSC/FDF wiring).
# tsffs.h is copied next to the harness by build.sh.
COPY DamnVulnerableSmmPkg /build/edk2-platforms/Platform/Intel/DamnVulnerableSmmPkg
COPY config/ /build/edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/BoardX58Ich10/
COPY qsp-dvsh.patch /tmp/qsp-dvsh.patch
RUN git -C edk2-platforms apply --ignore-whitespace /tmp/qsp-dvsh.patch

# Report every EDK2 ASSERT/breakpoint to TSFFS (HARNESS_ASSERT in CpuDeadLoop and
# CpuBreakpoint), so a failed ASSERT is an assert solution and not a timeout.
COPY DamnVulnerableSmmPkg/DvshHarness/tsffs.h /build/edk2/MdePkg/Include/Library/Tsffs.h
COPY edk2-harness-assert.patch /tmp/edk2-harness-assert.patch
RUN git -C edk2 apply /tmp/edk2-harness-assert.patch

# Step toggles, read by DvshDefines.dsc.inc. build.sh sets them per step.
ARG DVSH_PT_PROTECT=FALSE
ARG DVSH_HEAP_GUARD=FALSE
ARG DVSH_UBSAN=FALSE
RUN printf 'DEFINE DVSH_PT_PROTECT = %s\nDEFINE DVSH_HEAP_GUARD = %s\nDEFINE DVSH_UBSAN = %s\n' \
      "${DVSH_PT_PROTECT}" "${DVSH_HEAP_GUARD}" "${DVSH_UBSAN}" \
      > edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/BoardX58Ich10/DvshStep.dsc.inc

WORKDIR /build/edk2-platforms/Platform/Intel

RUN source /build/edk2/edksetup.sh && \
    python build_bios.py -p BoardX58Ich10X64 -d -t GCC
