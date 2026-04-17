# EDK2 Simics Platform ASAN Status

## Goal

Build and boot an ASAN-enabled `SimicsOpenBoardPkg` firmware for the `edk2-simics-platform` tutorial, then use it as the basis for TSFFS tutorial work.

## What Has Been Changed

### Build Path

The main tutorial Docker build was updated to use the sanitizer branches:

- `edk2`: `https://github.com/cglosner/edk2.git`, branch `simics-sanitizer`
- `edk2-platforms`: `https://github.com/cglosner/edk2-platforms.git`, branch `simics-sanitizer`
- `edk2-non-osi`: `https://github.com/cglosner/edk2-non-osi.git`, branch `simics-sanitizer`
- `FSP`: `https://github.com/cglosner/FSP.git`, branch `simics-sanitizer`

The Docker image also installs the LLVM toolchain and builds:

- platform: `BoardX58Ich10`
- target: `DEBUG`
- toolchain: `CLANGSAN`

The local `edk2-platforms.patch` application in the tutorial Dockerfile is currently disabled so the sanitizer branches build as-is.

### Runtime Path

The BIOS image now being tested is:

`project/workspace/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_CLANGSAN/FV/BOARDX58ICH10.fd`

`project/run.simics` passes this image directly as the BIOS override for the stock `qsp-x86/uefi-shell` target.

## Current Results

### Simics 7

The same `DEBUG_CLANGSAN` firmware image boots on Simics 7 and now reaches the UEFI shell.

The working path is:

- load the stock `qsp-x86/uefi-shell` target with the `DEBUG_CLANGSAN` BIOS override
- attach `minimal_boot_disk.craff` so `FS0:` contains `SimicsAgent.efi`
- select `EFI Internal Shell`
- drive shell input through PS/2 keyboard events
- download and run `tutorial.efi`

Conclusion for Simics 7:

- the firmware image is viable
- the target integration is now good enough to launch the UEFI shell
- the TSFFS harness starts and fuzzes

### Simics 6

Switching to Simics 6 made the target model closer to the original tutorial, but the ASAN firmware no longer boots cleanly.

The current Simics 6 `output.log` shows:

- image start failure:
  - `Error: Image at 000DBE9C000 start failed: 00000001`
- firmware volume problems:
  - `ERROR - The FV in 0xFFE60000 is invalid!`
- repeated sanitizer reports:
  - `ASAN MEMORY ACCESS check fail! __ubsan_handle_pointer_overflow is called:`
  - reported from `MdeModulePkg/Universal/HiiDatabaseDxe/Database.c`
- fatal boot failure:

~~~
<qsp.serconsole.con>GetUefiMemoryMap\r\n
<qsp.serconsole.con>Patch page table start ...\r\n
<qsp.serconsole.con>Patch page table done!\r\n
<qsp.serconsole.con>MemoryAttributesTable - NULL\r\n
[qsp.mb.cpu0.core[0][0] info] Reading from unknown MSR 0x17d. Taking GP fault.
<qsp.serconsole.con>!!!! X64 Exception Type - 0D(#GP - General Protection)  CPU Apic ID - 00000000 !!!!\r\n
<qsp.serconsole.con>ExceptionData - 0000000000000000\r\n
<qsp.serconsole.con>RIP  - 00000000DFEF795D, CS  - 0000000000000038, RFLAGS - 0000000000010002\r\n
<qsp.serconsole.con>RAX  - 0000000000000001, RCX - 000000000000017D, RDX - 00000000DFEC3980\r\n
<qsp.serconsole.con>RBX  - 0000000000000000, RSP - 00000000DFEC3960, RBP - 00000000DFEC39B0\r\n
<qsp.serconsole.con>RSI  - 0000000000000000, RDI - 00000000DFEC39E8\r\n
<qsp.serconsole.con>R8   - 0000000000000010, R9  - 0000000000000000, R10 - 00000000000001E0\r\n
<qsp.serconsole.con>R11  - 00000000DF242470, R12 - 00000000DFEB9050, R13 - 0000000000000000\r\n
<qsp.serconsole.con>R14  - 000000000000017D, R15 - 00000000DFF70EE0\r\n
<qsp.serconsole.con>DS   - 0000000000000020, ES  - 0000000000000020, FS  - 0000000000000020\r\n
<qsp.serconsole.con>GS   - 0000000000000020, SS  - 0000000000000020\r\n
<qsp.serconsole.con>CR0  - 0000000080010033, CR2 - 0000000000000000, CR3 - 00000000DFE86000\r\n
<qsp.serconsole.con>CR4  - 0000000000000668, CR8 - 0000000000000000\r\n
<qsp.serconsole.con>DR0  - 0000000000000000, DR1 - 0000000000000000, DR2 - 0000000000000000\r\n
<qsp.serconsole.con>DR3  - 0000000000000000, DR6 - 00000000FFFF0FF0, DR7 - 0000000000000400\r\n
<qsp.serconsole.con>GDTR - 00000000DFEB8000 000000000000004F, LDTR - 0000000000000000\r\n
<qsp.serconsole.con>IDTR - 00000000DFEBB000 00000000000001FF,   TR - 0000000000000040\r\n
<qsp.serconsole.con>FXSAVE_STATE - 00000000DFEC35C0\r\n
<qsp.serconsole.con>!!!! Find image based on IP(0xDFEF795D) /home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/workspace/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_CLANGSAN/X64/UefiCpuPkg/PiSmmCpuDxeSmm/PiSmmCpuDxeSmm/DEBUG/PiSmmCpuDxeSmm.dll (ImageBase=00000000DFECD000, EntryPoint=00000000DFF0D570) !!!!\r\n

~~~

Conclusion for Simics 6:

- this is no longer a shell-navigation problem
- the ASAN firmware crashes during boot on Simics 6
- switching to Simics 6 does not currently help the ASAN tutorial path

## Important Interpretation

The ASAN firmware is not universally broken, because the same image booted on Simics 7.

The current evidence points to a compatibility gap between:

- the ASAN-enabled firmware build
- and the Simics 6 CPU/platform model

In particular, the fatal Simics 6 stop is tied to an unsupported MSR access during SMM CPU initialization.

## Current Working Hypothesis

There are two separate issues:

1. Simics 7:
   the ASAN firmware boots and can fuzz `Tutorial.efi` when TSFFS is initialized after the shell is ready.
2. Simics 6:
   the older target logic is closer to the tutorial assumptions, but the ASAN firmware itself does not boot reliably on that simulator version.

## TSFFS Initialization Note

Do not call `init-tsffs` at the top of `project/run.simics` for this tutorial.

`init-tsffs` creates the TSFFS object and installs global Simics HAP callbacks. During firmware boot, recognized magic instructions or solution paths can make TSFFS register processor instruction callbacks before the actual harness starts. That makes the ASAN firmware boot significantly slower and can produce misleading pre-harness TSFFS messages.

The current `project/run.simics` intentionally only loads the module early:

~~~
load-module tsffs
~~~

It delays `init-tsffs` and TSFFS configuration until after `SimicsAgent.efi --download tutorial.efi` completes and immediately before launching:

~~~
tutorial.efi
~~~

This keeps TSFFS off the firmware boot path while still enabling harness start/stop handling once the tutorial harness executes.

## Recommended Next Step

Keep using Simics 7. The remaining cleanup is to reduce the PS/2 key-by-key command injection if a reliable UEFI shell stdin path is found, or to generate a startup script / disk image that launches the harness without synthetic keyboard input.

## Files Most Relevant To The Current State

- [Dockerfile](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/Dockerfile)
- [project/run.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/run.simics)
- [project/output.log](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/output.log)
