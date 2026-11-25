#!/bin/bash
# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0

# Boot one step's image, craft the seeds for it, replay them and print the verdicts.
# Run from the Simics project directory:  ./run-step.sh 2
set -e
STEP=${1:?usage: ./run-step.sh <1|2|2u|3>}
TUTORIAL=${TUTORIAL:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}

rm -rf "seeds-step${STEP}"
./simics --no-win --batch-mode -e "\$step=\"${STEP}\"" boot.simics >/dev/null
python3 "${TUTORIAL}/gen_seeds.py" "seeds-step${STEP}" \
    --boot-log "boot-step${STEP}.log" --symbols "images/step${STEP}.DvshSmi.debug"
./simics --no-win --batch-mode -e "\$step=\"${STEP}\"" replay.simics >"replay-step${STEP}.log" 2>&1
python3 "${TUTORIAL}/report.py" "replay-step${STEP}.log" | tee "report-step${STEP}.txt"
