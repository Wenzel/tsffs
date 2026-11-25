#!/usr/bin/env python3
# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0
"""Turn the `Replayed <seed>: <stop reason>` lines of a Simics log into one
line per seed: caught (and how) or survived.

usage: report.py SIMICS_LOG
"""
import re
import sys

EXC = {6: "#UD", 13: "#GP", 14: "#PF"}
rows = []
for line in open(sys.argv[1], errors="replace"):
    m = re.search(r"Replayed (\S+): (.*)", line)
    if not m:
        continue
    seed, reason = m.group(1).split("/")[-1], m.group(2)
    e = re.search(r'"Exception":\{"number":(\d+)', reason)
    if e:
        verdict = f"caught {EXC.get(int(e.group(1)), 'exception ' + e.group(1))}"
    elif "StopAssert" in reason:
        verdict = "caught ASSERT"
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
