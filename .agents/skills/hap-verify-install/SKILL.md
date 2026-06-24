---
name: hap-verify-install
description: Mandatory HAP verification install workflow for arkdown/OpenHarmony device or emulator validation. Use whenever Codex installs, verifies, launches, smoke-tests, or debugs a generated .hap package, especially with hdc, simulator/emulator, bundleName, ability startup, runtime logs, or package replacement.
---

# HAP Verify Install

## Rule

All HAP verification packages must be installed by uninstalling the target
bundle first, then installing the newly generated HAP. Do this even when `hdc
install -r` is available.

## Workflow

1. Determine the bundle name from the source profile or generated package
   metadata. Prefer `AppScope/app.json5#app.bundleName`.
2. Confirm a device is connected with `hdc list targets`.
3. Run `hdc uninstall <bundleName>` before every verification install.
   Treat "not installed" as non-blocking; treat transport/device errors as
   blocking.
4. Run `hdc install -r <path-to-hap>`.
5. Launch the app or requested ability only after install succeeds.
6. If install fails due to device storage, uninstall the same bundle again and
   retry once before reporting the device-space failure.
