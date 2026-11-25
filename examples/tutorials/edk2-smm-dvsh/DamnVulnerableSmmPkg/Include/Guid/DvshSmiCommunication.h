/** @file
  Wire protocol for the Damn Vulnerable SMI Handler (DVSH).

  Shared between the SMM driver (DamnVulnerableSmmPkg/DvshSmi) and any
  caller that uses EFI_SMM_COMMUNICATION_PROTOCOL to reach the handler, such
  as the TSFFS harness in DamnVulnerableSmmPkg/DvshHarness.

  SPDX-License-Identifier: BSD-2-Clause-Patent
**/

#ifndef DVSH_SMI_COMMUNICATION_H_
#define DVSH_SMI_COMMUNICATION_H_

#define DVSH_SMI_HANDLER_GUID \
  { 0x25e6d8fc, 0x29fa, 0x48c2, { 0x8f, 0x8c, 0x63, 0x05, 0xed, 0xa8, 0xf3, 0xee } }

extern EFI_GUID gDvshSmiHandlerGuid;

//
// Command catalogue. Ids 0x0F, 0x10, 0x11 and 0x14 are the first rows of the
// heap, stack and global OOB grids. The rest of each grid is appended: heap at
// 0x1F-0x24, stack at 0x25-0x2B, global at 0x2D-0x33. 0x2C is reserved and
// left unhandled by the handler on purpose.
//
typedef enum {
  DvshCmdRawReadMem                 = 0x01,
  DvshCmdRawWriteMem                = 0x02,
  DvshCmdSmmCallout                 = 0x03,
  DvshCmdSmramWrite                 = 0x04,
  DvshCmdCorruptedFunctionPointer   = 0x05,
  DvshCmdSignedIntOverflow          = 0x06,
  DvshCmdIntUnderflow               = 0x07,
  DvshCmdShiftOutOfBounds           = 0x08,
  DvshCmdArrayIndexOutOfBounds      = 0x09,
  DvshCmdMisalignedAccess           = 0x0A,
  DvshCmdDivideByZero               = 0x0B,
  DvshCmdNullDeref                  = 0x0C,
  DvshCmdPointerArithOverflow       = 0x0D,
  DvshCmdInvalidBoolEnumLoad        = 0x0E,
  DvshCmdHeapOobReadOverflow1Byte   = 0x0F,  // reused from old DvshCmdHeapOobRead
  DvshCmdHeapOobWriteOverflow1Byte  = 0x10,  // reused from old DvshCmdHeapOobWrite
  DvshCmdStackOobWriteOverflow1Byte = 0x11,  // reused from old DvshCmdStackOobWrite
  DvshCmdUseAfterFree               = 0x12,
  DvshCmdDoubleFree                 = 0x13,
  DvshCmdGlobalOobReadOverflow1Byte = 0x14,  // reused from old DvshCmdGlobalOob
  DvshCmdUseAfterReturn             = 0x15,
  DvshCmdUseAfterScope              = 0x16,
  DvshCmdUninitializedRead          = 0x17,
  DvshCmdStackCanarySmash           = 0x18,
  DvshCmdInvalidBool                = 0x19,
  DvshCmdUnreachable                = 0x1A,
  DvshCmdVlaBound                   = 0x1B,
  DvshCmdNonnullAttribute           = 0x1C,
  DvshCmdReturnsNonnullAttribute    = 0x1D,
  DvshCmdBuiltin                    = 0x1E,
  DvshCmdHeapOobReadOverflowHuge    = 0x1F,
  DvshCmdHeapOobReadUnderflow1Byte  = 0x20,
  DvshCmdHeapOobReadUnderflowHuge   = 0x21,
  DvshCmdHeapOobWriteOverflowHuge   = 0x22,
  DvshCmdHeapOobWriteUnderflow1Byte = 0x23,
  DvshCmdHeapOobWriteUnderflowHuge  = 0x24,
  DvshCmdStackOobReadOverflow1Byte   = 0x25,
  DvshCmdStackOobReadOverflowHuge    = 0x26,
  DvshCmdStackOobReadUnderflow1Byte  = 0x27,
  DvshCmdStackOobReadUnderflowHuge   = 0x28,
  DvshCmdStackOobWriteOverflowHuge   = 0x29,
  DvshCmdStackOobWriteUnderflow1Byte = 0x2A,
  DvshCmdStackOobWriteUnderflowHuge  = 0x2B,
  DvshCmdReserved0x2C                 = 0x2C,  // explicitly reserved: gap between the stack grid (0x25-0x2B) and the global grid (0x2D-0x33) below; unhandled by the switch, by design
  DvshCmdGlobalOobReadOverflowHuge    = 0x2D,
  DvshCmdGlobalOobReadUnderflow1Byte  = 0x2E,
  DvshCmdGlobalOobReadUnderflowHuge   = 0x2F,
  DvshCmdGlobalOobWriteOverflow1Byte  = 0x30,
  DvshCmdGlobalOobWriteOverflowHuge   = 0x31,
  DvshCmdGlobalOobWriteUnderflow1Byte = 0x32,
  DvshCmdGlobalOobWriteUnderflowHuge  = 0x33
} DVSH_COMMAND;

typedef enum {
  DvshStatusSuccess        = 0,
  DvshStatusInvalidCommand = 1,
  DvshStatusInvalidSize    = 2
} DVSH_STATUS;

#define DVSH_MAX_DATA_BYTES 256

//
// One flat struct for every command: a byte-mutation fuzzer and a seed
// crafter both stay branch-free (one layout, one switch in the handler)
// instead of a per-command union.
//
typedef struct {
  UINT32 Command;
  UINT32 Status;
  UINT64 Address;
  UINT64 Value;
  UINT32 Size;
  UINT32 Index;
  UINT8  Data[DVSH_MAX_DATA_BYTES];
} DVSH_SMI_REQUEST;

#endif
