# SPEC-048 Refactor Phases

本文件给出总的重构建议和执行阶段。阶段必须按依赖方向推进，不能先迁移高层 CLI 或插件再回补 session。

## Phase 0: 文档、审计、guardrail

目标：

- 保留 `meta/tasks/SPEC-048-build-session-architecture-refactor.md` 为源文档。
- 用本目录记录 crate 级迁移计划和 hvigor 上游映射。
- 用 [hvigor-flow-audit.md](hvigor-flow-audit.md) 固化启动、profile/resource、ArkTS/Rollup/ABC、native、package/sign、preview/test/reload/coverage 的完整上游链路。
- 用 [rolldown-oxc-parity-gap-audit.md](rolldown-oxc-parity-gap-audit.md) 固化迁到 Rolldown/OXC 前必须补齐的上游行为：`hvigorfile.ts` 脚本入口、Rollup lifecycle、自定义编译插件、SDK/TaskService snapshot、TypeScript semantic/checker、copy code resource、HAR artifact、AOT/cache/watch。
- 给 root `Cargo.toml`、crate manifests、CI 或 review checklist 增加依赖方向检查。

必须完成：

- 记录最终 crate 集：Add、Keep、Delete。
- 记录禁止边：`arkdown -> concrete plugins/profile/session`、domain -> session、session -> domain。
- 标记删除目标的最后生产调用方。
- 标记每个 stage 的 hvigor task 对应关系、输入 artifact、输出 artifact、并行汇合点。
- 每个 crate 的 `files.md` 必须引用具体上游 task/plugin/class/function；不能用“参考 hvigor”概括未审计分支。

退出条件：

- `cargo metadata --format-version 1 --no-deps` 可用于验证当前图。
- 每个 crate 都有对应子文档。
- `rolldown-oxc-parity-gap-audit.md` 中的缺口都有 crate owner 和目标文件 API。

## Phase 1: Foundation crates

目标：

- `arkdown-core` 只保留 domain-neutral 类型。
- `arkdown-path` 拥有所有 path/domain path/serializer 策略。
- `arkdown-ohos-schema` 拥有 raw OHOS JSON/JSON5 schema。
- `arkdown-ohpm` 拥有 lockfile、cache、registry、tarball、integrity。

执行顺序：

1. 从 `arkdown-utils`、resolver、profile、pack、CLI 中抽出 path 策略到 `arkdown-path`。
2. 从 config/profile/resource/pack 中抽 raw schema 到 `arkdown-ohos-schema`。
3. 从 `arkdown-profile` 中移出 OHPM registry/cache/lockfile 到 `arkdown-ohpm`。
4. `arkdown-core` 删除旧 plugin API、泛化 resolver/path helper、阶段策略类型。

退出条件：

- 所有 crate manifest 依赖均来自 root `[workspace.dependencies]`。
- `rg "contains\\(\"oh_modules\"\\)|replace\\('\\\\\\\\', \"/\"\\)|to_string_lossy" crates` 的生产命中都有迁移计划或已消除。
- `arkdown-utils` 不再有新增调用方。

## Phase 2: BuildSession and snapshots

目标：

- 创建或补齐 `arkdown-session`。
- `BuildSessionBuilder` 拥有所有启动发现和一次性读取。
- `ModuleCompileContext` 成为后续 plan 的唯一项目上下文来源。

执行顺序：

1. 建 `ProjectSnapshot`：project root、build profile、product、signing、target selection。
2. 建 `HvigorScriptSnapshot`：project/module `hvigorfile.ts`、`hvigorconfig.ts`、build option overlays、hooks、custom task/plugin graph、runtime effects。
3. 建 `ModuleSnapshot`：module root、module json、source roots、resource roots、profile refs、package metadata。
4. 建 `PackageSnapshot` 和 `DependencyGraphSnapshot`：local/remote HAR/HSP、ByteCode HAR、runtimeOnly、dynamic import、integrated HSP、native libs。
5. 建 `SdkSnapshot`：restool、package tool、sign tool、ets-loader、API schema、compile plugin entry。
6. 建 `ProfileStore`、`ResourceInventory`、`SourceStore`、`InputFingerprintSet`、`ArtifactLayout`。

退出条件：

- read-count 测试证明 manifests、package files、profile files、SDK metadata 每个 session 只读一次。
- 没有零指纹或空 snapshot 占位。
- `arkdown-session` 不依赖 profile/resource/pack/sign/cache/plugin/domain crate。

## Phase 3: Domain typed plans

目标：

- 每个 domain crate 从 session-derived plan 执行，不自己发现项目。

执行顺序：

1. `arkdown-profile` 接收 `ProfileProcessPlan`、`LoaderJsonPlan`、`PkgContextInfoPlan`。
2. `arkdown-resource` 接收 `ResourceCompilePlan` 和 artifact paths。
3. `arkdown-pack` 接收 `PackagePlan`，移除 late module json/integrated HSP 读取。
4. `arkdown-sign` 接收 `SignPlan`，只做签名。
5. `arkdown-obfuscate` 接收 `ObfuscationPlan`。
6. `arkdown-ts2abc` 接收 `Ts2abcPlan`，产出 `AbcCompilePlan` 或直接调用 `arkdown-abc`。
7. `arkdown-cache` 接收 `InputFingerprintSet`，不自己决定输入。

退出条件：

- domain crates 不依赖 `arkdown-session`。
- domain crates 不读取 project manifest、package manifest、SDK metadata。
- 同一 fixture 产物与 hvigor parity 测试保持稳定。

## Phase 4: Bundler and plugin context

目标：

- `arkdown` 只做 rolldown bundling。
- plugin crate 变成薄 adapter。

执行顺序：

1. `arkdown` 改为接收 `BundlePlan` 和 plugin trait objects。
2. `arkdown` 增加 `RolldownLifecycleBridge`，覆盖上游 `buildStart/resolveId/load/transform/moduleParsed/beforeBuildEnd/buildEnd/afterBuildEnd/shouldInvalidCache/cleanUp` 语义。
3. `arkdown-cli::hvigorfile_runtime` 增加 project/module `hvigorfile.ts` 和 `hvigorconfig.ts` 的 hvigor-compatible 执行、overlay 归一化、hook capture、自定义 task graph capture，并接入 `BuildRunner`。
4. `arkdown-cli::custom_plugins` 增加 `hvigorPluginFile/compilePluginPath` 加载、排序和 JS hook runtime execution；执行失败按 hvigor 等价错误语义失败，不能降级为 unsupported。
5. `arkdown-plugin-ets-transform` 调用 `arkdown-ets-transform`，不拥有 transform 内核。
6. `arkdown-plugin-ohos-resolve` 调用 `arkdown-resolver` 的 session index。
7. `arkdown-plugin-module-meta`、`arkdown-plugin-har-dedup` 消费 session/dependency graph fact。
8. `arkdown-plugin-json5` 从 source store 或 hook input 取内容。
9. `arkdown-plugin-router-map` 删除，router map invalidation 并入 `ProfileStore` fingerprint。
10. `arkdown-plugin-sensitive-check` 删除，case check 并入 resolver。

退出条件：

- `arkdown` 不依赖任何 `arkdown-plugin-*`。
- plugin hook 中无 project/package/profile/SDK 文件读取。
- plugin 热路径无 `Arc<Mutex<...>>` collector。
- lifecycle parity report 必须给出所有上游 Rollup hook 在 Rolldown 中的对应实现或等价 runtime/toolchain 委托；Phase 4 不允许保留 unsupported hook。

## Phase 5: CLI build runner

目标：

- CLI 从手写全链路改为 build-session + build-plan + runner。

执行顺序：

1. `cmd/build/args.rs` 只解析 CLI。
2. `cmd/build/session.rs` 调用 `BuildSessionBuilder`。
3. `cmd/build/plan.rs` 从 context 建 typed plans。
4. `cmd/build/runner.rs` 建 dependency waves。
5. `cmd/build/pipeline.rs` 执行单 module pipeline。
6. `cmd/build/plugins.rs` 组装 rolldown plugin trait objects。

退出条件：

- CLI 不再直接读写 profile/resource/package 路径布局。
- CLI broad dependencies 只出现在编排层，不泄漏到 `arkdown`。
- `cargo run -p arkdown-cli -- build --project tests/fixtures/minimal-hap` 仍能跑通。

## Phase 6: Cache, watch, deterministic performance

目标：

- cache/watch 统一使用 session inventory 和 fingerprint。
- 并行执行有 deterministic merge。

执行顺序：

1. `InputFingerprintSet` 覆盖所有 hvigor invalid-cache 关心的输入。
2. `arkdown-cache` manifest key 由 fingerprint 生成。
3. `arkdown-watcher` 监听 session inventory，而不是递归目录猜测。
4. `BuildRunner` 以 dependency wave 并行 module。
5. artifact writers 保持顺序写或最终顺序 merge。

退出条件：

- 单线程和多线程构建输出一致。
- 修改一个 module 输入不误伤无关 dependency wave。
- watch invalidation 能解释来自哪个 fingerprint 变化。

## Phase 7: Legacy removal

目标：

- 删除不在最终 crate 集中的旧 crate 和旧 helper。

删除顺序：

1. `arkdown-loader`：最后一个 legacy plugin driver 调用方迁移后删除。
2. `arkdown-utils`：path/hash/cache helper 均迁出后删除。
3. `arkdown-plugin-router-map`：ProfileStore fingerprint 覆盖 router map 后删除。
4. `arkdown-plugin-sensitive-check`：resolver case check 覆盖后删除。

退出条件：

- root workspace members 不再包含删除目标。
- `rg "arkdown-utils|arkdown-loader|arkdown-plugin-router-map|arkdown-plugin-sensitive-check" Cargo.toml crates` 只剩文档或无命中。
- `cargo metadata` 通过，且无违反 SPEC-048 的依赖边。
