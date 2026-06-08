# Rolldown/OXC Parity Gap Audit

本文是在 `upstream-coverage-manifest.md` 之后，对 hvigor/SDK 上游实现做符号级复核得到的强制修正清单。它回答一个问题：按当前 SPEC-048 crate 拆分迁到 Rolldown 和 OXC 后，哪些 hvigor 构建行为会丢失、放错 crate，或在阶段边上不自洽。

## 复核结论

当前重构计划主链路是自洽的，但仍存在 12 类会影响一次性对齐 hvigor 的缺口。修正原则如下：

- `arkdown` 不能只封装 Rolldown build/write，还必须拥有 hvigor Rollup lifecycle 到 Rolldown lifecycle 的桥接语义。
- `arkdown-cli` 必须承担自定义编译插件加载和 JS/TS runtime bridge；Rust domain crate 不加载 JS 插件，也不读取插件配置文件。
- `hvigorfile.ts` 是项目/模块构建脚本入口，不等同于 ArkTS `compilePluginFile`。前者影响 build option、overrides、hooks、task/plugin graph；后者只进入 `arkts-pack.js::loadCustomCompilePlugins`。
- `arkdown-session` 的 `SdkSnapshot`、`DependencyGraphSnapshot` 不能只保留泛化字段，必须覆盖上游 SDK tool/schema API 与 `TaskService` 依赖分类 view。
- OXC 只能替代 parser/AST/codegen 主路径；TypeScript Program、ArkTS checker、Arkoala、single-file declaration、const enum/cache invalidation 等语义不能因为换 OXC 被省略。
- profile/resource/pack/ts2abc 的隐藏边必须显式进 plan：copy code resource、preview resource、startup/share/router、oh-package metadata、AOT、legacy ABC、HAR artifact rewrite。

## 上游符号级差异清单

| 缺口 | 上游具体逻辑 | 当前计划问题 | 强制修正 |
| --- | --- | --- | --- |
| Rollup lifecycle bridge | `arkts-pack.js::doWrapper`、`wrapperBuildStart`、`wrapperResolveId`、`wrapperLoad`、`wrapperTransform`、`wrapperModuleParsed`、`wrapperBeforeBuildEnd`、`wrapperBuildEnd`、`wrapperShouldInvalidCache`、SDK plugin `afterBuildEnd/cleanUp` | `arkdown/files.md` 只写 bundle/run_rolldown，没有 lifecycle state、hook ordering、plugin context/share | `arkdown::lifecycle` 增加 `RolldownLifecycleBridge`、`HookOrderPlan`、`PluginSharedState`；`arkdown-cache::plugin_cache` 增加 hook invalidation |
| `hvigorfile.ts` 项目/模块脚本 | hvigor core/plugin factory 加载 project/module `hvigorfile.ts`/`hvigorconfig.ts`；`projectOhosConfigManager.getOverrides/getHooks`；`build-option-path-info.js::getHookFilePath`；build option priority 包含模块级 hvigorfile 配置 | 当前计划只写 `hvigorPluginFile/compilePluginPath`，没有完整覆盖 hvigorfile 脚本对配置、hook、task graph 的影响 | `arkdown-cli::hvigorfile_runtime` 必须以 hvigor-compatible JS/TS runtime 执行脚本、插件和 hooks；`arkdown-session::hvigor_script` 记录脚本文件、fingerprint、normalized overlay、hook graph、custom task graph；`BuildPlanBuilder` 必须先应用 overlay 再生成 stage plans |
| 自定义编译插件 | `arkts-pack.js::loadCustomCompilePlugins`、`validatePlugin`、`checkPluginDuplicateName`、`getPlugins` 的 `order: first/last/function` | 计划只覆盖内置 plugin，没有定义 JS custom plugin runtime | `arkdown-cli::custom_plugins` 负责读取 `hvigorPluginFile/compilePluginPath`、排序，并通过 `CustomPluginRuntimeBridge` 执行全部上游 hook；脚本自身失败时按 hvigor 等价错误语义失败 |
| 内置 plugin 顺序 | `arkts-pack.js::getBuiltInPlugins`：previewResourceWatcher、copy、common、invalidCache、duplicate、moduleMeta、resolve、routerMap、newOhmUrl、sensitiveCheck、commonjs、gc、json、wrapperChunk、waitAsync、collectImporters、SDK plugins、cleanConfig delete | 当前插件列表缺 copy/common/commonjs/gc/wrapperChunk/waitAsync/cleanConfig 归属 | `arkdown-cli::plugins` 生成完整 `PluginPlanSet`；不能实现为 Rolldown plugin 的项必须落到 `arkdown`/`arkdown-cache`/`arkdown-resource`/`arkdown-profile` 的 typed stage |
| compile config/output | `arkts-pack.js::generateConfig`、`getCompileEnv`、`getOutputConfig`、`manualChunks`、`getWatchConfig` | 当前 `BundlePlan` 缺 compileEnv、watch、manualChunks、preserveSymlinks、shimMissingExports、useAcornLoose、cacheDirectory、writeRollupCache 等字段 | `arkdown::plan` 增加 `CompileEnvironment`、`ManualChunkPolicy`、`WatchConfigBridge`、`RolldownOutputPolicy`；`arkdown-cache::key` 对齐 `getCompileEnv` |
| loader/source entry 分支 | `GenerateLoaderJson`：`declareDynamicImportInputs`、`addStartupInputFiles`、`setByteCodeHarInfoAndOtherCompileFiles`、`collectCompileFiles`、`collectDepEtsFolderFilesEntry`、`checkByteCodeHar` | 只写 compile entries，不足以覆盖 startup、dynamic import、dependency ets folder、bytecode HAR other compile files | `arkdown-profile::loader_json` 拆出 startup/dynamic/dependency-ets/bytecode-HAR fields；`arkdown-session::source_store` 提供一次性扫描结果 |
| copy code resource | `copy_resources_util.js::getCopyOptions`、`ArkCompile.copyCodeResource`：从 `aceModuleRoot/**/*` 复制非 `.ets/.ts/.js`，包含 `../share`、page ability manifest、preview app resources | 当前 resource crate 只写 restool 编译资源，遗漏 code resource copy | `arkdown-resource::copy_code_resource` 作为资源 stage 子步骤，输入来自 `SourceStore/ResourceInventory`，排除规则与上游一致 |
| ProcessResource/CompileResource 细节 | `ProcessResource::initCommandBuilder/getOptCompressionBuilder`、`CompileResource::customizeDistroFilter/writeShareConfig/processRouterMap/processStartupConfig/checkStartupTaskDependencies/validateDependencies` | profile 和 resource 阶段边界不够明确，router/share/startup 被粗略归 profile | `arkdown-profile` 写 router/share/startup profile artifacts；`arkdown-resource` 消费这些 artifacts 生成 restool config，并校验 startup task dependency |
| ProcessOhPackageJson | `process-oh-package-json.js::addArtifactType/processByteCodeHar/processInterfaceHar/addWorkerData/processWorkerConfig/addDynamicImport/processResourceConfig` | 当前 `oh_package_writer.rs` 缺 workers、runtimeOnly dynamic import、resource metadata、interface HAR warning 细节 | `arkdown-profile::oh_package_writer` 增加 worker/resource/runtimeOnly/bytecode/interface-HAR 分支 |
| ProcessHarArtifacts | `process-har-artifacts.js::copyCompiledSourceFileToTempDir/processBundleDepRes/copyBundledDepRes/processHarRouterMap/addPreloadSystemSoJson/processHarResStartupConfig/getObfuscationSrcEntryPath/processDeclarationEntry` 以及 obfuscated HAR ability、startup/insight intent srcEntry | 当前 `har_artifacts.rs` 只写 copy/declarations | `arkdown-pack::har_artifacts` 增加 HAR source staging、router/startup/share/native preload、obfuscated ability srcEntry、insight intent、bundle dependency resource |
| DependencyGraph/TaskService views | `TaskService`/dependency manager：remote HAR/HSP、ByteCode HAR、HSP recursion、SO deps、direct/all deps、root deps、affects compilation、dynamic HSP、remote HSP metadata | 当前 `DependencyGraphSnapshot` 只有 har/hsp/bytecode/native 基础 view | `arkdown-session::dependency_graph` 增加完整分类方法；domain plan 只消费窄 `DependencyView` |
| SDK tool/schema snapshot | `DefaultSdk`：restool、package/sign/syscap、module/app/router/share/startup/schema、ArkUIX、ets/js/sdk dirs、preview permission、AOT version 等大量 getter | 当前 `SdkSnapshot::tool_path/schema_path` 太泛化，无法核对缺失 SDK 输入 | `arkdown-session::sdk` 拆 `SdkToolchainSnapshot`、`SdkSchemaSnapshot`、`SdkPermissionSnapshot`、`SdkCompilePluginSnapshot`，每个上游 getter 有 owner |
| ETS transform semantic path | `rollup-plugin-ets-typescript.js`：`getCompilerHost`、`getArkoalaTsProgram`、`validateEts`、`getShouldEmitJs`、`setIncrementalFileInHar`、`checkRelateToConstEnum`；`rollup-plugin-ets-checker.js` | OXC AST 转换计划覆盖 syntax，但没有 TypeScript Program/semantic/checker/cache parity | `arkdown-ets-transform::semantic`、`transform_cache`、`arkoala` 必须存在；OXC 只替代 AST/codegen，不替代 semantic policy |
| API transform/module facts | `rollup-plugin-system-api.js::processSystemApi/processLibso/collectKitModules/replaceKitModules`、`process_lazy_import.js` | 当前 facts 写得泛化，缺 kit compare/replace 和 libso preload 传递边 | `arkdown-ets-transform::api_transform` 输出 `TransformFacts`，交给 `arkdown-profile` loader/pkg-context 和 `arkdown-pack` native preload |
| AOT/legacy ABC | SDK `lib/gen_aot.js`、legacy `gen_abc_plugin.js`/`generate_module_abc.js`/`generate_bundle_abc.js`、`run-ark.js::submitArkCompileWork` | `arkdown-ts2abc` 只写 fast-build genAbc 主路，遗漏 AOT 和 legacy/fallback | `arkdown-ts2abc::aot`、`legacy_bundle`、`module_mode/bundle_mode` 明确保留；legacy path 必须实现 Rust 等价 stage 或委托上游等价 toolchain 执行 |
| Cache/watch invalidation | `invalid-cache-plugin.js`、`rollup-cache-service.js`、`rollup-plugin-watch-change.js`、`shouldInvalidCache` hook | 当前 cache 只写 key/store，缺 plugin cache decision 与 watch event name bridge | `arkdown-cache::plugin_cache`、`rollup_cache`、`event_name`；`arkdown-watcher` 消费 session inventory |
| Preview/test/reload/coverage | `PreviewBuild`、`UnitTestBuild`、`OhosTestArkCompile`、`HotReloadBuild`、`ColdReloadArkTS`、coverage SDK hooks | CLI 子阶段已有，但与 source/loader/ts2abc/profile plan 的字段未强绑定 | `arkdown-cli` stage plan 必须复用 `ModuleCompileContext`、`SourceStore`、`ProfileStore`，不能另起文件扫描 |

## 修正后的阶段边

```text
BuildSession
  -> HvigorScriptSnapshot(project/module hvigorfile + hvigorconfig overlay + hooks + custom task graph)
  -> SdkSnapshot(tool/schema/compile-plugin/permission/aot)
  -> DependencyGraphSnapshot(all TaskService views)
  -> SourceStore(profile/resource/code-resource/worker/visual/startup inputs)
  -> ProfileStore(router/share/startup/privacy/form/shortcut/insight)
  -> ResourceInventory(restool roots + native/resource/code-resource views)

BuildPlanBuilder
  -> apply HvigorScriptOverlay before all stage plans
  -> splice HvigorHookGraph/HvigorCustomTaskGraph into BuildRunner
  -> ProfileProcessPlan(loader/pkg-context/module-json/oh-package)
  -> ResourceCompilePlan(restool + code resource + compiled resource)
  -> BundlePlan(compileEnv/output/watch/manualChunks/lifecycle)
  -> EtsTransformPlan(OXC AST + TS semantic/checker/Arkoala)
  -> Ts2abcPlan(fast/module/bundle/single-file/AOT/legacy)
  -> PackagePlan(HAP/HSP/HAR + HAR artifacts + native preload)
  -> SignPlan

BuildRunner
  -> dependency waves
  -> profile/resource 汇合到 ArkCompile
  -> Rolldown lifecycle bridge
  -> transform facts merge
  -> ABC/AOT
  -> package/sign
```

## 迁移到 Rolldown/OXC 的硬性兼容策略

1. `arkdown` 要验证 Rolldown 是否暴露等价 hook。没有等价 hook 的上游行为必须由 pre/post stage 模拟，并在 `LifecycleParityReport` 中标注。
2. OXC 负责 `.ets/.ts/.js` parse、AST transform、codegen 和 sourcemap；TypeScript semantic program、ArkTS checker、Arkoala 兼容逻辑不能用 OXC parse 结果直接替代，必须有 `SemanticProgramBridge` 或等价 Rust 实现。
3. JS 自定义编译插件必须能通过 `CustomPluginRuntimeBridge` 执行；不能用 hard diagnostic 代替能力。只有脚本自身运行失败时，才按 hvigor 同等错误语义失败。
4. `hvigorfile.ts` 的 project/module script、overrides、hooks、自定义 task/plugin graph 必须由 `HvigorfileRuntimeBridge` 执行并镜像到 `BuildRunner`。禁止用静态解析、忽略、降级或 unsupported diagnostic 代替执行。
5. 所有上游全局 `projectConfig/loaderConfig/share` 必须变成 typed state：`BuildSession` 只读输入，`BuildPlan` 固化策略，`LifecycleBridge` 持有本次 bundle 内 hook state，`TransformFacts`/`BundleFacts` 做跨 stage 传递。
6. 每个 legacy/fallback 路径必须二选一：实现对应 Rust stage，或委托给上游等价 runtime/toolchain 执行。不能被主链路描述覆盖掉。

## 必补 parity 测试矩阵

| 测试 | 覆盖上游 |
| --- | --- |
| HAP debug/release ESMODULE，含 `.ets/.ts/.js/.json/.visual/.d.ets` | `GenerateLoaderJson`、`arkts-pack.js`、SDK `etsTransform/apiTransform/genAbc` |
| HAR `UseTsHar`、bytecode HAR、interface HAR、runtimeOnly dynamic import | `process-oh-package-json.js`、`ProcessHarArtifacts`、`genAbc` |
| HSP with shared HAR/TGZ、integrated HSP、native preload system so | `PackageHsp`、`PrepareSharedHarRes`、`process_module_preload_file_utils.js` |
| startup/share/router/privacy/form/shortcut/insight intent | `CompileResource` profile processors、`ProcessHarArtifacts` |
| custom compile plugin with `order: first/last/function` and each supported hook | `arkts-pack.js::loadCustomCompilePlugins/doWrapper` |
| project/module `hvigorfile.ts` with overrides, hooks, and `arkOptions.compilePluginFile` | hvigor script loader、`projectOhosConfigManager`、`BuildOptionPath`、`ArkCompile.getHvigorPluginFile` |
| watch/hot reload/cold reload/preview/unit/ohosTest/coverage | hvigor mode-specific task graph |
| AOT and compatible API <= 10 JS branch | `gen_aot.js`、`ark_compiler/transform.js` |
