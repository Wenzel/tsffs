# Steps 5off, 5poff, 5 and 5p: the same board built from FuzzUEr's forks of edk2 and
# edk2-platforms with the CLANGSAN toolchain (clang 15). Build context: the tutorial
# directory (build.sh passes -f fuzzuer/Dockerfile .).
FROM ghcr.io/tianocore/containers/fedora-37-build:a0dd931

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN dnf install -y clang lld llvm && dnf clean all

WORKDIR /build

# Pinned commits of third-party branches (cglosner/*, branch simics-working for the two
# that matter), fetched by SHA so the build does not move under you.
ARG EDK2_SHA=fa877c0eb8b2b0b31e4e2de8f488bc227896feec
ARG EDK2_PLATFORMS_SHA=6137f5a87f4e43cee1b3dcfa15397dbf94ae4df7
ARG EDK2_NON_OSI_SHA=61b65fccfe4c75bc9ecb7b542412a436e3db5de6
ARG FSP_SHA=98e497f733450b97f590cab0556f05d23cdf3e02

RUN fetch() { git init -q "$1" && git -C "$1" remote add origin "https://github.com/cglosner/$1.git" \
      && git -C "$1" fetch -q --depth 1 origin "$2" && git -C "$1" checkout -q FETCH_HEAD; } \
    && fetch edk2 "${EDK2_SHA}" \
    && fetch edk2-platforms "${EDK2_PLATFORMS_SHA}" \
    && fetch edk2-non-osi "${EDK2_NON_OSI_SHA}" \
    && fetch FSP "${FSP_SHA}" \
    && git -C edk2 submodule update --init --depth 1 -q \
    && git -C edk2-platforms submodule update --init --depth 1 -q

ENV EDK_TOOLS_PATH="/build/edk2/BaseTools/" \
    PACKAGES_PATH="/build/edk2:/build/edk2-platforms:/build/edk2-non-osi" \
    WORKSPACE="/build" \
    CLANGSAN_BIN="/usr/bin/"

RUN source edk2/edksetup.sh && make -C edk2/BaseTools/

# TSFFS: HARNESS_ASSERT() in CpuDeadLoop/CpuBreakpoint, so any EDK2 ASSERT is a TSFFS
# assert solution. tsffs.h is copied next to the harness by build.sh.
COPY DamnVulnerableSmmPkg/DvshHarness/tsffs.h /build/edk2/MdePkg/Include/Library/Tsffs.h
COPY edk2-harness-assert.patch /tmp/edk2-harness-assert.patch
# AsanLib: keep its shared state (fuzzing window, untrusted regions) in reserved memory
# when the platform provides it, so instrumented SMM code can read it under restricted
# SMM memory access.
COPY fuzzuer/edk2-asan-shared-state.patch /tmp/edk2-asan-shared-state.patch
RUN git -C edk2 apply /tmp/edk2-harness-assert.patch \
    && git -C edk2 apply --ignore-whitespace /tmp/edk2-asan-shared-state.patch

# Board: DVSH wiring, plus the ASan shadow as its own reserved range, PEI memory after it,
# and the shared ASan state page (SimicsPei/MemDetect.c).
COPY DamnVulnerableSmmPkg /build/edk2-platforms/Platform/Intel/DamnVulnerableSmmPkg
COPY fuzzuer/config/ /build/edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/BoardX58Ich10/
COPY fuzzuer/edk2-platforms.patch /tmp/edk2-platforms.patch
RUN git -C edk2-platforms apply --ignore-whitespace /tmp/edk2-platforms.patch

# Step toggles, read by DvshDefines.dsc.inc.
ARG DVSH_PT_PROTECT=FALSE
ARG DVSH_ASAN=FALSE
RUN printf 'DEFINE DVSH_PT_PROTECT = %s\nDEFINE DVSH_ASAN = %s\n' "${DVSH_PT_PROTECT}" "${DVSH_ASAN}" \
      > edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/BoardX58Ich10/DvshStep.dsc.inc

WORKDIR /build/edk2-platforms/Platform/Intel

RUN source /build/edk2/edksetup.sh && python build_bios.py -p BoardX58Ich10 -d -t CLANGSAN
