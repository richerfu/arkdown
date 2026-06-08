# crates/arkdown-resource

Action: Keep.

## 目标职责

`arkdown-resource` 负责 resource inventory 的编译和 restool 输入输出：生成 restool config、执行 compile/link、处理 `resources.index`、`ResourceTable.txt`、compiled resources、rawfile/resource profile。它消费 `ResourceCompilePlan`，不发现项目。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| ProcessResource | `src/tasks/process-resource.js`, `abstract/abstract-process-resource.js` | `plan.rs`, `profile.rs`, config writer |
| CompileResource | `src/tasks/compile-resource.js`, `abstract/abstract-compile-resource.js` | `compiler.rs` |
| restool command/config builder | `src/builder/restool-command-builder.js`, `restool-file-config-builder.js` | `command.rs`, `layout.rs` |
| resource table/index | restool outputs | `table.rs`, `index.rs` |
| ids/defined ids | restool `--ids`, `--defined-ids` | `ids.rs` |
| media compression | `src/builder/opt-compression-builder.js` | `media.rs` |
| HAR resource dependencies | `CompileResourcesUtil`, `har-target-util` | `dependency_resources.rs` |
| ProcessCompiledResources | `src/tasks/process-compiled-resources.js` | `compiled.rs` |

## 目标目录

```text
src/
  lib.rs
  error.rs
  plan.rs
  scan.rs
  layout.rs
  command.rs
  compiler.rs
  profile.rs
  table.rs
  index.rs
  ids.rs
  media.rs
  element.rs
  dependency_resources.rs
  compiled.rs
```

## 文件作用和内部结构

- `plan.rs`: `ResourceCompilePlan`、input roots、dependency resource roots、output artifact paths、thread/compression/icon check options。
- `scan.rs`: 从 session resource inventory 过滤 resource entries，不直接走 project root。
- `layout.rs`: restool output layout 的 typed path view。
- `command.rs`: restool command/config 构造，所有 path serialization 走 `arkdown-path`。
- `compiler.rs`: restool executor，输入 plan，输出 `ResourceCompileOutput`。
- `profile.rs`: module json/resource profile 和 restool config JSON 关联。
- `table.rs`: `ResourceTable.txt` parse/write metadata。
- `index.rs`: `resources.index` metadata。
- `ids.rs`: generated/defined ids 处理。
- `media.rs`: media compression config conversion。
- `element.rs`: resource element model。
- `dependency_resources.rs`: HAR/HSP dependency resource merge order。
- `compiled.rs`: ProcessCompiledResources 对应产物整理。

## 串联关系

```text
ProfileProcessOutput
  -> ResourceCompilePlan
  -> arkdown-resource
  -> ResourceCompileOutput
  -> arkdown-profile GeneratePkgModuleJson
  -> arkdown-pack
```

## 迁移要点

- 不在 resource crate 里重新读 module json/build profile。
- resource binary contents 不进入 `BuildSession`，只在 compiler/writer 中流式读取。
- 所有 directory traversal 必须排序后影响产物。

