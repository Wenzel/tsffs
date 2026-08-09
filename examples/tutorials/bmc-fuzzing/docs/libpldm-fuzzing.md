# Fuzzing libpldm's firmware-update package decoder

## Target

`decode_pldm_firmware_update_package()` in
[`openbmc/libpldm`](https://github.com/openbmc/libpldm)'s
`src/dsp/firmware_update.c` - the PLDM Type 5 (Firmware Update, DSP0267)
package header decoder. This is the top candidate identified when shortlisting
fuzz targets on the BIOS↔BMC interaction surface: a pure
`decode(buf, len) -> struct` entry point, no grammar/auth required to reach
it, and a parsing bug here (e.g. in the component-image or descriptor
records) is close to an arbitrary-flash-write class of bug during a real
firmware update.

## Harnessing approach: compiled-in, not closed-box

Enabling PLDM in a full OpenBMC image (`DISTRO_FEATURES += "pldm"`) pulls in
`pldmd` as a live D-Bus daemon - harnessing it closed-box would mean booting
the full image, then finding and breaking at the daemon's actual socket-read
call site in a (likely stripped) binary. Instead, this harness links
libpldm's decoder source **directly** against a small standalone C program
and runs it bare-metal on the target AST2600 model's Cortex-A7 core - no
OpenBMC boot required at all. This is the same technique validated
internally as a smoke test on a synthetic target (see the BMC fuzzing task
hub note in the internal vault) applied here to real OpenBMC source instead
of a synthetic example.

### Why this works for this specific function

`decode_pldm_firmware_update_package()` and everything it calls
(`src/dsp/firmware_update.c`, `src/dsp/base.c`, `src/edac.c`, `src/utils.c`)
only need `memcpy`/`memcmp`/`memset`/`__assert_fail` from the C library -
verified by compiling with `-ffreestanding` and checking the undefined
symbols in the resulting object files. `harness-libpldm/src/harness_main.c`
supplies trivial standalone implementations of all four, so the decoder runs
correctly with zero OS/libc dependency.

`pldm_edac_crc32_validate` is a weak symbol in `src/edac.c`; the harness
overrides it to always return "checksum OK" (0), the same approach
libpldm's own `tests/fuzz/fuzz.c` takes - CRC32 correctness is irrelevant to
the parsing logic under test, and skipping it means the fuzzer doesn't waste
time defeating a checksum to reach the interesting code paths.

## Building

`harness-libpldm/build.sh`:
1. Clones `openbmc/libpldm` (if not already present) into `libpldm-src/`.
2. Hand-generates `include/libpldm/api.h` and `config.h` from their `.in`
   templates - normally Meson does this from `PACKAGECONFIG`/build options,
   selecting the "production" ABI (deprecated + stable symbols, no unstable
   ones), matching the default in `openbmc/openbmc`'s
   `meta-phosphor/recipes-phosphor/libpldm/libpldm_git.bb` recipe. Building
   outside Meson means generating the equivalent by hand.
3. Compiles the four needed `.c` files plus the harness with
   `arm-linux-gnueabihf-gcc -ffreestanding -fno-builtin`, targeting
   `-mcpu=cortex-a7`.
4. Links against `src/link.ld` (entry at `0x80000000`, the AST2600 BMC DDR
   base) into a single freestanding ELF, `pldm_fw_update_harness.elf`.

### Compiler flag gotcha: NEON/VFP codegen

The default `-mfloat-abi` for this cross-compiler is `hard`, which lets GCC
emit NEON instructions (`vmov.i32`, `vorr`, `vst1.8`) for plain integer
struct-zeroing (`struct pldm_package pkg = {0};`). The target Simics
Cortex-A7 core rejected these as Undefined Instruction on every single
execution - every fuzzing iteration crashed identically, immediately after
`HARNESS_START`, which was the first sign something was wrong with the
*harness* rather than the target. Fixed by adding
`-fno-tree-vectorize -fno-tree-slp-vectorize`, which stops GCC from
autovectorizing memory operations into NEON at `-O0`; this in turn required
a `memset` stub too (GCC emits a libcall instead once vectorization is
disabled).

### `tsffs-gcc-arm32.h` `HARNESS_START` fix

Same upstream bug already documented for the ARM32 smoke test: the stock
`tsffs/harness/tsffs-gcc-arm32.h` calls `__orr_extended2` (2 args) for
`HARNESS_START`, omitting the `DEFAULT_INDEX` argument every other
architecture's header passes (compare `tsffs-gcc-aarch64.h`, which correctly
uses `__orr_extended3` with the index first). Without the fix, the buffer
pointer lands in `r10` (read by TSFFS as a bogus harness index), and the
fuzzer never recognizes the start harness at all. `src/tsffs.h` here is a
patched copy with `HARNESS_START` corrected to pass `DEFAULT_INDEX`.

## Running

`scripts/fuzz-libpldm.simics`, run from a Simics project with the AST2600
model and TSFFS loaded (see the top-level README's "Model dependency" note):

```sh
./simics --batch-mode /path/to/scripts/fuzz-libpldm.simics
```

The script boots the standalone AST2600 target far enough to instantiate the
Cortex-A7 core and its memory map, stops before any real firmware would run,
loads `pldm_fw_update_harness.elf` directly into DDR, and points the core's
PC at it. ARM exceptions `[1, 4, 5]` (Data Abort, Prefetch Abort, Undefined
Instruction) are configured as TSFFS solutions.

## Result

A 10-minute campaign against a fresh random corpus found **20 distinct
crash signatures** (24 files including duplicate/repeat discoveries under
the same hash - see `solutions/` after a run), at a steady several-execs/sec
rate with continuous snapshot restore across iterations (no hangs, no
snapshot corruption). One signature (`d659a368aac8ef8e`) was independently
rediscovered 4 times with byte-identical content, a strong signal these are
real, deterministic crashes rather than fuzzer noise.

**Not yet done**: root-causing which specific parsing path in
`decode_pldm_firmware_update_package()` (or the code it calls, e.g. the
firmware/downstream-device-ID-record or component-image-information
iterators) each signature corresponds to. `@tsffs.iface.fuzz.repro(<path>)`
did not reproduce the crash for at least one signature tested manually
against a fresh checkpoint - this needs further investigation (possibly a
snapshot/repro state mismatch specific to this harness's global buffer
state) before triaging root cause. The crashes are confirmed real via the
fuzzer's own live classification during the campaign (`solutions/` is only
populated on an actual `Solution` stop event), just not yet reproduced
standalone after the fact.
