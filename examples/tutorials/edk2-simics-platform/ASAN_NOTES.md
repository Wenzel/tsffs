# EDK2 Simics Platform ASAN Notes

## Future Investigations

### Internal Shell Boot Option Ordering

Investigate why the tutorial currently needs to insert the internal shell boot option at index `0`.

Current workaround:

- The Dockerfile changes `EfiBootManagerAddLoadOptionVariable (&NewOption, MAX_UINTN)` to `EfiBootManagerAddLoadOptionVariable (&NewOption, 0)` in `BoardBdsHookLib.c`.
- With `MAX_UINTN`, the internal shell option is appended to `BootOrder`.
- With `0`, the internal shell option is inserted at the front of `BootOrder`.
- In the current ASAN firmware plus Simics 7 setup, the shell boot path is reliable only with the index `0` behavior.

What remains unclear:

- Why `qsp.mb.simics_uefi->selected_boot_option = "Internal Shell"` is not sufficient when the shell option is appended.
- Whether Simics matches the option description but firmware `BootOrder` still wins.
- Whether `BootNext`, stale NVRAM variables, or existing variable-store state override the Simics-side selection.
- Whether the ASAN firmware build changes boot option numbering/order enough to expose a Simics target assumption.
- Whether the internal shell option is registered too late for Simics' selected boot option mechanism unless it is first in `BootOrder`.

Things to verify:

- Dump `BootOrder`, `BootNext`, and all `Boot####` variables before BDS starts selecting a boot option.
- Compare those variables with and without the index `0` patch.
- Check when Simics applies `selected_boot_option` relative to firmware boot option registration.
- Test whether clearing/pinning the variable store makes `selected_boot_option` work without patching `BoardBdsHookLib.c`.
- Look for a Simics-side way to select or reorder the internal shell option without modifying firmware source.

### Early `init-tsffs` Firmware Slowdown

Investigate why calling `init-tsffs` at the beginning of `project/run.simics` makes firmware initialization significantly slower.

Current hypothesis:

- `load-module tsffs` is mostly passive, but `init-tsffs` creates the TSFFS object.
- Creating the object registers global Simics HAP callbacks before the firmware reaches the shell.
- A recognized magic instruction or solution path during firmware boot may make TSFFS add the processor early.
- Adding the processor registers instruction before/after callbacks, so the rest of firmware boot may run under TSFFS instruction instrumentation.
- Even without many visible ASAN reports, stop/resume events and early instruction callbacks can explain the slowdown.

Things to verify:

- Add tracing around `Tsffs::add_processor()` to confirm whether it fires before `Tutorial.efi` starts.
- Log which HAP path first interacts with TSFFS during firmware boot.
- Compare boot time for:
  - `load-module tsffs` only
  - `load-module tsffs` plus `init-tsffs`
  - delayed `init-tsffs` after shell startup
- Check whether firmware/ASAN code emits TSFFS-recognized magic values before the harness executes.

### PS/2 Key-By-Key Shell Input

Investigate why the current launch path types shell commands key by key through the PS/2 keyboard instead of sending whole lines through:

~~~
qsp.serconsole.con.input "Tutorial.efi\n"
~~~

Current hypothesis:

- The UEFI shell prompt is reading from `gST->ConIn` via `WaitForKey` / `ReadKeyStroke`.
- In this firmware configuration, serial console output works, but serial console input is not necessarily connected to the UEFI shell's active `ConIn`.
- PS/2 keyboard events are reaching `ConIn`, which is why key-by-key injection is reliable.

Things to verify:

- Inspect the UEFI console input handles and device paths once the shell starts.
- Determine whether the serial console input device is registered with the console splitter.
- Test whether changing console variables or target configuration can make `qsp.serconsole.con.input` feed `ConIn`.
- Try a minimal shell `startup.nsh` path to avoid interactive input entirely.
- Evaluate whether Simics agent download plus shell launch can be done through a cleaner file-staging mechanism.

### Reducing `run.simics` Modifications

Investigate whether `project/run.simics` can be simplified.

Current concern:

- The script currently waits on debug strings such as `TSFFS StdinTrace` and `TSFFS ShellTrace`.
- Those strings come from firmware debug output that we intentionally inserted while debugging shell progress and input handling.
- That means the runtime script is coupled to instrumentation that may not belong in the final tutorial flow.

Things to verify:

- Find a stable Simics-side signal for shell readiness that does not depend on inserted firmware debug output.
- Check whether the shell prompt text can be matched reliably without custom firmware traces.
- Replace per-character PS/2 commands with a reusable helper if PS/2 remains necessary.
- Prefer a non-interactive launch path if possible:
  - pre-populated disk image with `startup.nsh`
  - NVRAM boot option that directly launches the harness
  - Simics agent file transfer followed by a shorter shell trigger
- Decide whether the debug trace insertions in the Dockerfile should remain, be gated behind a build flag, or move into a temporary debugging patch.
