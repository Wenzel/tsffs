#!/bin/bash
# Copyright (C) 2024 Intel Corporation
# SPDX-License-Identifier: Apache-2.0

# Like run-step.sh, but boots a fresh Simics for every seed, JOBS at a time.
# Needed for the sanitizer steps: replaying the whole directory in one session
# wedged after a few seeds there (every later seed faulted before the harness
# ran), so the verdicts after that point meant nothing.
#   ./run-isolated.sh 5 [JOBS]
set -e
STEP=${1:?usage: ./run-isolated.sh <step> [jobs]}
JOBS=${2:-16}
TUTORIAL=${TUTORIAL:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}

rm -rf "seeds-step${STEP}" "iso-step${STEP}"
./simics --no-win --batch-mode -e "\$step=\"${STEP}\"" boot.simics >/dev/null
python3 "${TUTORIAL}/gen_seeds.py" "seeds-step${STEP}" \
    --boot-log "boot-step${STEP}.log" --symbols "images/step${STEP}.DvshSmi.debug"

# one directory per seed, one Simics per directory
for seed in "seeds-step${STEP}"/*; do
    mkdir -p "iso-step${STEP}/$(basename "${seed}")"
    cp "${seed}" "iso-step${STEP}/$(basename "${seed}")/"
done
ls -d "iso-step${STEP}"/* | xargs -P "${JOBS}" -I{} sh -c \
    './simics --no-win --batch-mode -e "\$step=\"'"${STEP}"'\"" -e "\$seeds=\"{}\"" replay.simics > "{}.log" 2>&1'

cat "iso-step${STEP}"/*.log >"replay-step${STEP}.log"
python3 "${TUTORIAL}/report.py" "replay-step${STEP}.log" | tee "report-step${STEP}.txt"
