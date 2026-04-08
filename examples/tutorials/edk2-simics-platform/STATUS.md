# EDK2 Simics ASAN Status

Date: 2026-04-07

## Goal

Boot the custom `BOARDX58ICH10_CUSTOM.fd` firmware with the packaged `qsp-x86/uefi-shell`
target, keep `minimal_boot_disk.craff` mounted so `SimicsAgent.efi` is available on
`FS0:`, and reliably reach the UEFI shell to launch `Tutorial.efi`.

## Current Simics Script

Current file: [project/fuzz.simics](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/fuzz.simics)

Important state:

- Uses `load-target "qsp-x86/uefi-shell"`
- Overrides BIOS with `BOARDX58ICH10_CUSTOM.fd`
- Mounts `minimal_boot_disk.craff` as `disk0`
- Tries the packaged shell hint:
  - `@conf.qsp.mb.simics_uefi.selected_boot_option = "Internal Shell"`
- Then falls back to keyboard automation:
  - wait for `End Load Options Dumping`
  - press `ESC`
  - wait for `Boot Manager Menu`
  - wait `5.0` seconds
  - press `KP_DOWN` eight times
  - press `ENTER`
  - wait for `Shell>`

## Findings

### 1. Packaged shell selection does not work with this custom BIOS

The packaged Simics preset normally selects the shell through the Simics metadata
device, not with keyboard input.

Relevant packaged file:
- `/home/wenzel/simics/simics-qsp-x86-7.48.0/targets/qsp-x86/uefi-shell.target.yml.include`

Key line:

```simics
$system.mb.simics_uefi->selected_boot_option = "Internal Shell"
```

Result with custom BIOS:

- The custom BIOS still boots with:
  - `Boot0000: UiApp`
  - `Boot0008: EFI Internal Shell`
- It ignores the `selected_boot_option = "Internal Shell"` hint.

Evidence:
- [project/output.log](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/output.log)

### 2. `minimal_boot_disk.craff` is being mounted and discovered

This image is not the primary boot path in the current tutorial flow. It is used as the
filesystem that contains `SimicsAgent.efi`.

Evidence from log:

- `Installed Fat filesystem on ...`
- `FSOpen: Open 'NvVars' Success`

This is consistent with:
- [docs/src/tutorials/edk2-uefi/testing-the-application.md](/home/wenzel/Projets/tsffs/docs/src/tutorials/edk2-uefi/testing-the-application.md)

### 3. Keyboard hotkey delivery is working

This is the most important recent progress.

With the keyboard-driven script, the log shows:

- `[Bds]BmHotkeyCallback: 0017:0000`
- `[Bds]Hotkey for Boot0000 pressed - Success`
- `[Bds]Exit the waiting!`
- `[Bds] Booting Boot Manager Menu.`

So:

- `ESC` is successfully reaching BDS
- the system does leave the countdown
- the remaining problem is only selecting `EFI Internal Shell` inside the menu

### 4. Previous menu navigation selected the wrong thing

A previous script version used a longer sequence after `ESC`, and the result was:

- `[Bds]Booting UiApp`

That means:

- the Boot Manager menu was opened
- the follow-up navigation selected `UiApp`, not `EFI Internal Shell`

### 5. Firmware-side timing is very slow

The custom BIOS is `DEBUG_CLANGSAN`, sanitizer-heavy, and boots much slower than the
stock packaged firmware, especially without VMP.

This means all menu automation must tolerate long delays.

### 6. The custom BIOS performs a DXE cold reset before BDS

Recent runs show the machine reaches DXE, triggers repeated sanitizer reports from
`HiiDatabaseDxe`, then executes:

- `DXE ResetSystem2: ResetType Cold`
- `ResetCold_CF9`

After that, a second boot cycle continues and does eventually reach BDS.

This means any shell-entry automation must survive one full firmware reset before the
boot menu phase is observable.

## ASAN / TSFFS Notes

- The BIOS patch in `edk2-platforms.patch` is currently disabled in the build path by
  [Dockerfile-custom](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/Dockerfile-custom).
- The current EDK2 `simics-sanitizer` branch already contains TSFFS wiring in
  `MdeModulePkg/Library/AsanLib/Asan.c`.
- Serial ASAN reports are visible in logs.
- We have seen repeated pre-harness UBSAN/ASAN-style reports from:
  - `/workspace/edk2/MdeModulePkg/Universal/HiiDatabaseDxe/Database.c`

## Next Experiments

1. Keep the current `ESC` hotkey entry path, because it clearly works.
2. Adjust only the in-menu keys after `Booting Boot Manager Menu`.
3. Try the minimal shell selection sequence that matches a wrapped last-item selection:
   - `ESC`
   - `KP_UP`
   - `ENTER`
4. If that still lands on `UiApp`, try:
   - `ESC`
   - longer wait
   - direct `DOWN` traversal to the internal shell entry
   - `ENTER`
5. If needed, try repeating the selection key before `ENTER`:
   - `KP_UP`, `KP_UP`, `ENTER`
6. If menu text is only visible in the graphics console, inspect whether the same
   interaction should be driven through a different input device or console object.
7. Keep in mind that the current custom firmware reaches BDS only after one DXE cold
   reset, so each experiment must wait through that full first boot.

## Experiment Log

### 2026-04-07: `ESC -> KP_UP -> ENTER`

Result:

- The Boot Manager menu still opens correctly.
- The follow-up `KP_UP -> ENTER` selection still ends at `UiApp`.

Evidence from [project/output.log](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/output.log):

- `[Bds]BmHotkeyCallback: 0017:0000`
- `[Bds]Hotkey for Boot0000 pressed - Success`
- `[Bds] Booting Boot Manager Menu.`
- `[Bds]Booting UiApp`

Conclusion:

- `ESC` is confirmed working.
- `KP_UP` does not wrap to `EFI Internal Shell`.
- The next best experiment is explicit downward traversal after the menu opens.

### 2026-04-07: exact-label metadata selection still ignored

Test:

- set `@conf.qsp.mb.simics_uefi.selected_boot_option = "EFI Internal Shell"`
- remove all keyboard automation
- wait only for `Shell>`

Result:

- the firmware still performs a DXE cold reset first
- after the second boot reaches BDS, the boot order is unchanged:
  - `Boot0000: UiApp`
  - `Boot0008: EFI Internal Shell`
- no `Shell>` prompt appears automatically

Conclusion:

- the packaged `simics_uefi` boot-selection hint is definitely ignored by this custom BIOS
- the remaining viable path is keyboard-driven Boot Manager navigation after the second-boot
  `End Load Options Dumping`

### 2026-04-07: first shell-selection sequence that stops choosing `UiApp`

Test:

- wait for second-boot `End Load Options Dumping`
- `ESC`
- wait for `Boot Manager Menu`
- wait `5.0` seconds
- `KP_DOWN` x8
- `ENTER`

Result:

- this is the first sequence that stops landing on `UiApp`
- the firmware reports:
  - `[Bds]Booting EFI Internal Shell`

Remaining issue:

- the serial log still does not reliably show a final `Shell>` prompt afterward
- so the menu-selection problem appears solved, but the shell handoff is still not fully
  usable from the current script

### 2026-04-07: local retest of `KP_DOWN` x8 is not stable yet

Test:

- keep the `ESC -> wait Boot Manager Menu -> wait 5s -> KP_DOWN x8 -> ENTER` sequence
- rerun with the exact packaged target and custom BIOS in the main workspace

Result in the main workspace:

- the machine still opens Boot Manager:
  - `[Bds]BmHotkeyCallback: 0017:0000`
  - `[Bds] Booting Boot Manager Menu.`
- but the current local rerun lands on:
  - `[Bds]Booting UiApp`

Conclusion:

- the promising shell-selection sequence is not stable enough yet in the main workspace
- the shell is still not reached reliably

### 2026-04-07: disabling `tsffs.stop_on_harness` during shell bring-up did not fix selection

Test:

- set `@tsffs.stop_on_harness = False` during firmware and shell bring-up
- re-enable it only right before `Tutorial.efi`

Result:

- repeated firmware sanitizer messages still occur
- Boot Manager still opens
- the current local run still selects `UiApp`

Conclusion:

- TSFFS pause behavior is not the only cause of the shell-selection instability

### 2026-04-07: `ESC -> wait for Boot Manager Menu -> DOWN x2 -> ENTER -> UP -> ENTER -> ENTER`

Result:

- The exact compatibility-style navigation still does not reach the shell.
- The firmware again falls through to `UiApp`.
- `UiApp` then cold-resets the machine, leading to another boot cycle.

Evidence from [project/output.log](/home/wenzel/Projets/tsffs/examples/tutorials/edk2-simics-platform/project/output.log):

- `[Bds] Booting Boot Manager Menu.`
- `[Bds]Booting UiApp`
- `DXE ResetSystem2: ResetType Cold, Call Depth = 1.`

Conclusion:

- This BIOS/preset combination is not following the submenu path assumed by the generic compatibility script.
- The next experiment should treat the Boot Manager as a direct single list and traverse straight to `EFI Internal Shell`.

### 2026-04-07: `ESC -> wait for Boot Manager Menu -> settle 5s -> KP_DOWN x8 -> ENTER`

Result:

- This is the first sequence that selected the shell entry instead of `UiApp`.
- The log shows `[Bds]Booting EFI Internal Shell`.
- I have not yet seen a stable `Shell>` prompt in `output.log`, so the selection problem is solved but the full shell handoff still needs one more step of cleanup or waiting.

Evidence:

- `[Bds]Booting EFI Internal Shell`

Conclusion:

- The packaged `qsp-x86/uefi-shell` target can be combined with the custom BIOS, but only by scripted menu navigation.
- The reliable selection sequence is:
  - wait for `End Load Options Dumping`
  - press `ESC`
  - wait for `Boot Manager Menu`
  - wait 5 seconds
  - press `KP_DOWN` eight times
  - press `ENTER`
- The remaining issue is post-selection shell bring-up, not menu selection.

## Constraints

- Avoid firmware changes unless necessary.
- Prefer Simics scripting and packaged targets.
- Keep `qsp-x86/uefi-shell`.
- Keep `minimal_boot_disk.craff`.
