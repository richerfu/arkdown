# SPEC-048 Crate Refactor Plan

本文档目录是 `meta/tasks/SPEC-048-build-session-architecture-refactor.md` 的执行拆分版。它按 crate 建立独立子目录，说明每个 crate 的目标职责、内部文件结构、对应 hvigor 上游逻辑，以及 crate 之间如何串联。

## 文档结构

```text
meta/tasks/SPEC-048-crate-refactor-plan/
  README.md
  phases.md
  upstream-hvigor-map.md
  hvigor-flow-audit.md
  arkts-transform-audit.md
  upstream-coverage-manifest.md
  rolldown-oxc-parity-gap-audit.md
  end-to-end-file-flow.md
  crates/
    arkdown-core/README.md
    arkdown-path/README.md
    ...
```

每个 `crates/<crate>/README.md` 都必须回答四个问题：

1. 这个 crate 保留、添加还是删除。
2. 这个 crate 的主逻辑对应 hvigor 哪些任务、插件、工具或模型。
3. 这个 crate 内部目标文件怎么拆，每个文件承担什么代码结构。
4. 这个 crate 的输入来自哪里，输出交给哪个后续 crate。

每个 `crates/<crate>/files.md` 是文件级实现合同，必须补齐：

1. 每个目标文件内应该有哪些 `struct`、`enum`、`trait`、`type`。
2. 每个目标文件内应该有哪些 public 或 stage-facing 方法/关联函数；这些方法必须挂在结构体、枚举或 trait 上，不得规划 public free function。
3. 这些 API 从当前 arkdown 哪些文件迁移。
4. 这些 API 对应 hvigor 或当前 `arkdown-abc` 哪段上游/既有逻辑。

## 文件级 API 规则

所有 `files.md` 必须遵守同一导出形态：

- crate 对外 API 必须由命名类型承载，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- `pub fn` free function 不允许作为目标 crate 导出面。表格中的“方法/关联函数”列只能表示对应类型的 `impl` 方法、associated function、builder 方法或 trait 方法。
- 旧代码中的 free function 只能出现在“迁移来源”列。迁移时必须挂到拥有上游语义的类型上。
- 如果旧 free function 是跨 crate 共享的纯算法或基础转换，不能留在 domain crate 导出；必须迁入已有 common/foundation owner crate，例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate，并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行必须对齐上游原始逻辑；`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件。若是 Rust ownership adapter，必须说明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 上游源码根

本计划基于本机 DevEco Studio 6.0.0.858 内置 hvigor 代码梳理：

```text
/Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/hvigor-ohos-plugin/
  src/tasks/
  src/utils/
  src/project/dependency/
  src/sdk/
  src/builder/
  node_modules/@ohos/hvigor-arkts-compose/dist/src/
  node_modules/@ohos/hvigor-arkts-base/dist/src/
```

后续如果 DevEco 版本升级，必须先重新跑上游映射核对，再更新这些 crate 文档。

## 流程审计入口

[hvigor-flow-audit.md](hvigor-flow-audit.md) 是从启动配置读取、task DAG、profile/resource、ArkTS/Rollup/ABC、native、HAP/HSP/HAR package、sign、preview/test/reload/coverage 全链路反查出来的自洽性审计。后续修改任意 crate 子文档时，必须先确认没有违反该审计里的阶段边：

- `GenerateLoaderJson` 与 `CompileResource` 在 `ArkCompile` 汇合，不是父子关系。
- SDK `compile_plugin.js` 的 `etsChecker/etsTransform/apiTransform/genAbc` 顺序必须被 arkdown 分层保留。
- HAP/HSP/HAR package plan 必须覆盖 `MakePackInfo`、`CacheNativeLibs`、`GenerateMetadata`、`SyscapTransform`、router/share/startup、privacy、integrated HSP、compiled resources、shared tgz 等辅助产物。
- native、preview、unit test、ohosTest、coverage、reload 是 CLI stage 分支，不能让 domain crate 自行探测。

[arkts-transform-audit.md](arkts-transform-audit.md) 是 ArkUI/ArkTS 到 TS/JS/ABC 的细粒度审计。它明确 `GenerateLoaderJson` 产生哪些源码入口、`.ets/.ts/.js/.json/.d.ts/.d.ets/.visual/lib*.so` 如何分流、SDK `etsTransform/apiTransform/genAbc` 的分支顺序，以及 arkdown 使用 OXC 时的迁移顺序。

[upstream-coverage-manifest.md](upstream-coverage-manifest.md) 是本轮上游审计覆盖口径。它锁定 DevEco Studio 6.0.0.858，列出 hvigor task/plugin graph、ArkTS compose/base graph、SDK compile plugin graph 的可执行 JS 文件数和分支矩阵。后续不能再只说“参考 hvigor”，必须能追溯到该 manifest 的入口和分支。

[rolldown-oxc-parity-gap-audit.md](rolldown-oxc-parity-gap-audit.md) 是迁移到 Rolldown/OXC 前的强制差异审计。它列出上游符号级复核后发现的缺口：`hvigorfile.ts` 脚本入口、Rollup lifecycle bridge、自定义编译插件、SDK/TaskService snapshot、TypeScript semantic/checker、copy code resource、HAR/oh-package/AOT/cache/watch 等。任意 crate 子文档若与该 audit 冲突，必须先修子文档。

[end-to-end-file-flow.md](end-to-end-file-flow.md) 是文件级串联图。它把 29 个 crate 子文档中的 stage-facing 文件、结构体和方法串成 `BuildSession -> BuildPlanBuilder -> BuildRunner -> ModulePipeline -> profile/resource/bundle/ets/abc/package/sign/cache/watch` 的完整调用链，并标出每段对应的 hvigor 上游阶段。

## SPEC-049 当前基线

`SPEC-049` 的 ABC Rust AOT 重构已经合并到当前分支，后续 SPEC-048 计划必须以这条链作为 ABC 基线，而不是回到旧的 `AbcModule -> AbcFile` happy path：

```text
arkdown-cli::Ts2abcStage
  -> arkdown-ts2abc::Ts2abcPlan/Ts2abcOptions
  -> arkdown-abc::aot::AotCompileOptions
  -> AotRunner
     -> CompileFileScheduler
     -> ProgramRegistry
     -> AbcInputProgramCompiler
     -> NpmModuleEntryCompiler
     -> DepsRelationResolver
     -> ProgramOptimizer
     -> AotOutputGenerator
  -> ABC/proto/cache outputs
  -> PackagePlan
```

本轮重新审计后的强制结论：

1. `arkdown-abc` 文档必须按当前 `aot/`、`program/`、`ir/`、`emit/` 目录重新描述；旧计划中的 `emit/index_section.rs`、`ir/assignment_targets.rs` 已不是当前基线。
2. `arkdown-ts2abc` 当前仍是 SDK/es2abc command facade，不是 production typed plan；必须把 `compile_to_abc*`、fileInfo 生成、record/path derivation 迁到 `Ts2abcCompiler`、`Ts2abcPlanBuilder`、`FileInfoManifestWriter` 等命名类型上。
3. 计划必须显式保留 SPEC-049 的上游对齐点：MurmurHash3、Program key、ABC input merge consistency、dependency BFS、static/dynamic import 收集、npm entries、Program cache、proto output、global optimization gate。
4. 计划也必须显式列出 SPEC-049 尚未完全等价的点：non-merge filelist 第 4 列 output path、`dumpSymbolTable` cache bypass、dynamic import `IsLabel()`、`AnalysisProgram` 全局分析、annotation records 与 ets2panda 校验、`pandaFileVersion` cache interop。

## 总体重构建议

1. 先建底座，再迁移行为。`arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-ohpm`、`arkdown-session` 必须先形成稳定 API，否则 profile、resolver、plugin 会继续反向读取文件。
2. `BuildSession` 必须拥有真实启动读：build profile、module/app json、oh-package、lockfile、SDK、profile、resource、source inventory、artifact layout、fingerprint。零指纹和占位 snapshot 必须删除。
3. `hvigorfile.ts`/`hvigorconfig.ts` 不能被 build profile 读取替代。项目/模块脚本产生的 build option overlays、hooks、自定义 task/plugin graph、`arkOptions.compilePluginFile` 必须通过 hvigor-compatible runtime bridge 执行并归一化为 `HvigorScriptSnapshot`，再进入后续 typed plan 和 `BuildRunner`。
4. domain crate 只消费 typed plan。`arkdown-profile`、`arkdown-resource`、`arkdown-pack`、`arkdown-sign`、`arkdown-obfuscate`、`arkdown-ts2abc`、`arkdown-cache` 不得依赖 `arkdown-session`。
5. `arkdown` 只能做 rolldown bundling 和 bundle output 转换。它不得依赖具体 plugin crate、profile crate、session crate。
6. plugin crate 必须薄化。插件构造参数来自 `ModuleCompileContext` 或更窄 plan，hook 内不得读 `module.json5`、`oh-package.json5`、profile 文件、SDK 文件或向上查找包根。
7. 路径策略必须集中到 `arkdown-path`。artifact slash path、OHM URL、module specifier、filesystem path 不能继续混成 `String`。
8. cache key 必须来自 `InputFingerprintSet`，包括源码、profile、manifest、SDK、resource、dependency graph、transform options。占位 hash 和局部配置 hash 必须清理。
9. 删除目标 crate 不能保留兼容壳。最后生产调用方迁移后，同一阶段删除 `arkdown-utils`、`arkdown-loader`、`arkdown-plugin-router-map`、`arkdown-plugin-sensitive-check`。
10. Rolldown/OXC 迁移不得只覆盖主 happy path。`arkdown` 必须保留 hvigor Rollup lifecycle 语义，`arkdown-ets-transform` 必须保留 TypeScript semantic/checker/Arkoala 分支，`arkdown-ts2abc` 必须覆盖 module ABC、bundle ABC、single-file emit、AOT、legacy ABC fallback，不能用 OXC parser 成功替代这些上游行为。
11. ABC 迁移必须直接落到 `arkdown-abc::aot::AotRunner` 或 SDK `es2abc` delegate。不得新增 CLI handoff；当前 handoff 数据必须迁入 `Ts2abcPlan`、source map/cache/fileInfo 等正式 owner，不能绕过 `ProgramRegistry`、dependency BFS、Program cache、ABC input consistency 这些 SPEC-049 已经对齐的阶段。
12. 所有 handoff/bridge/intermediate 类文件都必须改造成最终架构形态。迁移完成后删除原文件，不保留 debug/parity/compat 变体；调试和 parity 所需数据由 typed plan snapshot、stage output 或正式 artifact writer 提供。

## 目标执行链

```text
arkdown-cli
  -> BuildArgs
  -> arkdown-session::BuildSessionBuilder
     -> ProjectSnapshot
     -> ModuleSnapshot[]
     -> PackageSnapshot[]
     -> DependencyGraphSnapshot
     -> SdkSnapshot
     -> ProfileStore
     -> ResourceInventory
     -> SourceStore
     -> InputFingerprintSet
     -> ArtifactLayout
  -> arkdown-cli::BuildPlanBuilder
     -> ModuleCompileContext[]
     -> ResourceCompilePlan
     -> ProfileProcessPlan
     -> BundlePlan
     -> ObfuscationPlan
     -> Ts2abcPlan
     -> PackagePlan
     -> SignPlan
  -> arkdown-cli::BuildRunner
     -> dependency waves
     -> ModulePipeline
        -> profile/resource/loader/pkgContext
        -> rolldown bundle
        -> plugin facts merge
        -> obfuscation
        -> ts2abc
           -> arkdown-abc AOT runner or SDK es2abc backend
        -> package
        -> sign
  -> arkdown-watcher invalidation over session inventory
```

## 阶段入口

详细阶段见 [phases.md](phases.md)。

| Phase | 目标 | 退出条件 |
| --- | --- | --- |
| 0 | 文档和 guardrail | crate 文档、禁止边、删除目标清晰 |
| 0A | SPEC-049 AOT 基线回灌 | ABC/ts2abc/CLI 文档对齐当前 AOT runner，所有剩余上游差异有 owner |
| 1 | foundation | path/schema/core/OHPM API 固化 |
| 2 | session | 启动 snapshot 和 read-once 测试可用 |
| 3 | domain plans | profile/resource/pack/sign/cache 等只吃 plan |
| 4 | bundler/plugins | `arkdown` 去具体 plugin 依赖，plugin 薄化 |
| 5 | CLI runner | CLI 只做编排，模块 wave pipeline 成型 |
| 6 | cache/watch | fingerprint、watch inventory、增量失效统一 |
| 7 | legacy removal | 删除旧 crate 和旧 helper |

Phase 4 开始前必须完成 [rolldown-oxc-parity-gap-audit.md](rolldown-oxc-parity-gap-audit.md) 中所有 “强制修正” 对应的文件级计划。否则迁到 Rolldown/OXC 后只会得到一个可运行但不等价 hvigor 的构建链。

## Crate 子文档索引

Foundation:

- [arkdown-core](crates/arkdown-core/README.md)
- [arkdown-path](crates/arkdown-path/README.md)
- [arkdown-ohos-schema](crates/arkdown-ohos-schema/README.md)

Startup and graph:

- [arkdown-ohpm](crates/arkdown-ohpm/README.md)
- [arkdown-config](crates/arkdown-config/README.md)
- [arkdown-resolver](crates/arkdown-resolver/README.md)
- [arkdown-session](crates/arkdown-session/README.md)

Domain stages:

- [arkdown-profile](crates/arkdown-profile/README.md)
- [arkdown-resource](crates/arkdown-resource/README.md)
- [arkdown-cache](crates/arkdown-cache/README.md)
- [arkdown-abc](crates/arkdown-abc/README.md)
- [arkdown-ts2abc](crates/arkdown-ts2abc/README.md)
- [arkdown-pack](crates/arkdown-pack/README.md)
- [arkdown-sign](crates/arkdown-sign/README.md)
- [arkdown-obfuscate](crates/arkdown-obfuscate/README.md)

Bundler and transform:

- [arkdown](crates/arkdown/README.md)
- [arkdown-ets-transform](crates/arkdown-ets-transform/README.md)

Rolldown plugins:

- [arkdown-plugin-ets-transform](crates/arkdown-plugin-ets-transform/README.md)
- [arkdown-plugin-ohos-resolve](crates/arkdown-plugin-ohos-resolve/README.md)
- [arkdown-plugin-module-meta](crates/arkdown-plugin-module-meta/README.md)
- [arkdown-plugin-json5](crates/arkdown-plugin-json5/README.md)
- [arkdown-plugin-duplicate-check](crates/arkdown-plugin-duplicate-check/README.md)
- [arkdown-plugin-har-dedup](crates/arkdown-plugin-har-dedup/README.md)

Orchestration:

- [arkdown-cli](crates/arkdown-cli/README.md)
- [arkdown-watcher](crates/arkdown-watcher/README.md)

Delete targets:

- [arkdown-utils](crates/arkdown-utils/README.md)
- [arkdown-loader](crates/arkdown-loader/README.md)
- [arkdown-plugin-router-map](crates/arkdown-plugin-router-map/README.md)
- [arkdown-plugin-sensitive-check](crates/arkdown-plugin-sensitive-check/README.md)
