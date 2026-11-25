#!/usr/bin/env python3
# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0
"""Craft one DVSH_SMI_REQUEST per DVSH command, with the arguments that make
the bug trigger.

usage: gen_seeds.py SEED_DIR [--boot-log LOG --symbols DvshSmi.debug]

smram-write needs the address of a writable byte inside SMRAM. DvshSmi loads
into SMRAM at an address the firmware prints at boot ("Loading SMM driver at
0x... DvshSmi.efi"); add the offset of `mDvshGlobalBuffer` from the module's
.debug file to get it. Without --boot-log/--symbols that seed aims at address
0 instead, which is outside SMRAM.
"""
import argparse
import re
import struct
import subprocess
from pathlib import Path

HEADER = Path(__file__).parent / "DamnVulnerableSmmPkg/Include/Guid/DvshSmiCommunication.h"

# Fields: Address, Value, Size, Index. Everything else is zero. The `*Oob*`
# grids and the UBSan "validated" commands (0x19-0x1E) need nothing: they
# trigger on the command alone.
ARGS = {
    "RawReadMem": dict(Size=16),  # address 0, outside SMRAM
    "RawWriteMem": dict(Size=16),
    "SmramWrite": dict(Size=1),  # Address filled in below
    "SignedIntOverflow": dict(Value=1),  # 1 + INT32_MAX
    "ShiftOutOfBounds": dict(Size=32),  # 1u << 32
    "ArrayIndexOutOfBounds": dict(Index=16),  # one past a 16-byte array
    "StackCanarySmash": dict(Value=4),
}


def smram_address(boot_log, symbols):
    base = re.search(r"Loading SMM driver at (0x[0-9A-Fa-f]+) EntryPoint=\S+ DvshSmi\.efi", Path(boot_log).read_text(errors="replace"))
    sym = re.search(r"^([0-9a-f]+) \w mDvshGlobalBuffer$", subprocess.run(["nm", symbols], capture_output=True, text=True, check=True).stdout, re.M)
    if not (base and sym):
        raise SystemExit("could not find DvshSmi load address or mDvshGlobalBuffer")
    return int(base.group(1), 16) + int(sym.group(1), 16)


ap = argparse.ArgumentParser()
ap.add_argument("seed_dir", type=Path)
ap.add_argument("--boot-log")
ap.add_argument("--symbols")
opts = ap.parse_args()

if opts.boot_log and opts.symbols:
    ARGS["SmramWrite"]["Address"] = smram_address(opts.boot_log, opts.symbols)

opts.seed_dir.mkdir(parents=True, exist_ok=True)
for name, value in re.findall(r"DvshCmd(\w+)\s*=\s*(0x[0-9A-Fa-f]+)", HEADER.read_text()):
    if name.startswith("Reserved"):  # id the handler leaves unhandled on purpose
        continue
    a = ARGS.get(name, {})
    # <IIQQII> = Command, Status, Address, Value, Size, Index; then Data[256]
    (opts.seed_dir / f"{int(value, 16):02x}-{name}").write_bytes(
        struct.pack("<IIQQII", int(value, 16), 0, a.get("Address", 0), a.get("Value", 0), a.get("Size", 0), a.get("Index", 0)) + bytes(256)
    )
