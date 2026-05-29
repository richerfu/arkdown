# Parity Report — Minimal HAP Fixture

Documents the field-by-field comparison between arkdown and hvigor golden output
for the `minimal-hap` integration fixture (API 12, single `EntryAbility`).

## module.json

### `module` section

| Field | hvigor golden | arkdown | Status |
|---|---|---|---|
| `name` | `"entry"` | `"entry"` | ✅ |
| `type` | `"entry"` | `"entry"` | ✅ |
| `packageName` | `"entry"` | `"entry"` | ✅ |
| `virtualMachine` | `"ark12.0.6.0"` | `"ark12.0.6.0"` (SDK-discovered) / `"ark12.0.1.0"` (fallback) | ✅ |
| `compileMode` | `"esmodule"` | `"esmodule"` | ✅ |
| `deviceTypes` | `["phone"]` | `["phone"]` | ✅ |
| `deliveryWithInstall` | `true` | `true` | ✅ |
| `installationFree` | `false` | `false` | ✅ |
| `pages` | `"$profile:main_pages"` | `"$profile:main_pages"` | ✅ |
| `mainElement` | `"EntryAbility"` | `"EntryAbility"` | ✅ |
| `dependencies` | `[]` | `[]` | ✅ |

### `module.abilities[0]` section

| Field | hvigor golden | arkdown | Status |
|---|---|---|---|
| `name` | `"EntryAbility"` | `"EntryAbility"` | ✅ |
| `srcEntry` | `"./ets/entryability/EntryAbility.ets"` | `"./ets/entryability/EntryAbility.ets"` | ✅ |
| `exported` | `true` | `true` | ✅ |
| `icon` | `"$media:icon"` | `"$media:icon"` | ✅ |
| `iconId` | `16777219` (`0x1000003`) | resolved from `resource_ids["icon"]` | ✅ |
| `label` | `"$string:EntryAbility_label"` | `"$string:EntryAbility_label"` | ✅ |
| `labelId` | `16777216` (`0x1000000`) | resolved from `resource_ids["EntryAbility_label"]` | ✅ |
| `description` | `"$string:EntryAbility_desc"` | `"$string:EntryAbility_desc"` | ✅ |
| `descriptionId` | `16777217` (`0x1000001`) | resolved from `resource_ids["EntryAbility_desc"]` | ✅ |
| `startWindowIcon` | `"$media:startIcon"` | `"$media:startIcon"` | ✅ |
| `startWindowIconId` | `16777220` (`0x1000004`) | resolved from `resource_ids["startIcon"]` | ✅ |
| `startWindowBackground` | `"$color:start_window_background"` | `"$color:start_window_background"` | ✅ |
| `startWindowBackgroundId` | `16777218` (`0x1000002`) | resolved from `resource_ids["start_window_background"]` | ✅ |

### `app` section

| Field | hvigor golden | arkdown | Status |
|---|---|---|---|
| `bundleName` | `"com.example.myapplication"` | from `AppScope/app.json5` | ✅ |
| `bundleType` | `"app"` | `"app"` | ✅ |
| `icon` | `"$media:app_icon"` | from `AppScope/app.json5` | ✅ |
| `iconId` | `16777219` | resolved from `resource_ids["app_icon"]` | ✅ |
| `label` | `"$string:app_name"` | from `AppScope/app.json5` | ✅ |
| `labelId` | `16777216` | resolved from `resource_ids["app_name"]` | ✅ |
| `versionCode` | `1000000` | from `AppScope/app.json5` | ✅ |
| `versionName` | `"1.0.0"` | from `AppScope/app.json5` | ✅ |
| `targetAPIVersion` | `50000012` | computed from `compatible_sdk_version` | ✅ |
| `minAPIVersion` | `50000012` | same as `targetAPIVersion` | ✅ |
| `targetPatchAPIVersion` | `0` | hardcoded `0` | ✅ |
| `targetMinorAPIVersion` | `0` | hardcoded `0` | ✅ |
| `buildMode` | `"debug"` | from `BuildMode` | ✅ |
| `debug` | `true` | from `BuildMode` | ✅ |
| `compileSdkType` | `"HarmonyOS"` | hardcoded | ✅ |
| `compileSdkVersion` | `"6.0.0.47"` | from `SdkConfig::compile_sdk_version` | ✅ |
| `apiReleaseType` | `"Release"` | hardcoded | ✅ |
| `appEnvironments` | `[]` | `[]` | ✅ |

## Implementation Notes

`virtualMachine` uses the pattern `ark<version>` where `<version>` is discovered from
the SDK's `es2abc` binary at `--target-api-sub-version Release`. The fallback (no SDK
available) uses `<compile_api_version>.0.1.0`, e.g. `ark12.0.1.0` for API 12.

Resource IDs (`iconId`, `labelId`, etc.) are resolved by `write_module_json` via the
`resource_ids: Option<&HashMap<String, u32>>` parameter. The caller passes the
`ResourceCompileOutput::resource_ids` map produced by `arkdown-resource`. ID values
start at `0x01000000` and increment monotonically in the order resources are scanned
(`string` elements first, then other elements, then `media`, then `profile`).

The golden values for a minimal-hap fixture with the standard DevEco Studio template:
- `app_name` (string) → `0x01000000` = 16777216
- `module_desc` (string) → `0x01000001` = 16777217
- `start_window_background` (color) → `0x01000002` = 16777218
- `icon` (media) → `0x01000003` = 16777219
- `startIcon` (media) → `0x01000004` = 16777220

These match hvigor's `restool` output for the same resource layout.

## HAP zip structure

The full 11-entry HAP zip matches hvigor golden exactly (see `hvigor-gap-plan.md`
Phase 6 for details on remaining gaps in `ets/modules.abc` and `ets/sourceMaps.map`).
