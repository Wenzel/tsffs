// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

#include <Library/BmpSupportLib.h>
#include <Library/MemoryAllocationLib.h>
#include <Library/UefiApplicationEntryPoint.h>
#include <Library/UefiLib.h>
#include <Uefi.h>

#include "tsffs.h"

void hexdump(UINT8 *buf, UINTN size) {
  for (UINTN i = 0; i < size; i++) {
    if (i != 0 && i % 26 == 0) {
      Print(L"\n");
    } else if (i != 0 && i % 2 == 0) {
      Print(L" ");
    }
    Print(L"%02x", buf[i]);
  }
  Print(L"\n");
}

EFI_STATUS
EFIAPI
UefiMain(IN EFI_HANDLE ImageHandle, IN EFI_SYSTEM_TABLE *SystemTable) {
  EFI_GRAPHICS_OUTPUT_BLT_PIXEL *GopBlt = NULL;
  UINTN GopBltSize = 0;
  UINTN Height = 0;
  UINTN Width = 0;
  RETURN_STATUS Status;
  UINTN MaxInputSize = 0x4000;
  UINTN InputSize = MaxInputSize;
  UINT8 *Input = AllocatePool(MaxInputSize);

  if (!Input) {
    return EFI_OUT_OF_RESOURCES;
  }

  HARNESS_START(Input, &InputSize);

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
  Print(L"Input: %p Size: %d\n", Input, InputSize);
#endif
  // This parser takes attacker-controlled bytes and allocates its output buffer
  // itself, which makes it a good small target for ASAN validation work.
  Status = TranslateBmpToGopBlt(Input, InputSize, &GopBlt, &GopBltSize, &Height, &Width);

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
  Print(L"TranslateBmpToGopBlt() -> %r, %ux%u, blt=%p size=%u\n", Status, Width, Height, GopBlt, GopBltSize);
  if (InputSize <= 0x100) {
    hexdump(Input, InputSize);
  }
#endif

  if (GopBlt != NULL) {
    FreePool(GopBlt);
  }

  FreePool(Input);
  HARNESS_STOP();

  return EFI_SUCCESS;
}
