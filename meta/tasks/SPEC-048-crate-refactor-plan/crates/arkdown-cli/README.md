# crates/arkdown-cli

Action: Keep.

## 目标职责

`arkdown-cli` 是唯一允许组合所有 stage/domain/plugin crate 的地方。它负责命令解析、session 构造、build plan 构造、plugin 组装、dependency wave runner、module pipeline、progress/diagnostics。它不应把 stage policy 散落在 `cmd/build.rs` 单文件或通用 helper 中。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| command entry task/assemble | `assemble-*.js`, `task-initializer.js` | `cmd/build/args.rs`, `runner.rs` |
| task registry and dependencies | `src/utils/task-initializer.js`, `task-creator.js` | `cmd/build/runner.rs`, `pipeline.rs` |
| ArkCompile worker dispatch | `src/tasks/worker/run-ark.js`, `job.js` | `cmd/build/pipeline.rs` stage orchestration |
| plugin list construction | `arkts-pack.js::getBuiltInPlugins` | `cmd/build/plugins.rs` |
| stage output/progress | hvigor task events/logging | `progress.rs`, diagnostics |

## 目标目录

```text
src/
  main.rs
  lib.rs
  progress.rs
  diagnostics.rs
  cmd/
    mod.rs
    build/
      mod.rs
      args.rs
      session.rs
      plan.rs
      plugins.rs
      runner.rs
      pipeline.rs
      stages.rs
      artifacts.rs
      cache.rs
```

## 文件作用和内部结构

- `cmd/build/args.rs`: CLI flags 到 `BuildArgs`，不读项目。
- `cmd/build/session.rs`: 调用 `BuildSessionBuilder`。
- `cmd/build/plan.rs`: `BuildPlanBuilder`，从 `ModuleCompileContext` 创建 typed domain plans。
- `cmd/build/plugins.rs`: 组装 concrete rolldown plugins，唯一允许依赖 plugin crates 的 build 层。
- `cmd/build/runner.rs`: `BuildRunner`，dependency waves、parallel module scheduling、diagnostic aggregation。
- `cmd/build/pipeline.rs`: `ModulePipeline`，单 module stage 顺序。
- `cmd/build/stages.rs`: domain stage adapters。
- `cmd/build/artifacts.rs`: CLI artifact report，不生成 path policy。
- `cmd/build/cache.rs`: cache plan wiring。
- `progress.rs`: progress/logging。
- `diagnostics.rs`: CLI diagnostic rendering。

## 串联关系

```text
CLI args
  -> BuildSession
  -> BuildPlan
  -> BuildRunner
  -> ModulePipeline
  -> domain stages and plugins
```

## 迁移要点

- `cmd/build.rs` 中手写 profile/resource/package 路径布局必须拆入 `cmd/build/*` 或 session/domain plans。
- CLI broad dependencies 只允许在编排模块，不泄漏到 `arkdown`。
- stage runner 可以并行 module wave，但 archive/sign/cache commit 保持顺序边界。

