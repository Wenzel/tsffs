// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

#include <Library/BmpSupportLib.h>
#include <Library/DebugLib.h>
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

static VOID RunCorruptionTest(UINT8 Selector, UINT8 *Input) {
  UINT8 TestId = Selector % 5;

  switch (TestId) {
  case 0: {
    UINT8 *Buf = AllocatePool(8);
    Print(L"[Tutorial] Test 0: heap overflow\n");
    if (Buf != NULL) {
      Buf[8] = Input != NULL ? Input[0] : 0;
      FreePool(Buf);
    }
    break;
  }
  case 1: {
    UINT8 *Buf = AllocatePool(8);
    Print(L"[Tutorial] Test 1: heap use-after-free\n");
    if (Buf != NULL) {
      FreePool(Buf);
      Buf[0] = Input != NULL ? Input[0] : 0;
    }
    break;
  }
  case 2: {
    volatile UINT8 StackBuf[8];
    Print(L"[Tutorial] Test 2: stack overflow\n");
    StackBuf[8] = Input != NULL ? Input[0] : 0;
    break;
  }
  case 3: {
    UINT8 *Buf = AllocatePool(8);
    Print(L"[Tutorial] Test 3: heap underflow\n");
    if (Buf != NULL) {
      Buf[-1] = Input != NULL ? Input[0] : 0;
      FreePool(Buf);
    }
    break;
  }
  case 4: {
    Print(L"[Tutorial] Test 4: pointer overflow\n");
    {
      volatile UINT8 *OverflowPtr = Input + ~(UINTN)0;
      if (OverflowPtr == Input) {
        Print(L"[Tutorial] unreachable\n");
      }
    }
    break;
  }
  default:
    break;
  }
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

  DEBUG ((DEBUG_ERROR, "TSFFS Tutorial DEBUG marker before harness start\n"));
  HARNESS_START(Input, &InputSize);

  Print(L"[Tutorial] corruption selector = %u\n", InputSize > 0 ? Input[0] : 0);
  RunCorruptionTest(InputSize > 0 ? Input[0] : 0, Input);

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
