# Testing SMM Memory Protections With TSFFS Replay

This tutorial drives the **Damn Vulnerable SMI Handler (DVSH)**, an SMM driver
with about 50 deliberate memory-safety and undefined-behavior bugs behind one
flat request struct, on the public EDK2 Simics QSP board. It does not run a
fuzzing campaign. For each bug there is one hand-crafted seed, and TSFFS
*replays* the whole set against the same firmware built several ways. The result is
a deterministic answer to "which of these bugs does each protection turn into
an observable fault?"

| Step | Build | What it adds |
|---|---|---|
| 1 | `./build.sh 1` | Nothing. Baseline. |
| 2 | `./build.sh 2` | SMM page-table protection (`PcdCpuSmmRestrictedMemoryAccess`) |
| 2u | `./build.sh 2u` | Step 2 plus UBSan (trap mode) in the handler |
| 3 | `./build.sh 3` | SMM Heap Guard (`PcdHeapGuardPropertyMask`) **instead of** step 2 |
| 4 | `./build.sh 4` | Steps 2 and 3 together. This one is expected to fail. |
| 5off | `./build.sh 5off` | Control for the next three: the same board built from FuzzUEr's forks with clang 15, `DvshSmi` not instrumented |
| 5poff | `./build.sh 5poff` | 5off plus SMM page-table protection |
| 5 | `./build.sh 5` | AddressSanitizer and UBSan (FuzzUEr's runtime) on `DvshSmi` |
| 5p | `./build.sh 5p` | Step 5 plus SMM page-table protection |

## Layout

```
DamnVulnerableSmmPkg/DvshSmi        the vulnerable SMI handler (DXE_SMM_DRIVER)
DamnVulnerableSmmPkg/DvshHarness    TSFFS harness: SmmCommunication + HARNESS_START/STOP
config/                             DSC includes: step toggles, SMM protection PCDs
qsp-dvsh.patch                      wires the above into SimicsOpenBoardPkg
edk2-harness-assert.patch           HARNESS_ASSERT() in CpuDeadLoop/CpuBreakpoint (MdePkg)
fuzzuer/                            steps 5off-5p: Dockerfile, config and patches for FuzzUEr's forks
Dockerfile, build.sh                builds one step into project/images/step<N>.fd
gen_seeds.py                        crafts one seed per DVSH command
report.py                           turns the replay log into one verdict per seed
project/boot.simics                 boots an image and keeps the serial log
project/replay.simics               replays a seed directory through the handler
project/run-step.sh                 boot + seeds + replay + report for one step
project/run-isolated.sh             the same with a fresh Simics per seed (the sanitizer steps)
```

DVSH is a self-contained package that depends on `MdePkg` and `MdeModulePkg`
only. The command catalogue is in
`DamnVulnerableSmmPkg/Include/Guid/DvshSmiCommunication.h`.

## The target request

Every command uses one struct, so a byte-mutating fuzzer never has to pick a
layout:

```c
typedef struct {
  UINT32 Command;   // selects the bug
  UINT32 Status;
  UINT64 Address;   // used by raw-read/raw-write/smram-write/...
  UINT64 Value;
  UINT32 Size;
  UINT32 Index;
  UINT8  Data[256];
} DVSH_SMI_REQUEST;
```

## The harness

`DvshHarness` is a DXE driver. At `ReadyToBoot`, after `SmmReadyToLock`, so
every SMM protection is live, it finds a PI SMM communication region, addresses
it to `gDvshSmiHandlerGuid`, and runs one testcase per loop iteration:

```c
HARNESS_START_WITH_MAXIMUM_SIZE_AND_PTR_INDEX (0, CommBuffer->Data, &InputSize, PayloadMaxSize);
if (InputSize < sizeof (DVSH_SMI_REQUEST)) { HARNESS_STOP (); continue; }
SmmCommunication->Communicate (SmmCommunication, CommBuffer, &CommBufferSize);
HARNESS_STOP ();
```

The testcase TSFFS writes becomes the request. TSFFS treats exceptions 6 (#UD,
what UBSan traps raise), 13 (#GP) and 14 (#PF), and the 3 s timeout, as
solutions. The same harness would work unchanged for a fuzzing campaign. Here
the testcases are crafted instead.

## Which protections can be toggled on this board

Checked against the public EDK2 sources used here (`UefiCpuPkg.dec`,
`MdeModulePkg.dec`, `SimicsOpenBoardPkg` `OpenBoardPkg*.dsc`):

| PCD | Board default | Used here |
|---|---|---|
| `gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmRestrictedMemoryAccess` | not set (package default `TRUE`) | **step 2**: set `TRUE`; steps 1 and 3 set it `FALSE` |
| `gEfiMdeModulePkgTokenSpaceGuid.PcdHeapGuardPropertyMask` (+ `PcdHeapGuardPoolType`, `PcdHeapGuardPageType`) | not set (`0`) | **step 3**: `0x0F`, pool type `0x5E` |
| `gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmStackGuard` | `FALSE` | not changed |
| `gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmCodeAccessCheckEnable` | `TRUE` | not changed |
| `gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmProfileEnable` | `FALSE` | not changed. It also conflicts with restricted memory access. |

The first two are mutually exclusive on X64. `UefiCpuPkg.dec` says so, and
step 4 shows what that looks like.

One board fix is needed before SMM works at all: `qsp-dvsh.patch` flips
`PcdSmrrEnable` to `TRUE` in `OpenBoardPkgPcd.dsc`. The Simics CPU advertises
SMRR, the stock board says `FALSE`, and `PiSmmCpuDxeSmm` then fails an
`ASSERT` and dead-loops during boot, so no SMM driver (including DVSH) ever
runs.

## Building

You need Docker, and a Simics project with the QSP packages and a TSFFS that
supports replaying a directory (`repro` on a directory, TSFFS PR #471 or later).

```sh
./build.sh 1     # also: 2, 2u, 3, 4
```

Each build clones the pinned EDK2 repositories, so the first one is slow. It
writes `project/images/step<N>.fd` and the handler's `DvshSmi.debug` next to it.
The step toggles reach EDK2 as a generated `DvshStep.dsc.inc`, because
`build_bios.py` does not forward environment variables to `build`.

## The seeds

`gen_seeds.py` writes one 288-byte `DVSH_SMI_REQUEST` per command, with the
arguments that make that bug fire:

| Command | Arguments |
|---|---|
| raw-read-mem, raw-write-mem | `Address=0`, `Size=16`: low memory, outside SMRAM |
| smram-write | `Address` = `mDvshGlobalBuffer`, a writable byte inside SMRAM, `Size=1` |
| signed-int-overflow | `Value=1` (1 + `INT32_MAX`) |
| shift-OOB | `Size=32` (`1u << 32`) |
| array-index-OOB | `Index=16` (one past a 16-byte array) |
| stack-canary-smash | `Value=4` |
| everything else | the command id alone. Those bugs trigger on it by themselves. |

The in-SMRAM address depends on where the firmware loads the driver, so
`run-step.sh` boots the image first, reads `Loading SMM driver at ... DvshSmi.efi`
from the serial log, and adds the offset of `mDvshGlobalBuffer` from
`DvshSmi.debug`. Without that, `gen_seeds.py` aims `smram-write` at address 0,
which is outside SMRAM and not the case it is meant to show.

## Running

From the Simics project, with `images/` filled by `build.sh`, and `boot.simics`,
`replay.simics`, `run-step.sh` and `targets/qsp-x86/` (the QSP target that boots the
step image) copied in:

```sh
TUTORIAL=/path/to/examples/tutorials/edk2-smm-dvsh ./run-step.sh 2
```

It boots `images/step2.fd` (`boot.simics`), crafts `seeds-step2/`, replays them
(`replay.simics`) and prints one verdict per seed:

```
01-RawReadMem                              caught #PF
02-RawWriteMem                             caught #PF
...
18 of 50 caught
```

`replay.simics` is two lines of TSFFS configuration and one call:

```python
@tsffs.iface.fuzz.repro(SIM_lookup_file("%simics%") + "/seeds-step" + simenv.step)
```

TSFFS runs every seed of the directory in name order, each from the initial
snapshot, and logs `Replayed <seed>: <stop reason>`. `report.py` reads those
lines. A seed is *caught* when the stop reason is an exception or a timeout, and
*survived* when the handler returned and the harness stopped normally. Each step
takes about a minute and a half, and the result is the same on every run.

The sanitizer steps (5off to 5p) use `run-isolated.sh` instead, which boots a fresh
Simics for every seed and runs `JOBS` of them at a time. Replaying the whole
directory in one session wedged on the instrumented images: after one particular
seed (`use-after-return`) every later seed faulted before the harness ran, so
those verdicts meant nothing. I did not find why. A fresh boot per seed avoids it.

```sh
TUTORIAL=/path/to/examples/tutorials/edk2-smm-dvsh ./run-isolated.sh 5p 12
```

## Results

50 seeds per step (the reserved id 0x2C is skipped). Commands not listed
survived in all four steps. That is the interesting part: no protection here
turns them into a fault.

| Seed | 1 none | 2 page-table | 2u PT + UBSan | 3 Heap Guard |
|---|:-:|:-:|:-:|:-:|
| smm-callout, corrupted-function-pointer | #PF | #PF | #PF | #PF |
| the 6 `*-OOB-*-overflow-huge` seeds (heap, stack, global) | #GP | #GP | #GP | #GP |
| the 6 `*-OOB-*-underflow-huge` seeds | #GP | #GP | #UD | #GP |
| raw-read-mem, raw-write-mem | | #PF | #PF | |
| null-deref | | #PF | #UD | |
| signed-int-overflow, int-underflow, shift-OOB, array-index-OOB, misaligned-access, divide-by-zero, stack-canary-smash, invalid-bool, unreachable, vla-bound, nonnull-attribute, returns-nonnull-attribute, builtin (13) | | | #UD | |
| heap-oob-read/write-overflow-1byte | | | | #PF |
| double-free (back to back) | ASSERT | ASSERT | ASSERT | #PF |
| **Caught** | **15** | **18** | **31** | **17** |

Reading it:

- **The `*-huge` OOB seeds need no protection.** They add or subtract
  `0x7FFFFFFF00000000` and produce a non-canonical address, so the CPU raises
  #GP on its own.
- **Page-table protection (step 2)** unmaps ordinary RAM from SMM. The raw
  read, the raw write and the NULL dereference now fault with #PF, where in
  step 1 they silently succeeded.
- **UBSan (step 2u)** is the only thing that sees undefined behavior the
  hardware accepts: signed overflow, a misaligned load, a bad shift, a
  one-past-the-end index. Each traps as #UD. It is the full
  `-fsanitize=undefined`, in trap mode (no runtime library), applied to
  `DvshSmi` only through a module-level `<BuildOptions>` override, so the rest
  of the firmware is untouched and boot is unaffected. Seven seeds that the
  hardware already caught now trap as #UD first: null-deref (#PF before) and the
  six `*-underflow-huge` seeds (#GP before). I did not check which UBSan check
  fires for them.
- **Heap Guard (step 3)** turns the two one-byte heap overflows into #PF on the
  guard page. The double free also ends in a #PF there (see below).
- **The double-free result is not a protection, and it is not what a real
  double free looks like.** The seed frees the same block twice in a row. The
  first free zeroes the pool header signature, so the second `FreePool()` hits
  `ASSERT (FreePoolHdr->Header.Signature == POOL_HEAD_SIGNATURE)` in
  `MdeModulePkg/Core/PiSmmCore/Pool.c:386`. That is a sanity check of the
  allocator, and only a DEBUG build turns it into a stop. A real bug usually
  frees a stale pointer after the block was handed out again
  (`FreePool (p); q = AllocatePool (16); FreePool (p);`). Then the header is
  valid again, the check passes, and `q`'s block goes back on the free list
  while `q` is still in use. This tutorial does not include that case, and I
  have not measured how each step treats it.
- **`CpuDeadLoop()` and `CpuBreakpoint()` report to TSFFS.** `edk2-harness-assert.patch`
  puts `HARNESS_ASSERT()` at their entry, so any EDK2 `ASSERT` ends as a TSFFS
  assert solution (`StopAssert`) at once. Without it the double free above ran
  into `CpuDeadLoop()` and was reported as a 3 s timeout. In step 3 the replay
  stops with a #PF at an address inside `PiSmmCore`. I did not analyze it
  further, so I can't say which access faulted.
- **Neither protection helps with** use-after-free, use-after-return,
  use-after-scope, the uninitialized read, the one-byte stack and global
  over/underflows, the one-byte heap *underflows* (`PcdHeapGuardPropertyMask`
  bit 7 is clear, so the buffer hugs the tail guard, not the head), and
  smram-write. For smram-write the address is valid SMRAM, so the write is
  legal as far as the page tables are concerned. These are the cases that would
  need a real SMM sanitizer.

### Step 4: both protections together

`./build.sh 4` builds, and then the firmware stops during boot:

```sh
./simics --no-win --batch-mode -e '$step="4"' boot.simics
grep ASSERT boot-step4.log
```

```
DXE_ASSERT!: [PiSmmCpuDxeSmm] .../UefiCpuPkg/PiSmmCpuDxeSmm/SmmCpuMemoryManagement.c (1839):
  !(IsRestrictedMemoryAccess () && (_gPcd_FixedAtBuild_PcdHeapGuardPropertyMask & (0x00000008 | 0x00000004)) != 0)
```

`PiSmmCpuDxeSmm` asserts that restricted memory access and the SMM heap-guard
bits (`BIT2 | BIT3`) are not both set, then dead-loops. Nothing after it runs,
including DVSH and the harness, so the harness never reaches `HARNESS_START`
(the boot log never prints `DVSH harness: CommBuffer`) and there is nothing to
replay.

## Step 5: AddressSanitizer from FuzzUEr

[FuzzUEr](https://github.com/BreakingBoot/FuzzUEr) (MIT) carries an AddressSanitizer
runtime for EDK2, and forks of `edk2` and `edk2-platforms` that already wire it into
this board, including the shadow memory. Steps 5off to 5p reuse those forks instead of
porting the runtime to the upstream tree used by steps 1 to 4. They build with clang 15
(`-t CLANGSAN`) from `fuzzuer/Dockerfile`, at pinned commits of
`cglosner/edk2@simics-working` and `cglosner/edk2-platforms@simics-working`.

Only `DvshSmi` is switched between instrumented and not. **The fork instruments the
platform's own SMM modules whatever that switch says**, so 5off is a control for DVSH, not
a build without any sanitizer. `PiSmmCore`'s pool, for example, reports a double free in
all four columns below.

A sanitizer report reaches TSFFS as an assert stop (`cpuid` with `eax = 0x54711`), the
same stop `HARNESS_ASSERT` gives an EDK2 `ASSERT`, so `report.py` reads the serial output
to say which it was: `asan <class>`, `ubsan <check>` or a plain assert. The harness opens
the sanitizer's fuzzing window around the SMI (`AsanSetFuzzingActive`); outside it a
report is only printed.

### Making it work with restricted SMM memory access

Step 5p was blocked at first, and the stock fork would not boot with
`PcdCpuSmmRestrictedMemoryAccess=TRUE` even with `DvshSmi` uninstrumented. (The fork's own
`UefiCpuPkg.dec` defaults that PCD to `FALSE`, so it had never run that way.) Three things
had to change, all in third-party code:

1. **The shadow was free memory in DXE's eyes.** `SimicsPei/MemDetect.c` published a
   `RT_Data` allocation HOB for the 448 MB shadow inside the one system-memory resource,
   and DXE's memory map at `EndOfDxe` showed it as part of a single `Conventional`
   entry. Restricted access marks free memory not present in the SMM page tables, so the
   first instrumented SMM access read an unmapped shadow byte and took a #PF. The patch
   publishes the shadow as its own reserved resource range.
2. **PEI memory spanned the shadow.** Splitting the resource HOB then failed in `DxeCore`
   (`ASSERT (Found)`, no single resource HOB held the PHIT range), because PEI memory was
   published as everything above the DXE FV. PEI memory now starts after the shadow.
3. **The runtime read its shared state through the HOB list.** `AsanLib` keeps the
   fuzzing window and the untrusted regions in `ASAN_INFO`, reached through a pointer into
   the HOB list, which is `BootServicesData` and also unmapped from SMM. With 1 and 2
   fixed, the next fault was `cmp DWORD PTR [rax+0xa8], 0` on that pointer. A new
   `ASAN_INFO.AsanSharedInfo` field lets a platform keep that state in a reserved page in
   front of the shadow; zero keeps the old behavior.

Fixes 1 and 2 are in `fuzzuer/edk2-platforms.patch`, fix 3 in
`fuzzuer/edk2-asan-shared-state.patch`. The platform also needed `ZeroMem` on its
`ASAN_INFO` before filling it: the struct has grown fields since that code was written, and
the stale ones were stack garbage that sent the runtime reading junk and hung the boot.
These patches are mine, not FuzzUEr's. Fix 3 changes their runtime, so it is a proposal
to send back, not something to rely on being upstream.

### Results

50 seeds per step, a fresh boot per seed. Commands not listed survived in all four
columns, except those in the second list.

| Seed | 5off | 5poff | 5 | 5p |
|---|:-:|:-:|:-:|:-:|
| smm-callout, corrupted-function-pointer | #PF | #PF | #PF | #PF |
| the 6 `*-OOB-*-overflow-huge` seeds | #GP | #GP | #GP | #GP |
| double-free | asan | asan | asan | asan |
| raw-read-mem, raw-write-mem, null-deref | | #PF | | #PF |
| signed-int-overflow, int-underflow, shift-OOB, array-index-OOB, divide-by-zero, stack-canary-smash, unreachable (7) | | | ubsan | ubsan |
| the 6 `*-OOB-*-underflow-huge` seeds | #GP | #GP | ubsan `pointer_overflow` | ubsan `pointer_overflow` |
| heap 1-byte OOB, read and write, overflow and underflow (4) | | | asan `heap-buffer-overflow` | asan `heap-buffer-overflow` |
| stack 1-byte OOB (4) | | | asan `stack-buffer-overflow` | asan `stack-buffer-overflow` |
| global 1-byte OOB (4) | | | asan `global-buffer-overflow` | asan `global-buffer-overflow` |
| use-after-free | | | asan `heap-use-after-free` | asan `heap-use-after-free` |
| vla-bound | | | timeout | #PF |
| **Caught** | **15** | **18** | **36** | **39** |

Survived in 5p: smram-write, misaligned-access, pointer-arith-overflow,
invalid-bool-enum-load, use-after-return, use-after-scope, uninitialized-read,
invalid-bool, nonnull-attribute, returns-nonnull-attribute and builtin.

Reading it:

- **5poff matches step 2** on the upstream tree: restricted access adds the raw read, the
  raw write and the NULL dereference, as #PF. That the patched fork reproduces it is the
  check that the platform fix did not change the memory protection.
- **ASan finds what no protection did**: the 1-byte heap, stack and global overflows and
  underflows (12 seeds) and the use-after-free. Page-table protection and Heap Guard
  never saw these as faults, except the heap overflows under Heap Guard.
- **The two protections stack**: 5p is 5 plus the three #PF from 5poff.
- **What it still misses** is mostly what its configuration turns off. The runtime has no
  handler for the `bool`, `nonnull-attribute`, `returns-nonnull-attribute`, `builtin` and
  `vla-bound` UBSan checks, so they are disabled and four of those seeds survive. The
  fork's flags disable the alignment check (misaligned-access) and use-after-scope, and
  there is no fake stack for use-after-return. `uninitialized-read` needs MSan.
  `smram-write` is a legal write into SMRAM, which no sanitizer here knows is wrong.
- **`vla-bound` differs between 5 and 5p** (timeout against #PF), and I did not look into
  why.
- **`double-free` is caught by `PiSmmCore`, not by `DvshSmi`'s instrumentation**, which is
  why the control column has it too. It is also still the back-to-back shape from step 1,
  not a stale-pointer double free.

## Limitations

- The results come from TSFFS built from this repository at `01eb52f`, with
  Simics 7 and the QSP x86 package.
- Step 4's failure and the double-free `ASSERT` are DEBUG-build behavior. I did
  not test a RELEASE build, where they would behave differently.
- Steps 5off to 5p depend on two third-party forks pinned by SHA in `fuzzuer/Dockerfile`.
  I did not check them against newer FuzzUEr commits, and the patches here are against
  those SHAs only.
- I did not test a RELEASE build for steps 5off to 5p either.
- The in-SMRAM address trick assumes `nm` is installed and the build was made
  with `build.sh`, so that `DvshSmi.debug` matches the image.
