/** @file
  Damn Vulnerable SMI Handler (DVSH).

  Registers a single SMI handler identified by gDvshSmiHandlerGuid that
  dispatches on DVSH_SMI_REQUEST.Command to a catalogue of intentional bugs,
  one clean trigger per memory-safety or undefined-behavior class:

  - raw-read-mem / raw-write-mem: raw memory access at a caller-supplied
    address, with no SmmIsBufferOutsideSmmValid guard, by design.
  - smm-callout: calls a Boot Services function pointer stashed at DXE load
    time (code living outside SMRAM) from SMM context.
  - smram-write: the same unguarded write, aimed by the caller at a valid
    SMRAM address to show the write silently succeeds there.
  - corrupted-function-pointer: an indirect call through a caller-supplied
    address.
  - UBSan family: each command sinks its result through a volatile local so
    GCC -O2 cannot fold the bug away. null-deref faults on real hardware (#PF);
    divide-by-zero's unused result lets GCC drop the division unless UBSan
    checks it; the rest are undefined behavior with no x86 trap and print
    SURVIVED.
  - heap / stack / global OOB grids: eight commands per region, one per
    (access, direction, magnitude): read or write, overflow or underflow, 1
    byte or a huge offset. The offset is fixed in the handler and Value is
    ignored, so a seed carries only the command id. The huge offsets produce a
    non-canonical address and fault (#GP). The 1-byte cases stay inside
    SMM-managed memory and SURVIVE unless something instruments the access.
  - use-after-free, double-free, use-after-return, use-after-scope: lifetime
    bugs on pool and stack memory.
  - uninitialized-read: branches on freshly allocated, never initialized pool
    contents.
  - stack-canary-smash: an out-of-bounds stack write aimed at where
    -fstack-protector's canary would sit, if this module's command line did
    not carry -fno-stack-protector like every other EDK2 module in this build.
  - invalid-bool, unreachable, vla-bound, nonnull-attribute,
    returns-nonnull-attribute, builtin: UBSan checks that trigger on the
    command alone.

  The three OOB grids share one DvshGridDispatch() taking a DVSH_GRID_ROW
  table and a region-specific buffer pointer.

  Every command is bracketed by an "ENTER" print before the bug and a
  "SURVIVED" print after -- so a bug that slips through uncaught is
  visible as a printed SURVIVED line rather than a halted handler.
  Markers go straight to SerialPortLib rather than DEBUG(): this
  platform's DSC does not set the Pcd that gates DEBUG() output for new
  packages, which would otherwise silently drop every marker. SerialPortLib
  is the same underlying transport DebugLib itself writes to once enabled,
  and is SMM-safe (no Boot Services dependency), unlike UefiLib's Print().

  SPDX-License-Identifier: BSD-2-Clause-Patent
**/

#include <PiSmm.h>
#include <Library/BaseMemoryLib.h>
#include <Library/MemoryAllocationLib.h>
#include <Library/PrintLib.h>
#include <Library/SerialPortLib.h>
#include <Library/SmmServicesTableLib.h>
#include <Library/UefiDriverEntryPoint.h>

#include <Guid/DvshSmiCommunication.h>

// gDvshSmiHandlerGuid itself is defined by AutoGen (the INF's [Guids] ##
// PRODUCES entry generates the storage); only the extern declaration from
// the header above is needed here.

// Fixed-size global buffer for global-OOB, distinct from the heap (pool)
// and stack (local array) OOB commands.
STATIC UINT8  mDvshGlobalBuffer[16];

// Stashed at DXE load time (DvshSmiEntryPoint), while Boot Services is
// still live, then called from the SMI handler below -- the classic SMM
// callout pattern: a pointer captured before (or spanning) the
// ExitBootServices boundary, dereferenced from a context where it points
// outside SMRAM.
STATIC EFI_BOOT_SERVICES  *mDvshBootServices = NULL;

//
// Targets for -fsanitize=nonnull-attribute / returns-nonnull-attribute. The
// check keys on the attribute; noinline keeps the call and return real.
//
STATIC VOID __attribute__ ((noinline, nonnull)) DvshTakesNonNull (UINT8 *Ptr)
{
  __asm__ volatile ("" :: "r" (Ptr));
}

STATIC UINT8 * __attribute__ ((noinline, returns_nonnull)) DvshReturnsNonNull (VOID)
{
  volatile UINTN  Zero;

  Zero = 0;
  return (UINT8 *) Zero;
}


typedef VOID (EFIAPI *DVSH_CALLBACK)(VOID);

STATIC
VOID
EFIAPI
DvshLog (
  IN CONST CHAR8  *Format,
  ...
  )
{
  CHAR8    Buffer[256];
  UINTN    Length;
  VA_LIST  Marker;

  VA_START (Marker, Format);
  Length = AsciiVSPrint (Buffer, sizeof (Buffer), Format, Marker);
  VA_END (Marker);

  SerialPortWrite ((UINT8 *) Buffer, Length);
}

#define DVSH_ENTER(Name, Cmd) \
  DvshLog ("[DVSH] >>> ENTER " Name " cmd=0x%02x\n", (UINT32) (Cmd))

#define DVSH_SURVIVED(Name, Cmd) \
  DvshLog ("[DVSH] <<< SURVIVED " Name " cmd=0x%02x\n", (UINT32) (Cmd))

//
// Shared grid row/access shape for the heap, stack and global OOB
// grids: one row per (access, direction, magnitude)
// case. The offset is fixed here -- never taken from the request's Value
// field -- so a seed for one of these commands carries only the command
// id. Access size is one byte against each call site's fixed 16-byte
// region (AllocatePool() object, local array, or mDvshGlobalBuffer).
//
typedef enum {
  DvshGridAccessRead,
  DvshGridAccessWrite
} DVSH_GRID_ACCESS;

typedef struct {
  DVSH_COMMAND      Command;
  DVSH_GRID_ACCESS  Access;
  INTN              Offset;
  CONST CHAR8       *Name;
} DVSH_GRID_ROW;

STATIC CONST DVSH_GRID_ROW  mDvshHeapGrid[] = {
  { DvshCmdHeapOobReadOverflow1Byte,   DvshGridAccessRead,  16,                             "heap-oob-read-overflow-1byte"    },
  { DvshCmdHeapOobReadOverflowHuge,    DvshGridAccessRead,  0x7FFFFFFF00000000LL,           "heap-oob-read-overflow-huge"     },
  { DvshCmdHeapOobReadUnderflow1Byte,  DvshGridAccessRead,  -1,                             "heap-oob-read-underflow-1byte"   },
  { DvshCmdHeapOobReadUnderflowHuge,   DvshGridAccessRead,  -0x7FFFFFFF00000000LL,          "heap-oob-read-underflow-huge"    },
  { DvshCmdHeapOobWriteOverflow1Byte,  DvshGridAccessWrite, 16,                             "heap-oob-write-overflow-1byte"   },
  { DvshCmdHeapOobWriteOverflowHuge,   DvshGridAccessWrite, 0x7FFFFFFF00000000LL,           "heap-oob-write-overflow-huge"    },
  { DvshCmdHeapOobWriteUnderflow1Byte, DvshGridAccessWrite, -1,                             "heap-oob-write-underflow-1byte"  },
  { DvshCmdHeapOobWriteUnderflowHuge,  DvshGridAccessWrite, -0x7FFFFFFF00000000LL,          "heap-oob-write-underflow-huge"   },
};

STATIC CONST DVSH_GRID_ROW  mDvshStackGrid[] = {
  { DvshCmdStackOobReadOverflow1Byte,   DvshGridAccessRead,  16,                             "stack-oob-read-overflow-1byte"    },
  { DvshCmdStackOobReadOverflowHuge,    DvshGridAccessRead,  0x7FFFFFFF00000000LL,           "stack-oob-read-overflow-huge"     },
  { DvshCmdStackOobReadUnderflow1Byte,  DvshGridAccessRead,  -1,                             "stack-oob-read-underflow-1byte"   },
  { DvshCmdStackOobReadUnderflowHuge,   DvshGridAccessRead,  -0x7FFFFFFF00000000LL,          "stack-oob-read-underflow-huge"    },
  { DvshCmdStackOobWriteOverflow1Byte,  DvshGridAccessWrite, 16,                             "stack-oob-write-overflow-1byte"   },
  { DvshCmdStackOobWriteOverflowHuge,   DvshGridAccessWrite, 0x7FFFFFFF00000000LL,           "stack-oob-write-overflow-huge"    },
  { DvshCmdStackOobWriteUnderflow1Byte, DvshGridAccessWrite, -1,                             "stack-oob-write-underflow-1byte"  },
  { DvshCmdStackOobWriteUnderflowHuge,  DvshGridAccessWrite, -0x7FFFFFFF00000000LL,          "stack-oob-write-underflow-huge"   },
};

STATIC CONST DVSH_GRID_ROW  mDvshGlobalGrid[] = {
  { DvshCmdGlobalOobReadOverflow1Byte,   DvshGridAccessRead,  16,                             "global-oob-read-overflow-1byte"    },
  { DvshCmdGlobalOobReadOverflowHuge,    DvshGridAccessRead,  0x7FFFFFFF00000000LL,           "global-oob-read-overflow-huge"     },
  { DvshCmdGlobalOobReadUnderflow1Byte,  DvshGridAccessRead,  -1,                             "global-oob-read-underflow-1byte"   },
  { DvshCmdGlobalOobReadUnderflowHuge,   DvshGridAccessRead,  -0x7FFFFFFF00000000LL,          "global-oob-read-underflow-huge"    },
  { DvshCmdGlobalOobWriteOverflow1Byte,  DvshGridAccessWrite, 16,                             "global-oob-write-overflow-1byte"   },
  { DvshCmdGlobalOobWriteOverflowHuge,   DvshGridAccessWrite, 0x7FFFFFFF00000000LL,           "global-oob-write-overflow-huge"    },
  { DvshCmdGlobalOobWriteUnderflow1Byte, DvshGridAccessWrite, -1,                             "global-oob-write-underflow-1byte"  },
  { DvshCmdGlobalOobWriteUnderflowHuge,  DvshGridAccessWrite, -0x7FFFFFFF00000000LL,          "global-oob-write-underflow-huge"   },
};

//
// Shared (access x direction x magnitude) search-and-access dispatcher for
// the heap/stack/global OOB grids above -- the three call sites below
// differ only in which region-specific storage they hand in (an
// AllocatePool() block, a stack local array, or the static global array);
// the lookup and the read/write itself are identical, so they live here
// once. Buffer may be NULL (the heap call site's allocation failed): the
// ENTER/SURVIVED markers still print, matching the original per-region
// dispatchers, but the access itself is skipped.
//
STATIC
VOID
DvshGridDispatch (
  IN OUT DVSH_SMI_REQUEST     *Req,
  IN     CONST DVSH_GRID_ROW  *Table,
  IN     UINTN                TableSize,
  IN OUT volatile UINT8       *Buffer
  )
{
  CONST DVSH_GRID_ROW  *Row;
  UINTN                Idx;

  Row = NULL;
  for (Idx = 0; Idx < TableSize; Idx++) {
    if (Table[Idx].Command == (DVSH_COMMAND) Req->Command) {
      Row = &Table[Idx];
      break;
    }
  }

  if (Row == NULL) {
    return;
  }

  DvshLog ("[DVSH] >>> ENTER %a cmd=0x%02x\n", Row->Name, Req->Command);

  if (Buffer != NULL) {
    if (Row->Access == DvshGridAccessRead) {
      volatile UINT8  Value;

      Value = *(Buffer + Row->Offset);
      (VOID) Value;
    } else {
      *(Buffer + Row->Offset) = Req->Data[0];
    }
  }

  DvshLog ("[DVSH] <<< SURVIVED %a cmd=0x%02x\n", Row->Name, Req->Command);
}

STATIC
VOID
DvshHeapGridDispatch (
  IN OUT DVSH_SMI_REQUEST  *Req
  )
{
  UINT8  *Heap;

  Heap = AllocatePool (16);
  DvshGridDispatch (Req, mDvshHeapGrid, ARRAY_SIZE (mDvshHeapGrid), (volatile UINT8 *) Heap);
  if (Heap != NULL) {
    FreePool (Heap);
  }
}

STATIC
VOID
DvshStackGridDispatch (
  IN OUT DVSH_SMI_REQUEST  *Req
  )
{
  // StackArr/Sink stay volatile and are touched before and after the
  // dispatch so GCC5 -O2 cannot remove the 1-byte cases, same guarantee
  // the pre-consolidation per-region dispatcher gave.
  volatile UINT8  StackArr[16];
  volatile UINT8  Sink;

  StackArr[0] = 0xAA;
  DvshGridDispatch (Req, mDvshStackGrid, ARRAY_SIZE (mDvshStackGrid), StackArr);
  Sink = StackArr[0];
  (VOID) Sink;
}

STATIC
VOID
DvshGlobalGridDispatch (
  IN OUT DVSH_SMI_REQUEST  *Req
  )
{
  // Forced write-before-read: guarantees a real memory access to
  // mDvshGlobalBuffer even under aggressive optimization, the same
  // guarantee AllocatePool() (heap) and the volatile local array (stack)
  // already give their regions.
  mDvshGlobalBuffer[0] = 0;
  DvshGridDispatch (Req, mDvshGlobalGrid, ARRAY_SIZE (mDvshGlobalGrid), mDvshGlobalBuffer);
}

// Deliberately returns the address of a local: the bug under test is the
// caller dereferencing it after the frame is gone. The pragma suppresses
// GCC5's -Wreturn-local-addr for exactly this one intentional instance.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wreturn-local-addr"
STATIC
UINT8 *
DvshDanglingStackPointer (
  VOID
  )
{
  UINT8  Local[16];
  UINT8  *Addr;

  Local[0] = 0xAA;
  Addr      = Local;
  __asm__ volatile ("" : "+r"(Addr));
  return Addr;
}
#pragma GCC diagnostic pop

STATIC
EFI_STATUS
EFIAPI
DvshSmiHandler (
  IN     EFI_HANDLE  DispatchHandle,
  IN     CONST VOID  *Context         OPTIONAL,
  IN OUT VOID        *CommBuffer      OPTIONAL,
  IN OUT UINTN       *CommBufferSize  OPTIONAL
  )
{
  DVSH_SMI_REQUEST  *Req;

  if ((CommBuffer == NULL) || (CommBufferSize == NULL) ||
      (*CommBufferSize < sizeof (DVSH_SMI_REQUEST))) {
    DvshLog (
      "[DVSH] rejected: CommBuffer=%p CommBufferSize=%p size=%lu need=%lu\n",
      CommBuffer,
      CommBufferSize,
      (UINTN) (CommBufferSize != NULL ? *CommBufferSize : 0),
      (UINTN) sizeof (DVSH_SMI_REQUEST)
      );
    return EFI_SUCCESS;
  }

  Req = (DVSH_SMI_REQUEST *) CommBuffer;
  Req->Status = DvshStatusSuccess;

  switch (Req->Command) {
    case DvshCmdRawReadMem:
      DVSH_ENTER ("raw-read-mem", Req->Command);
      if ((Req->Size == 0) || (Req->Size > DVSH_MAX_DATA_BYTES)) {
        Req->Status = DvshStatusInvalidSize;
        break;
      }
      CopyMem (Req->Data, (VOID *) (UINTN) Req->Address, Req->Size);
      DVSH_SURVIVED ("raw-read-mem", Req->Command);
      break;

    case DvshCmdRawWriteMem:
      DVSH_ENTER ("raw-write-mem", Req->Command);
      if ((Req->Size == 0) || (Req->Size > DVSH_MAX_DATA_BYTES)) {
        Req->Status = DvshStatusInvalidSize;
        break;
      }
      CopyMem ((VOID *) (UINTN) Req->Address, Req->Data, Req->Size);
      DVSH_SURVIVED ("raw-write-mem", Req->Command);
      break;

    case DvshCmdSmmCallout:
      DVSH_ENTER ("smm-callout", Req->Command);
      if (mDvshBootServices != NULL) {
        mDvshBootServices->Stall (1);
      }
      DVSH_SURVIVED ("smm-callout", Req->Command);
      break;

    case DvshCmdSmramWrite:
      DVSH_ENTER ("smram-write", Req->Command);
      if ((Req->Size == 0) || (Req->Size > DVSH_MAX_DATA_BYTES)) {
        Req->Status = DvshStatusInvalidSize;
        break;
      }
      // No SmmIsBufferOutsideSmmValid guard, by design -- the caller aims
      // Address at a valid SMRAM location to show the write is not
      // checked at all, not just that it reaches outside SMRAM.
      CopyMem ((VOID *) (UINTN) Req->Address, Req->Data, Req->Size);
      DVSH_SURVIVED ("smram-write", Req->Command);
      break;

    case DvshCmdCorruptedFunctionPointer:
      DVSH_ENTER ("corrupted-function-pointer", Req->Command);
      {
        volatile DVSH_CALLBACK  Callback;

        Callback = (DVSH_CALLBACK) (UINTN) Req->Address;
        Callback ();
      }
      DVSH_SURVIVED ("corrupted-function-pointer", Req->Command);
      break;

    case DvshCmdSignedIntOverflow:
      DVSH_ENTER ("signed-int-overflow", Req->Command);
      {
        volatile INT32  A;
        volatile INT32  B;
        volatile INT32  Result;

        A      = (INT32) Req->Value;
        B      = MAX_INT32;
        Result = A + B;
        (VOID) Result;
      }
      DVSH_SURVIVED ("signed-int-overflow", Req->Command);
      break;

    case DvshCmdIntUnderflow:
      DVSH_ENTER ("int-underflow", Req->Command);
      {
        volatile INT32  A;
        volatile INT32  B;
        volatile INT32  Result;

        // Signed subtraction one below INT32_MIN: unlike unsigned
        // wraparound (well-defined), this is genuine signed-integer-
        // underflow UB that a UBSan build will trap.
        A      = MIN_INT32;
        B      = 1;
        Result = A - B;
        (VOID) Result;
      }
      DVSH_SURVIVED ("int-underflow", Req->Command);
      break;

    case DvshCmdShiftOutOfBounds:
      DVSH_ENTER ("shift-OOB", Req->Command);
      {
        volatile UINT32  Shift;
        volatile UINT32  Result;

        Shift  = (UINT32) Req->Size;
        Result = ((UINT32) 1) << Shift;
        (VOID) Result;
      }
      DVSH_SURVIVED ("shift-OOB", Req->Command);
      break;

    case DvshCmdArrayIndexOutOfBounds:
      DVSH_ENTER ("array-index-OOB", Req->Command);
      {
        volatile UINT8  Arr[16];
        volatile UINT8  Value;
        UINTN           Idx;

        Idx      = (UINTN) Req->Index;
        Arr[0]   = 0;
        Value    = Arr[Idx];
        (VOID) Value;
      }
      DVSH_SURVIVED ("array-index-OOB", Req->Command);
      break;

    case DvshCmdMisalignedAccess:
      DVSH_ENTER ("misaligned-access", Req->Command);
      {
        volatile UINT64  Value;
        UINT64           *Unaligned;

        // Req->Data is the start of a 256-byte buffer; offsetting by 1
        // byte guarantees an 8-byte-misaligned UINT64 access.
        Unaligned = (UINT64 *) (VOID *) (Req->Data + 1);
        Value     = *Unaligned;
        (VOID) Value;
      }
      DVSH_SURVIVED ("misaligned-access", Req->Command);
      break;

    case DvshCmdDivideByZero:
      DVSH_ENTER ("divide-by-zero", Req->Command);
      {
        volatile UINT32  Divisor;
        UINT32           Result;

        // Command-only trigger: the divisor is a volatile constant 0.
        // Result is deliberately NOT volatile: without UBSan GCC drops the
        // unused division (no div instruction, so no #DE) and this prints
        // SURVIVED; with -fsanitize=integer-divide-by-zero the check on
        // Divisor stays and traps as #UD (ud2), the UBSan signal.
        Divisor = 0;
        Result  = 100 / Divisor;
        (VOID) Result;
      }
      DVSH_SURVIVED ("divide-by-zero", Req->Command);
      break;

    case DvshCmdNullDeref:
      DVSH_ENTER ("null-deref", Req->Command);
      {
        volatile UINT8  *NullPtr;
        volatile UINT8  Value;

        NullPtr = NULL;
        __asm__ volatile ("" : "+r"(NullPtr));
        Value   = *NullPtr;
        (VOID) Value;
      }
      DVSH_SURVIVED ("null-deref", Req->Command);
      break;

    case DvshCmdPointerArithOverflow:
      DVSH_ENTER ("pointer-arith-overflow", Req->Command);
      {
        volatile UINTN  SunkAddress;
        UINT8           *Overflowed;

        // Computed but never dereferenced: the UB is the out-of-bounds
        // pointer arithmetic itself, not a crash from using the result.
        Overflowed  = Req->Data + (UINTN) Req->Value;
        SunkAddress = (UINTN) Overflowed;
        (VOID) SunkAddress;
      }
      DVSH_SURVIVED ("pointer-arith-overflow", Req->Command);
      break;

    case DvshCmdInvalidBoolEnumLoad:
      DVSH_ENTER ("invalid-bool-enum-load", Req->Command);
      {
        UINT8            RawByte;
        BOOLEAN          InvalidBool;
        volatile BOOLEAN Loaded;

        // Stamp an arbitrary caller-controlled byte into BOOLEAN-sized
        // storage, bypassing normal construction, then load it back --
        // any value other than 0/1 is an invalid load for this type.
        RawByte = Req->Data[0];
        CopyMem (&InvalidBool, &RawByte, sizeof (BOOLEAN));
        Loaded = InvalidBool;
        (VOID) Loaded;
      }
      DVSH_SURVIVED ("invalid-bool-enum-load", Req->Command);
      break;

    case DvshCmdHeapOobReadOverflow1Byte:
    case DvshCmdHeapOobReadOverflowHuge:
    case DvshCmdHeapOobReadUnderflow1Byte:
    case DvshCmdHeapOobReadUnderflowHuge:
    case DvshCmdHeapOobWriteOverflow1Byte:
    case DvshCmdHeapOobWriteOverflowHuge:
    case DvshCmdHeapOobWriteUnderflow1Byte:
    case DvshCmdHeapOobWriteUnderflowHuge:
      DvshHeapGridDispatch (Req);
      break;

    case DvshCmdStackOobReadOverflow1Byte:
    case DvshCmdStackOobReadOverflowHuge:
    case DvshCmdStackOobReadUnderflow1Byte:
    case DvshCmdStackOobReadUnderflowHuge:
    case DvshCmdStackOobWriteOverflow1Byte:
    case DvshCmdStackOobWriteOverflowHuge:
    case DvshCmdStackOobWriteUnderflow1Byte:
    case DvshCmdStackOobWriteUnderflowHuge:
      DvshStackGridDispatch (Req);
      break;

    case DvshCmdUseAfterFree:
      DVSH_ENTER ("use-after-free", Req->Command);
      {
        UINT8  *Heap;

        Heap = AllocatePool (16);
        if (Heap != NULL) {
          volatile UINT8  Value;

          FreePool (Heap);
          // Dangling pointer, read after the free -- the SMM pool
          // allocator does not unmap freed memory, so this stays
          // mapped and SURVIVES without an ASAN-style guard.
          Value = Heap[0];
          (VOID) Value;
        }
      }
      DVSH_SURVIVED ("use-after-free", Req->Command);
      break;

    case DvshCmdDoubleFree:
      DVSH_ENTER ("double-free", Req->Command);
      {
        UINT8  *Heap;

        Heap = AllocatePool (16);
        if (Heap != NULL) {
          FreePool (Heap);
          FreePool (Heap);
        }
      }
      DVSH_SURVIVED ("double-free", Req->Command);
      break;

    case DvshCmdGlobalOobReadOverflow1Byte:
    case DvshCmdGlobalOobReadOverflowHuge:
    case DvshCmdGlobalOobReadUnderflow1Byte:
    case DvshCmdGlobalOobReadUnderflowHuge:
    case DvshCmdGlobalOobWriteOverflow1Byte:
    case DvshCmdGlobalOobWriteOverflowHuge:
    case DvshCmdGlobalOobWriteUnderflow1Byte:
    case DvshCmdGlobalOobWriteUnderflowHuge:
      DvshGlobalGridDispatch (Req);
      break;

    case DvshCmdUseAfterReturn:
      DVSH_ENTER ("use-after-return", Req->Command);
      {
        UINT8           *Dangling;
        volatile UINT8  Value;

        Dangling = DvshDanglingStackPointer ();
        __asm__ volatile ("" : "+r"(Dangling));
        Value    = Dangling[0];
        (VOID) Value;
      }
      DVSH_SURVIVED ("use-after-return", Req->Command);
      break;

    case DvshCmdUseAfterScope:
      DVSH_ENTER ("use-after-scope", Req->Command);
      // The dangling-pointer escape below is exactly the bug under test.
      // No pragma suppression needed: this platform's GCC5 toolchain
      // predates -Wdangling-pointer (GCC12+), so it never warns here.
      {
        UINT8           *Escaped;
        volatile UINT8  Value;

        {
          UINT8  Scoped[16];

          Scoped[0] = 0xBB;
          Escaped   = Scoped;
        }
        // Scoped is out of scope here, but its stack slot is still part
        // of this function's frame -- reads back the stale value and
        // SURVIVES without an ASAN-style guard.
        Value = Escaped[0];
        (VOID) Value;
      }
      DVSH_SURVIVED ("use-after-scope", Req->Command);
      break;

    case DvshCmdUninitializedRead:
      DVSH_ENTER ("uninitialized-read", Req->Command);
      {
        UINT8  *Heap;

        Heap = AllocatePool (16);
        if (Heap != NULL) {
          volatile UINT8  Value;

          // AllocatePool makes no zeroing guarantee -- Heap[0] is
          // uninitialized. Branching on it is exactly what MSAN's
          // uninitialized-value check flags; the volatile sink on both
          // arms keeps GCC5 -O2 from folding the branch away.
          if (Heap[0] != 0) {
            Value = 1;
          } else {
            Value = 0;
          }

          (VOID) Value;
          FreePool (Heap);
        }
      }
      DVSH_SURVIVED ("uninitialized-read", Req->Command);
      break;

    case DvshCmdStackCanarySmash:
      DVSH_ENTER ("stack-canary-smash", Req->Command);
      {
        volatile UINT8  StackArr[16];
        volatile UINT8  Sink;
        UINTN           Idx;

        // Req->Value controls how far past StackArr the write lands --
        // small values only clobber adjacent frame bytes and SURVIVE; a
        // value large enough to reach the saved return address risks
        // crashing on return instead. The intent here is specifically
        // -fstack-protector's canary slot: this build compiles every
        // EDK2 module (this one included) with -fno-stack-protector, so
        // there is no canary to smash and no check to catch this today.
        Idx            = 16 + (UINTN) Req->Value;
        StackArr[Idx]  = Req->Data[0];
        Sink           = StackArr[0];
        (VOID) Sink;
      }
      DVSH_SURVIVED ("stack-canary-smash", Req->Command);
      break;

    case DvshCmdInvalidBool:
      DVSH_ENTER ("invalid-bool", Req->Command);
      {
        volatile _Bool  InvalidBool;
        volatile _Bool  Loaded;

        // Explicit -fsanitize=bool target. EDK2's BOOLEAN is UINT8, so the
        // invalid-bool-enum-load command above has no _Bool load for the
        // check to instrument. Stamp the constant 2 into real _Bool
        // storage through a UINT8 alias, then load it as _Bool: any value
        // other than 0/1 is UB. volatile keeps GCC from folding the load
        // to a known 0/1. The request fields are ignored on purpose, so
        // Command alone triggers it.
        *(volatile UINT8 *) &InvalidBool = 2;
        Loaded = InvalidBool;
        (VOID) Loaded;
      }
      DVSH_SURVIVED ("invalid-bool", Req->Command);
      break;

    case DvshCmdUnreachable:
      DVSH_ENTER ("unreachable", Req->Command);
      {
        volatile UINT32  Selector;
        volatile UINT32  Sink;

        // Command-only trigger: Selector is a constant GCC cannot see
        // through (volatile) and the default arm is declared unreachable.
        Selector = 7;
        switch (Selector) {
          case 0:
            Sink = 0;
            break;
          case 1:
            Sink = 1;
            break;
          default:
            __builtin_unreachable ();
        }
        (VOID) Sink;
      }
      DVSH_SURVIVED ("unreachable", Req->Command);
      break;

    case DvshCmdVlaBound:
      DVSH_ENTER ("vla-bound", Req->Command);
      {
        volatile INTN  Len;

        // Command-only trigger: a negative VLA length from a volatile
        // constant. Without the check the stack adjustment rounds to 0.
        Len = -1;
        {
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wvla"
          UINT8  Vla[Len];

          Vla[0] = 0;
          __asm__ volatile ("" :: "r" (Vla));
#pragma GCC diagnostic pop
        }
      }
      DVSH_SURVIVED ("vla-bound", Req->Command);
      break;

    case DvshCmdNonnullAttribute:
      DVSH_ENTER ("nonnull-attribute", Req->Command);
      {
        volatile UINTN  Zero;

        // Command-only trigger: pass NULL to a nonnull-attributed parameter.
        Zero = 0;
        DvshTakesNonNull ((UINT8 *) Zero);
      }
      DVSH_SURVIVED ("nonnull-attribute", Req->Command);
      break;

    case DvshCmdReturnsNonnullAttribute:
      DVSH_ENTER ("returns-nonnull-attribute", Req->Command);
      {
        volatile UINT8  *Ptr;

        // Command-only trigger: a returns_nonnull function returns NULL.
        Ptr = DvshReturnsNonNull ();
        (VOID) Ptr;
      }
      DVSH_SURVIVED ("returns-nonnull-attribute", Req->Command);
      break;

    case DvshCmdBuiltin:
      DVSH_ENTER ("builtin", Req->Command);
      {
        volatile UINT32  Zero;
        volatile INT32   Result;

        // Command-only trigger: __builtin_clz(0) is undefined.
        Zero   = 0;
        Result = __builtin_clz (Zero);
        (VOID) Result;
      }
      DVSH_SURVIVED ("builtin", Req->Command);
      break;

    default:
      Req->Status = DvshStatusInvalidCommand;
      break;
  }

  DvshLog (
    "[DVSH] cmd=%u addr=0x%lx size=%u status=%u\n",
    Req->Command,
    Req->Address,
    Req->Size,
    Req->Status
    );

  return EFI_SUCCESS;
}

EFI_STATUS
EFIAPI
DvshSmiEntryPoint (
  IN EFI_HANDLE        ImageHandle,
  IN EFI_SYSTEM_TABLE  *SystemTable
  )
{
  EFI_HANDLE  Handle;
  EFI_STATUS  Status;

  mDvshBootServices = SystemTable->BootServices;

  Handle = NULL;
  Status = gSmst->SmiHandlerRegister (
                    DvshSmiHandler,
                    &gDvshSmiHandlerGuid,
                    &Handle
                    );
  DvshLog ("[DVSH] registered handler: %r\n", Status);
  return Status;
}
