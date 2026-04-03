# ASAN-Enabled QSP BIOS in the TSFFS `edk2-simics-platform` Example

## Goal

Boot an ASAN-enabled UEFI firmware image inside the existing TSFFS Simics QSP platform tutorial, without changing the tutorial into a different workflow.

The target example is:

- [examples/tutorials/edk2-simics-platform](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform)

The desired end state is:

1. Build a custom `BoardX58Ich10` BIOS image with ASAN enabled.
2. Boot that BIOS with the tutorial's existing custom QSP target.
3. Keep the tutorial's `Logo.c` TSFFS harness patch active so TSFFS still fuzzes the boot logo path.

## Main Conclusion

The existing `edk2-uefi` tutorial is the wrong base for this work.

Why:

- It builds a UEFI application, not a board firmware image.
- ASAN here is not just an application build flag. The firmware itself must be ASAN-aware.
- The Simics/QSP platform needs platform-side setup for ASAN shadow memory before runtime checks can work.

The correct base is the BIOS tutorial/example:

- [examples/tutorials/edk2-simics-platform](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform)

## What "ASAN-aware firmware" means

Booting an instrumented module is not enough.

For this setup, the firmware image must include:

- `CLANGSAN` toolchain support in `edk2`
- the ASAN runtime libraries and allocator hooks in `edk2`
- platform-side PEI logic that allocates shadow memory and publishes the `ASAN_INFO` HOB

Without that HOB, the ASAN runtime disables itself at boot.

## BIOS vs SPI image

For the QSP target, these are separate:

- `bios`: the main firmware image mapped and executed by the platform
- `spi_flash`: the emulated SPI flash backing image attached to the chipset

For the BIOS tutorial flow, overriding the BIOS image is enough.

That is also what FuzzUEr does: it boots a custom `BOARDX58ICH10.fd` while leaving the default SPI flash image alone.

So for this task:

- we only needed to replace the BIOS image
- we did not need a custom SPI flash image

## Public branches that already contain the ASAN work

The required ASAN work is already public.

Use:

- `https://github.com/cglosner/edk2.git`, branch `simics-sanitizer`
- `https://github.com/cglosner/edk2-platforms.git`, branch `simics-sanitizer`

This matters because the `edk2-platforms` side is also patched for ASAN.
It is not enough to use only an ASAN-enabled `edk2` tree.

The platform-side changes include the Simics board PEI logic that allocates shadow memory and publishes the ASAN HOB.

## What we explicitly did not do

We did not add `HARNESS_ASSERT()` inside `Asan.c`.

Why:

- It is not required to build or boot the ASAN firmware.
- It is only needed if ASAN findings should be treated as TSFFS solutions/assertions.

For now, the goal was only:

- build the ASAN firmware
- boot it
- keep the existing tutorial harness on `Logo.c`

## Existing TSFFS harness patch still needed

Switching to public ASAN branches does not replace the tutorial's own TSFFS patching.

The local patch file:

- [edk2-platforms.patch](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/edk2-platforms.patch)

is still required because it turns the boot logo parser into a TSFFS harness target by patching:

- `SimicsOpenBoardPkg/Library/DxeLogoLib/Logo.c`

That patch adds:

- `#include "tsffs.h"`
- `HARNESS_START(...)`
- `HARNESS_STOP()`
- the existing assert-based tutorial behavior

So:

- public `cglosner/*` branches provide ASAN-capable firmware/platform
- local `edk2-platforms.patch` keeps the tutorial's boot logo fuzz harness

## Working file changes in the TSFFS example

The working path was implemented in:

- [build-custom.sh](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/build-custom.sh)
- [Dockerfile-custom](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/Dockerfile-custom)
- [project/run-custom.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/run-custom.simics)
- [project/fuzz.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/fuzz.simics)
- [project/fuzz2.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/fuzz2.simics)
- [project/targets/qsp-x86/qsp-uefi-custom.target.yml](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/targets/qsp-x86/qsp-uefi-custom.target.yml)
- [project/.package-list](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/.package-list)

## Build process that worked

### 1. Use the BIOS tutorial/example, not the UEFI app tutorial

Work in:

- [examples/tutorials/edk2-simics-platform](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform)

### 2. Build from public ASAN branches

In [build-custom.sh](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/build-custom.sh):

- clone `cglosner/edk2` `simics-sanitizer`
- clone `cglosner/edk2-platforms` `simics-sanitizer`
- keep `edk2-non-osi` and `FSP` from public upstream pins

### 3. Keep injecting the tutorial-local `tsffs.h`

The `Logo.c` patch is local to the tutorial, so the build still copies:

- `harness/tsffs.h`

into the Docker build context.

### 4. Install LLVM tools in the custom container

The `CLANGSAN` toolchain required `clang` and LLVM tooling that were not present in the stock container.

This was needed in:

- [Dockerfile-custom](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/Dockerfile-custom)

The symptom before this fix was:

- `make: *** ... Asan.obj] Error 127`

which indicated the build command could not execute the required tool.

### 5. Build with `CLANGSAN`

In the custom Dockerfile, this worked:

```sh
python build_bios.py -p BoardX58Ich10 -d -t CLANGSAN
```

Important detail:

- on the public `cglosner/edk2-platforms` branch, the board selector is `BoardX58Ich10`
- `BoardX58Ich10X64` was rejected by `build_bios.py`

### 6. Copy out the built BIOS artifact

The resulting BIOS image was copied from:

- `Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_CLANGSAN/FV/BOARDX58ICH10.fd`

into the project-local target images directory as:

- `project/targets/qsp-x86/images/BOARDX58ICH10_CUSTOM.fd`

## Simics project setup that worked

### 1. Create the project with the required packages

The project was initialized under:

- [project](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project)

using `ispm`.

Because `ispm` was not on `PATH`, the local binary was used:

```sh
/home/wenzel/simics/ispm/ispm
```

### 2. Add the missing crypto-engine package

The first Simics boot failed with:

- `No class crypto_engine_aes found`

That meant the project package set was incomplete.

The fix was to add:

- `../simics-crypto-engine-7.15.0`

to [project/.package-list](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/.package-list)

and rerun:

```sh
./bin/project-setup --force
```

### 3. Fix the custom target default BIOS path

The custom target YAML still defaulted to the old workspace `DEBUG_GCC` BIOS path, which caused `load-target` validation failures before the script override mattered.

The fix was to change the default BIOS in:

- [project/targets/qsp-x86/qsp-uefi-custom.target.yml](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/targets/qsp-x86/qsp-uefi-custom.target.yml)

to:

- `%simics%/targets/qsp-x86/images/BOARDX58ICH10_CUSTOM.fd`

## Runtime scripts aligned to the ASAN BIOS

The following scripts were updated to use the copied custom BIOS image:

- [project/run-custom.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/run-custom.simics)
- [project/fuzz.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/fuzz.simics)
- [project/fuzz2.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/fuzz2.simics)

They now use:

- `%simics%/targets/qsp-x86/images/BOARDX58ICH10_CUSTOM.fd`

and use:

- `@tsffs.exceptions = [12, 13, 14]`

## What we observed after boot

Booting the ASAN firmware with the tutorial harness succeeded far enough for TSFFS to see the harness start:

- TSFFS reported the `StartBufferPtrSizePtr` magic instruction

This is the key confirmation that:

1. the ASAN-enabled firmware image booted
2. the tutorial's `Logo.c` harness patch was still active
3. TSFFS reached the harness entry point

## Why fuzzing still reported "No interesting cases found"

TSFFS started, but then printed:

- `No interesting cases found from inputs!`

At that point the most likely issue was not firmware boot anymore.

The likely cause was missing or insufficient corpus data for the logo parser.

The project currently did not contain a populated:

- `project/corpus/`

for the BIOS logo target.

So the current state is:

- ASAN BIOS build: working
- Simics project setup: working
- Simics boot of ASAN BIOS: working
- TSFFS reaches `HARNESS_START()`: working
- productive fuzzing of the logo parser: still needs corpus/setup follow-up

## Minimal command sequence

### Build the ASAN BIOS

From:

- [examples/tutorials/edk2-simics-platform](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform)

run:

```sh
./build-custom.sh
```

### Initialize or update the Simics project

From the same directory:

```sh
/home/wenzel/simics/ispm/ispm projects project --create 1000-latest 1030-latest 2096-latest 8112-latest 31337-latest --ignore-existing-files --install-dir /home/wenzel/simics
```

If the project already exists and `.package-list` changed:

```sh
cd project
./bin/project-setup --force
```

### Boot the custom ASAN BIOS

```sh
cd /home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project
./simics -no-gui --no-win ./run-custom.simics
```

### Attempt fuzzing

```sh
./simics -no-gui --no-win ./fuzz.simics
```

## Key learnings

- The UEFI app tutorial was the wrong starting point because it does not produce firmware.
- The BIOS tutorial structure was already the correct shape for this work.
- ASAN required both `edk2` and `edk2-platforms` sanitizer branches.
- The public `cglosner/*` repositories already contain the needed ASAN firmware/platform work.
- The tutorial-local `Logo.c` patch must still be applied because it is the TSFFS harness.
- `CLANGSAN` required installing LLVM tooling in the container.
- The public board build uses `BoardX58Ich10`, not `BoardX58Ich10X64`.
- Simics project package composition matters; QSP needed the crypto-engine addon.
- Reaching TSFFS `HARNESS_START()` confirmed that the ASAN BIOS and tutorial harness were both live.

## Not yet addressed

- Turning ASAN violations into explicit TSFFS `HARNESS_ASSERT()` solutions
- curating an effective initial corpus for the logo parser
- deciding whether the UEFI tracker should be given an explicit map file from the copied custom build artifacts
