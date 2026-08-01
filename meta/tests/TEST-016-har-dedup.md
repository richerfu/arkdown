# TEST-016: arkdown HAR-dedup Rolldown adapter

**Crate**: `arkdown`
**Owner**: `crates/arkdown/src/plugins/har_dedup.rs`
**Original source**: `hvigor-arkts-compose/dist/src/plugins/har-deduplicate/har-deduplicate-util.js`,
`hsp-collect-har-files.js`

---

## Test File Layout

```
crates/arkdown/src/plugins/
├── har_dedup.rs              ← collect/real Rolldown passes
└── har_dedup_tests.rs        ← pass, exclusion, and target coverage
```

---

## collect_pass.rs

### TC-001: collect pass writes HAR source file paths to file

- **Setup**: HSP module graph containing modules from local HAR `my-har`
- **Expected**: `SOURCE_HAR_DUPLICATE_FILES_INFO` file contains paths of all `my-har` modules
- **Origin**: `hspCollectHarFilesPlugin` walks graph, writes `isHspCollectFilesCompile` list

### TC-002: collect pass writes ByteCode HAR references separately

- **Setup**: HSP module graph with ByteCode HAR reference
- **Expected**: `BYTECODE_HAR_DUPLICATE_FILES_INFO` file lists the bytecode HAR

### TC-003: collect pass does not affect output chunks

- **Expected**: `before_build_end` in `CollectPass` mode does not mark any modules external
  (it only writes to disk, does not modify the graph)

---

## dedup_pass.rs

### TC-004: real build excludes HAR files collected in pass 1

- **Setup**: pass 1 collected `["/path/to/my-har/src/main/ets/utils/Helper.ets"]`
- **Real build setup**: same module in HAP graph
- **Expected**: `Helper.ets` module is marked `external = true` in HAP's real build graph

### TC-005: modules NOT in the collected list remain internal

- **Setup**: HAR file `ComponentA.ets` NOT in collected list (not a dependency of the HSP)
- **Expected**: `ComponentA.ets` remains `external = false` in HAP graph

### TC-006: after dedup, the HAP bundle does not contain excluded HAR source

- **Setup**: two-pass build, HAR shared between HAP and HSP
- **Expected**: final HAP bundle JS does not contain code from the shared HAR's modules

---

## hsp_only.rs

### TC-007: plugin is no-op for HAP target

- **Setup**: `output_target = OutputTarget::Hap` (no HSP involvement)
- **Expected**: plugin never activates; no collect files written; no modules excluded

### TC-008: plugin is no-op for HAR target

- **Setup**: `output_target = OutputTarget::Har`
- **Expected**: no-op
