# Fuzzing a BMC (BIOS + OpenBMC)

- [Fuzzing a BMC (BIOS + OpenBMC)](#fuzzing-a-bmc-bios--openbmc)
  - [What This Covers](#what-this-covers)
  - [Getting Started](#getting-started)
  - [Structure](#structure)
  - [Status and Known Caveats](#status-and-known-caveats)

This tutorial is a feasibility example for fuzzing BMC firmware: a platform
BIOS and OpenBMC booting together in Simics, with a compiled-in TSFFS
harness on a real OpenBMC codebase (libpldm's firmware-update package
decoder). Unlike the other tutorials in this book, this one is documented
directly in its own `README.md` next to the code, since it is closer to a
runnable reference setup than a step-by-step walkthrough. The code lives at
[`examples/tutorials/bmc-fuzzing`](https://github.com/intel/tsffs/tree/main/examples/tutorials/bmc-fuzzing)
in the repository.

## What This Covers

- Building and booting a public EDK2 BIOS image (the X58I board from the
  [Fuzzing a Platform BIOS](../edk2-simics-platform-bios/README.md) tutorial)
  with its LogoFAIL harness.
- Building and booting real OpenBMC (`openbmc/openbmc`, machine
  `evb-ast2600`) to a login prompt on an ASPEED AST2600 Simics model,
  including two kernel-side fixes needed to make this OpenBMC build boot on
  that specific Simics target.
- Booting the BIOS and OpenBMC side by side in one Simics session (no
  interconnect wired between them yet).
- A **compiled-in**, bare-metal harness for libpldm's firmware-update
  package decoder, running directly on the AST2600's Cortex-A7 core without
  booting OpenBMC at all - see
  [Bare-Metal and Non-x86 Compiled-In Harnessing](../../harnessing/bare-metal.md)
  for the general technique.
- A first fuzzing campaign against that harness, with real crashes found.

## Getting Started

The [example's `README.md`](https://github.com/intel/tsffs/tree/main/examples/tutorials/bmc-fuzzing)
has the full setup and build/run instructions. In short:

```sh
cd examples/tutorials/bmc-fuzzing/bios-x58i && ./build.sh
cd ../openbmc-ast2600 && ./build.sh
cd harness-libpldm && ./build.sh   # needs gcc-arm-linux-gnueabihf
```

then, from a Simics project with the packages and AST2600 model described in
the example's README (the model itself is **not** distributed with TSFFS or
this example - see its "Model dependency" section):

```sh
./simics -no-gui --no-win \
    -e '$repo_root = "/absolute/path/to/tsffs/examples/tutorials/bmc-fuzzing"' \
    /path/to/scripts/combined-boot.simics
```

## Structure

- `bios-x58i/` - the X58I BIOS build and its LogoFAIL harness.
- `openbmc-ast2600/` - the OpenBMC build, plus `harness-libpldm/` (the
  libpldm firmware-update decoder harness).
- `scripts/` - Simics scripts to boot the BIOS and BMC together and to run
  the libpldm fuzzing campaign.
- `docs/libpldm-fuzzing.md` - a detailed write-up of the harnessing approach
  and campaign result.

## Status and Known Caveats

The BIOS<->BMC interconnect (LPC/KCS or eSPI, depending on platform) is not
wired up in this example - both boot independently in the same session, but
neither can currently drive input into the other. This is called out as
future work in the example's own status checklist.

The libpldm fuzzing campaign found real crashes, but at least one crash
signature did not reproduce standalone via `@tsffs.iface.fuzz.repro()` after
the campaign ended - see `docs/libpldm-fuzzing.md` for detail. Treat the
crash count as a lower bound pending that investigation, not yet as
individually triaged bugs.
