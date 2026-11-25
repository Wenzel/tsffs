#!/usr/bin/env python3
# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0
"""Turn the `Replayed <seed>: <stop reason>` lines of a Simics log into one
line per seed: caught (and by what) or survived.

An assert stop has three possible sources, told apart from the serial output
printed since the previous seed finished:

  asan <class>     an AddressSanitizer report (`bug_descr=<class>`)
  ubsan <check>    a UBSan handler (`__ubsan_handle_<check>`)
  assert           an EDK2 ASSERT (CpuDeadLoop/CpuBreakpoint)

usage: report.py SIMICS_LOG
"""
import re
import sys

EXC = {6: "#UD", 13: "#GP", 14: "#PF"}
CONSOLE = re.compile(r"^<[\w.\[\]]+serconsole[\w.]*>")

rows = []
serial = []  # serial lines since the previous "Replayed"
for line in open(sys.argv[1], errors="replace"):
    line = line.rstrip("\r\n")
    m = re.search(r"Replayed (\S+): (.*)", line)
    if not m:
        if CONSOLE.match(line):
            serial.append(CONSOLE.sub("", line).replace("\\r\\n", ""))
        continue

    seed, reason = m.group(1).split("/")[-1], m.group(2)
    text = "\n".join(serial)
    serial = []
    # The log of a fresh boot per seed carries the whole boot before the seed;
    # attribute only what happened once the handler was entered.
    text = text[text.rfind("[DVSH] >>> ENTER"):] if "[DVSH] >>> ENTER" in text else text

    e = re.search(r'"Exception":\{"number":(\d+)', reason)
    if e:
        verdict = f"caught {EXC.get(int(e.group(1)), 'exception ' + e.group(1))}"
    elif "StopAssert" in reason:
        asan = re.search(r"bug_descr=([\w-]+)", text)
        ubsan = re.search(r"__ubsan_handle_(\w+)", text)
        if asan:
            verdict = f"caught asan {asan.group(1)}"
        elif ubsan:
            verdict = f"caught ubsan {ubsan.group(1)}"
        else:
            verdict = "caught assert"
    elif "Timeout" in reason:
        verdict = "caught timeout"
    elif "Solution" in reason:
        verdict = "caught " + reason
    else:
        verdict = "survived"
    rows.append((seed, verdict))

for seed, verdict in rows:
    print(f"{seed:42s} {verdict}")
print(f"{sum(v.startswith('caught') for _, v in rows)} of {len(rows)} caught")
