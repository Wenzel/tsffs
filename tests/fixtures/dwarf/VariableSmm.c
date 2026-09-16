// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

// Synthetic stand-in for an EDK2 SMM variable-service handler.
//
// This is NOT real EDK2/UEFI source. It exists purely to give WSL gcc
// something to compile with `-g` so the resulting ELF's DWARF debug info can
// be used as an offline test fixture for `DwarfModule::intervals()`
// (see ../../../src/dwarf/mod.rs). The name/shape (`SmmVariableHandler` in a
// file named `VariableSmm.c`) is chosen to mirror the real-world EDK2 GCC5
// convention (per-module `.debug` ELF sidecar, non-zero link-time `.text`
// VMA) documented in the tsffs-simics-bios repo's `debug-uefi-source` skill,
// without requiring a full EDK2/Docker BIOS build.
//
// Regenerate the compiled fixture with (from a WSL shell, e.g.
// `wsl -d Ubuntu-24.04`):
//
//   gcc -g -O0 -ffreestanding -fno-stack-protector -fno-builtin \
//       -c VariableSmm.c -o VariableSmm.o
//   ld -o VariableSmm.debug VariableSmm.o --entry=SmmVariableHandler \
//       --section-start=.text=0x240
//
// Ground truth for the test (recorded here so it stays traceable if the
// toolchain changes and someone regenerates the fixture):
//   - `.text` link-time VMA: 0x240 (asked for explicitly via
//     `--section-start`; confirmed actual with `objdump -h`)
//   - `SmmVariableHandler` link-time address/size: read from `nm`/`objdump`
//     against the checked-in `VariableSmm.debug`, see the doc-comment on the
//     test in `../../../src/dwarf/mod.rs`.

typedef unsigned long UINTN;
typedef unsigned char UINT8;

// Sums the bytes of a variable name. Standing in for whatever real
// validation an EDK2 SMM variable handler would do to a caller-supplied
// name before touching NV storage.
static UINTN ValidateVariableName(const UINT8 *Name, UINTN Length) {
  UINTN Checksum = 0;

  for (UINTN Index = 0; Index < Length; Index++) {
    Checksum += Name[Index];
  }

  return Checksum;
}

// Synthetic analog of a real BIOS SMM handler, e.g. the SMI dispatch
// callback registered by EDK2's VariableSmm driver.
UINTN SmmVariableHandler(const UINT8 *Name, UINTN Length, UINT8 *Data) {
  UINTN Checksum = ValidateVariableName(Name, Length);

  if (Checksum == 0) {
    return 0;
  }

  Data[0] = (UINT8)(Checksum & 0xFF);

  return Checksum;
}
