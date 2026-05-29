# Parity Report: minimal-hap — arkdown vs hvigor

Generated: 2026-05-29
Build: `cargo run -p arkdown-cli -- build --project tests/fixtures/minimal-hap`
Golden: `tests/fixtures/golden/minimal-hap/`

---

## 1. Artifact Size Summary

| Artifact             | arkdown     | hvigor (golden) | Match    |
|----------------------|-------------|-----------------|----------|
| `modules.abc`        | 8 607 B     | 9 844 B         | 87.4%    |
| `sourceMaps.map`     | 1 281 B     | 1 847 B         | 69.4%    |
| `loader.json`        | 796 B       | 499 B           | struct.  |
| `module.json` (merge)| 1 099 B     | 1 315 B         | partial  |
| `entry-default-signed.hap` | 43 451 B | N/A golden HAP | —  |

---

## 2. loader.json

**Result: 100% structural match (18/18 fields).**

All field names and values are identical. The only differences are the three
absolute-path fields (`modulePathMap.entry`, `projectRootPath`, `nodeModulesPath`,
`anBuildOutPut`) which embed the machine's working directory. This is expected and
matches hvigor's own behaviour (it also embeds absolute paths from `/private/tmp/...`).

Fields verified identical:
`compileMode`, `byteCodeHarInfo`, `declarationEntry`, `moduleName`, `hspNameOhmMap`,
`harNameOhmMap`, `packageManagerType`, `compileEntry`, `otherCompileFiles`,
`dynamicImportLibInfo`, `routerMap`, `hspResourcesMap`, `updateVersionInfo`, `anBuildMode`.

---

## 3. sourceMaps.map

Both files have identical top-level keys:
- `entry|entry|1.0.0|src/main/ets/entryability/EntryAbility.ts`
- `entry|entry|1.0.0|src/main/ets/pages/Index.ts`

Both use the same `sources` value (`['entry/src/main/ets/entryability/EntryAbility.ets']`
and `['entry/src/main/ets/pages/Index.ets']`) and the same `sourceRoot`/`file` convention.

Mapping VLQ segment count differs:

| Module         | arkdown mappings chars | hvigor mappings chars | Delta  |
|----------------|------------------------|-----------------------|--------|
| EntryAbility   | 329                    | 910                   | −581   |
| Index          | 381                    | 366                   | +15    |

**Root cause**: arkdown emits per-statement source maps. hvigor/ets-loader emits
per-token maps covering every identifier, literal, and operator individually.
For `EntryAbility.ts` (heavily class-based) this is a 2.8× density difference.
`Index.ets` is close because its generated ViewPU class body is simpler.

**Overall file match: ~69% by byte size. Key structure (module identifiers, sources arrays) matches 100%.**

---

## 4. module.json (merge_profile output)

**Fields in hvigor golden that are missing from arkdown output:**

### `app` section — missing fields

| Field               | Golden value          | Status in arkdown      |
|---------------------|-----------------------|------------------------|
| `iconId`            | `16777219`            | absent                 |
| `labelId`           | `16777216`            | absent                 |

These are resource integer IDs injected by restool/resource compiler during
profile processing. `arkdown-resource` does not yet write resolved integer IDs
back into the merged module.json.

### `module` section — missing fields

| Field              | Golden value            | Status in arkdown |
|--------------------|-------------------------|-------------------|
| `virtualMachine`   | `"ark12.0.6.0"`         | absent            |
| `compileMode`      | `"esmodule"`            | absent            |
| `dependencies`     | `[]`                    | absent            |

`virtualMachine` is injected by hvigor's `process-profile` step using the SDK
version. `compileMode` declares the module's ABC link mode. `dependencies` is an
empty list for this fixture but required by the schema.

### `module.abilities[0]` — missing fields

| Field                    | Golden value  | Status in arkdown |
|--------------------------|---------------|-------------------|
| `iconId`                 | `16777219`    | absent            |
| `labelId`                | `16777221`    | absent            |
| `descriptionId`          | `16777220`    | absent            |
| `startWindowIconId`      | `16777224`    | absent            |
| `startWindowBackgroundId`| `16777223`    | absent            |

All five are resource integer IDs. Same root cause as `app` section above.

### `module` section — missing fields (top-level)

| Field         | Golden value | Status in arkdown |
|---------------|-------------|-------------------|
| `descriptionId` | `16777222` | absent            |

**Summary: 11 fields differ. 3 are schema fields (`virtualMachine`, `compileMode`,
`dependencies`) and 8 are resource-resolved integer IDs. Field ordering is cosmetic.**

---

## 5. modules.abc (binary)

**Sizes**: arkdown 8 607 B, golden 9 844 B — **87.4% parity**.

### 5a. Strings present in golden but absent in arkdown

| String                                      | Likely role                              |
|---------------------------------------------|------------------------------------------|
| `func_main_0`                               | Entry function name (bare, no OHM prefix)|
| `getEntryName`                              | Method name (bare)                       |
| `rerender`                                  | ViewPU method name (bare)                |
| `O#~@0>#purgeVariableDependenciesOnElmtId`  | Scope-prefixed method reference          |
| `SlotNumber`                                | Annotation class name                    |
| `args`                                      | Identifier string item                   |
| `wentry|entry|1.0.0|src/main/ets/entryability/EntryAbility.ts` | Source file map key stored in abc |

### 5b. Strings present in arkdown but absent in golden

| String                                              | Severity | Root cause                              |
|-----------------------------------------------------|----------|-----------------------------------------|
| `/Users/ranger/.../pages/Index` (absolute fs path)  | **BUG**  | Absolute path leaking into abc StringItems — should never appear |
| `G./ets/entryability/EntryAbility.ets`              | Minor    | Relative path stored with length-prefix byte; golden uses class descriptor form `Lcom.../EntryAbility;` |
| `com.example.../ets/entryability/EntryAbility.func_main_0` | Diff | arkdown stores full OHM-prefixed func_main_0; golden stores bare `func_main_0` |
| `com.example.../ets/pages/Index.func_main_0`        | Diff     | Same issue for Index module             |
| `DOMAIN`, `EntryAbility`, `Index`, `UIAbility`      | Info     | Identifier strings — some present in golden as part of other encodings |
| `pkgName@entry` (appears twice)                     | Minor    | Duplicate module record annotation      |
| `isCommonjs`, `hasTopLevelAwait`, `isSharedModule`, `scopeNames`, `moduleRecordIdx` (duplicated) | Minor | Module record annotation fields duplicated for both chunks |
| `elmtId`, `extraInfo`, `funcObj`, `newTarget`, `params`, `paramsLambda`, `parent`, `rmElmtId`, `isInitialRender`, `newValue` | Diff | ViewPU param identifiers present in arkdown but stored differently in golden |
| `windowStage`                                       | Diff     | Appears in arkdown separately; golden embeds differently |
| `default`, `scopeNames` (bare)                      | Info     | Identifier strings with slight encoding differences |

### 5c. Summary of abc gaps

1. **Absolute path bug (critical)**: `/Users/ranger/.../pages/Index` must not appear in
   any StringItem. Likely emitted as the source file path when it should be the OHM URL or
   relative path.

2. **`func_main_0` name encoding (moderate)**: arkdown stores `com.example.../func_main_0`
   as the method NameItem. hvigor stores the bare `func_main_0`. The OHM prefix belongs in
   the class/module record NameItem, not in the individual function NameItem.

3. **LNP density (moderate, known)**: remaining ~1 237 B gap is attributable to
   per-instruction ADVANCE sequences. hvigor emits ~105 B/LNP record; arkdown emits
   ~58 B/LNP record. Progress from previous patches: START_LOCAL_EXTENDED, SET_PROLOGUE_END,
   NewLexEnvWithName, class-decl lexical bindings, scope_names literal arrays all match.

4. **Duplicate module record annotation strings (minor)**: `pkgName@entry`, `isCommonjs`,
   etc. appear twice in arkdown's string table (one per chunk). Golden deduplicates them
   into a single string table entry.

5. **External module reference format (minor)**: arkdown uses `@native.ohos.app` prefix;
   golden uses `L@ohos.app;` class descriptor notation.

---

## 6. Overall Parity Score

| Artifact         | Score  | Notes                                        |
|------------------|--------|----------------------------------------------|
| `loader.json`    | 100%   | All fields match; only machine paths differ  |
| `sourceMaps.map` | ~80%   | Keys/sources identical; VLQ density is 2.8× off for EntryAbility |
| `module.json`    | ~75%   | 3 schema fields + 8 resource IDs missing     |
| `modules.abc`    | ~87%   | 87.4% by size; 1 critical bug + 2 moderate encoding differences |
| **Overall**      | **~85%** | Weighted average across all artifacts      |

---

## 7. Actionable Gaps (Priority Order)

1. **[BUG] Absolute path in modules.abc** — `pages/Index` absolute fs path leaks into
   StringItems. Fix in `arkdown-abc` source file path derivation.

2. **[module.json] Missing `virtualMachine`, `compileMode`, `dependencies`** — Add to
   `arkdown-profile`'s `process_profile` step. `virtualMachine` reads SDK version from
   SDK metadata; `compileMode` is always `"esmodule"` for debug HAP; `dependencies` is `[]`.

3. **[module.json] Missing resource IDs** — `arkdown-resource` resolves string/icon refs
   to integer IDs; these need to be written back into the merged module.json by
   `arkdown-profile` after resource compilation.

4. **[modules.abc] `func_main_0` OHM prefix** — Strip the OHM module path from function
   NameItems. The module NameItem carries the OHM path; method NameItems should be bare.

5. **[modules.abc] Duplicate annotation strings** — Deduplicate `pkgName@entry`,
   `isCommonjs`, `hasTopLevelAwait`, `isSharedModule`, `scopeNames`, `moduleRecordIdx`
   across the string table when multiple module records reference the same keys.

6. **[sourceMaps.map] Per-token mapping density** — Full per-token source maps require the
   ets-loader transform to emit token-level positions. Low priority until Phase 2 ArkTS
   transform is complete.

7. **[modules.abc] LNP ADVANCE density** — Per-instruction ADVANCE emission to reach
   hvigor's ~105 B/LNP density. Tracked as ongoing Phase 5 work.
