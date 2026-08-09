#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Builds the TSFFS harness for libpldm's decode_pldm_firmware_update_package
# as a bare-metal ARM32 ELF, for direct load-binary onto the target AST2600
# Simics model's arm-cortex-a7 core (see ../scripts/fuzz.simics).
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
LIBPLDM_SRC="${SCRIPT_DIR}/libpldm-src"

# Use the repo's own ARM32 harness header instead of vendoring a copy.
cp "${SCRIPT_DIR}/../../../../../harness/tsffs-gcc-arm32.h" "${SCRIPT_DIR}/src/tsffs.h"

if [ ! -d "${LIBPLDM_SRC}" ]; then
    git clone --depth 1 https://github.com/openbmc/libpldm.git "${LIBPLDM_SRC}"
fi

# libpldm normally generates these two headers from .in templates via Meson,
# using DISTRO_FEATURES/build options that select the "production" ABI
# (deprecated + stable symbols, no testing/unstable ones) - the same default
# the openbmc/openbmc libpldm_git.bb recipe uses. Generate the equivalent by
# hand since we're building outside Meson.
sed \
    -e 's/#mesondefine HAVE_LIBPLDM_API_DEPRECATED/#define HAVE_LIBPLDM_API_DEPRECATED 1/' \
    -e 's/#mesondefine HAVE_LIBPLDM_API_TESTING/\/* #undef HAVE_LIBPLDM_API_TESTING *\//' \
    "${LIBPLDM_SRC}/include/libpldm/api.h.in" > "${LIBPLDM_SRC}/include/libpldm/api.h"

sed \
    -e 's/#mesondefine HAVE_LIBPLDM_ABI_DEPRECATED/#define HAVE_LIBPLDM_ABI_DEPRECATED 1/' \
    -e 's/#mesondefine HAVE_LIBPLDM_ABI_TESTING/#define HAVE_LIBPLDM_ABI_TESTING 0/' \
    -e 's/#mesondefine HAVE_STRUCT_MCTP_FQ_ADDR/#define HAVE_STRUCT_MCTP_FQ_ADDR 0/' \
    "${LIBPLDM_SRC}/config.h.in" > "${LIBPLDM_SRC}/config.h"

CC=arm-linux-gnueabihf-gcc
LD=arm-linux-gnueabihf-ld
CFLAGS="-mcpu=cortex-a7 -marm -mfpu=vfpv3 -mno-unaligned-access -fno-tree-vectorize -fno-tree-slp-vectorize -ffreestanding -fno-builtin -fno-pic -O0 -g \
    -I ${LIBPLDM_SRC}/include -I ${LIBPLDM_SRC}/src \
    -include ${LIBPLDM_SRC}/config.h -I ${SCRIPT_DIR}/src"

OBJ_DIR="${SCRIPT_DIR}/build"
mkdir -p "${OBJ_DIR}"

"${CC}" -c ${CFLAGS} -o "${OBJ_DIR}/startup.o" "${SCRIPT_DIR}/src/startup.S"
"${CC}" -c ${CFLAGS} -o "${OBJ_DIR}/harness_main.o" "${SCRIPT_DIR}/src/harness_main.c"
"${CC}" -c ${CFLAGS} -o "${OBJ_DIR}/firmware_update.o" "${LIBPLDM_SRC}/src/dsp/firmware_update.c"
"${CC}" -c ${CFLAGS} -o "${OBJ_DIR}/base.o" "${LIBPLDM_SRC}/src/dsp/base.c"
"${CC}" -c ${CFLAGS} -o "${OBJ_DIR}/edac.o" "${LIBPLDM_SRC}/src/edac.c"
"${CC}" -c ${CFLAGS} -o "${OBJ_DIR}/utils.o" "${LIBPLDM_SRC}/src/utils.c"

"${LD}" -T "${SCRIPT_DIR}/src/link.ld" -o "${SCRIPT_DIR}/pldm_fw_update_harness.elf" \
    "${OBJ_DIR}/startup.o" "${OBJ_DIR}/harness_main.o" "${OBJ_DIR}/firmware_update.o" \
    "${OBJ_DIR}/base.o" "${OBJ_DIR}/edac.o" "${OBJ_DIR}/utils.o"

echo "Built pldm_fw_update_harness.elf"
