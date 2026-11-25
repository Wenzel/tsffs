// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

/** @file
  TSFFS harness for the Damn Vulnerable SMI Handler.

  At ReadyToBoot (after SmmReadyToLock, so every SMM protection is active) the
  fuzzer's testcase is written into the data area of an SMM communication
  buffer addressed to gDvshSmiHandlerGuid, and the SMI is triggered through
  EFI_SMM_COMMUNICATION_PROTOCOL.
**/

#include <Uefi.h>
#include <Library/BaseMemoryLib.h>
#include <Library/UefiBootServicesTableLib.h>
#include <Library/UefiLib.h>
#include <Guid/EventGroup.h>
#include <Guid/PiSmmCommunicationRegionTable.h>
#include <Protocol/SmmCommunication.h>
#include <Guid/DvshSmiCommunication.h>

#include "tsffs.h"

#define COMM_BUFFER_SIZE  0x1000

// The PI SMM communication region is the only non-SMRAM buffer SMM accepts
// once restricted memory access is on.
STATIC
EFI_SMM_COMMUNICATE_HEADER *
FindSmmCommRegion (
  VOID
  )
{
  EDKII_PI_SMM_COMMUNICATION_REGION_TABLE  *Table;
  EFI_MEMORY_DESCRIPTOR                    *Region;
  UINT32                                   Index;

  Table = NULL;
  if (EFI_ERROR (EfiGetSystemConfigurationTable (&gEdkiiPiSmmCommunicationRegionTableGuid, (VOID **)&Table)) || (Table == NULL)) {
    return NULL;
  }

  Region = (EFI_MEMORY_DESCRIPTOR *)(Table + 1);
  for (Index = 0; Index < Table->NumberOfEntries; Index++) {
    if ((Region->Type == EfiConventionalMemory) && (EFI_PAGES_TO_SIZE (Region->NumberOfPages) >= COMM_BUFFER_SIZE)) {
      return (EFI_SMM_COMMUNICATE_HEADER *)(UINTN)Region->PhysicalStart;
    }

    Region = (EFI_MEMORY_DESCRIPTOR *)((UINT8 *)Region + Table->DescriptorSize);
  }

  return NULL;
}

STATIC
VOID
EFIAPI
OnReadyToBoot (
  IN EFI_EVENT  Event,
  IN VOID       *Context
  )
{
  EFI_SMM_COMMUNICATION_PROTOCOL  *SmmCommunication;
  EFI_SMM_COMMUNICATE_HEADER      *CommBuffer;
  UINTN                           InputSize;
  UINTN                           CommBufferSize;
  UINTN                           PayloadMaxSize;

  gBS->CloseEvent (Event);

  if (EFI_ERROR (gBS->LocateProtocol (&gEfiSmmCommunicationProtocolGuid, NULL, (VOID **)&SmmCommunication))) {
    Print (L"DVSH harness: no SmmCommunication protocol\n");
    return;
  }

  CommBuffer = FindSmmCommRegion ();
  if (CommBuffer == NULL) {
    Print (L"DVSH harness: no SMM communication region\n");
    return;
  }

  Print (L"DVSH harness: CommBuffer at 0x%p\n", CommBuffer);
  CopyGuid (&CommBuffer->HeaderGuid, &gDvshSmiHandlerGuid);

  PayloadMaxSize = COMM_BUFFER_SIZE - OFFSET_OF (EFI_SMM_COMMUNICATE_HEADER, Data);
  InputSize      = PayloadMaxSize;

  while (TRUE) {
    HARNESS_START_WITH_MAXIMUM_SIZE_AND_PTR_INDEX (0, CommBuffer->Data, &InputSize, PayloadMaxSize);

    // The handler rejects anything shorter than one request; skip it cheaply.
    if (InputSize < sizeof (DVSH_SMI_REQUEST)) {
      HARNESS_STOP ();
      continue;
    }

    CommBuffer->MessageLength = InputSize;
    CommBufferSize            = OFFSET_OF (EFI_SMM_COMMUNICATE_HEADER, Data) + InputSize;
    SmmCommunication->Communicate (SmmCommunication, CommBuffer, &CommBufferSize);

    HARNESS_STOP ();
  }
}

EFI_STATUS
EFIAPI
DvshHarnessEntryPoint (
  IN EFI_HANDLE        ImageHandle,
  IN EFI_SYSTEM_TABLE  *SystemTable
  )
{
  EFI_EVENT  Event;

  return gBS->CreateEventEx (
                EVT_NOTIFY_SIGNAL,
                TPL_CALLBACK,
                OnReadyToBoot,
                NULL,
                &gEfiEventReadyToBootGuid,
                &Event
                );
}
