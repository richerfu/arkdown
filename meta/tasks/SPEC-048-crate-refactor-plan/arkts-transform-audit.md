# ArkTS/ArkUI Transform Audit

本文补齐 `hvigor-flow-audit.md` 中 ArkTS/ArkUI 到 TS/JS/ABC 的细粒度链路。它不是抽象说明，而是以 DevEco Studio 6.0.0.858 内置上游实现为准，逐项说明哪些文件会进入转换、每类文件如何转换、版本和构建模式如何改变分支，以及 arkdown 使用 OXC 时应如何迁移。

## 上游范围

SDK 入口：

```text
<DevEco>/Contents/sdk/default/openharmony/ets/build-tools/ets-loader/compile_plugin.js
```

核心上游文件：

| 上游文件 | 作用 | arkdown 归属 |
| --- | --- | --- |
| `compile_plugin.js` | SDK plugin 顺序：`memoryMonitor -> watchChangeFiles -> etsChecker -> visualTransform -> etsTransform -> apiTransform -> genAbc -> terser -> babel -> createProgramPlugin` | `arkdown-cli` 固化 plan 顺序，`arkdown-plugin-ets-transform` 薄适配 |
| `main.js` | 读取 `aceBuildJson`、`compileEntry`、`otherCompileFiles`、`cardEntryObj`、`modulePathMap`、partial update/ArkTSVersion/UseTsHar 等全局配置 | `arkdown-session::SdkSnapshot` + `ModuleCompileContext` |
| `lib/fast_build/common/init_config.js` | 将 `projectConfig.entryObj + otherCompileFiles` 合成 Rollup input | `arkdown-profile::loader_json` + `arkdown-cli::BuildPlanBuilder` |
| `lib/fast_build/visual/rollup-plugin-visual.js` | `.ets` visual 文件注入，watch 或非 esmodule 时生效 | `arkdown-ets-transform::visual` |
| `lib/process_visual.js` | `.visual` 生成 ETS 片段并注入 import、变量、build、aboutToAppear | `arkdown-ets-transform::visual` |
| `lib/fast_build/ets_ui/rollup-plugin-ets-typescript.js` | `.ets/.ts` 过滤、UI 校验、ArkUI lowering、kit/import path、HAR 增量文件 | `arkdown-ets-transform` |
| `lib/validate_ui_syntax.js` | ArkUI 语法校验、`@Entry/@Component/@Builder/@Extend` 等规则 | `arkdown-ets-transform::checker` |
| `lib/process_ui_syntax.js` + `process_component_*` | ArkUI DSL lowering，生成 ViewPU/ViewV2 形态 | `arkdown-ets-transform::component`、`viewpu` |
| `lib/import_path_expand.js` | 扩展 import 路径 | `arkdown-ets-transform::import_path` + `arkdown-resolver` |
| `lib/process_kit_import.js` | kit import 改写和 keep-ts 检查 | `arkdown-ets-transform::kit_import` |
| `lib/fast_build/system_api/rollup-plugin-system-api.js` | system API、kit module、`lib*.so` import 改写和 collection 文件 | `arkdown-ets-transform::api_transform` |
| `lib/fast_build/ark_compiler/rollup-plugin-gen-abc.js` | module/bundle/single-file ABC 输出插件 | `arkdown-ts2abc` + `arkdown-abc` |
| `lib/fast_build/ark_compiler/transform.js` | `genAbc` transform 阶段收集 TS/ETS/JS/JSON module source，JS 兼容 API10 Babel 分支 | `arkdown-ts2abc::module_source` |

## 哪些文件需要转换

上游不是简单扫描 `src/main/ets`。真实入口由 `GenerateLoaderJson` 生成，再由 SDK `init_config.js` 转为 Rollup input。arkdown 必须把这些入口来源保留成 typed origin，不能压成字符串数组。

### `compileEntry`

`compileEntry` 会包含这些源码入口：

| 来源 | 上游逻辑 | 文件类型 | arkdown plan |
| --- | --- | --- | --- |
| insight intent | `readInsightIntentSrcEntry`，以及 HAR dependency `insight_intent.json` 中的 `srcEntry` | `.ets/.ts/.js` | `CompileEntryOrigin::InsightIntent` |
| 模块源码目录 | HSP/HAR full compilation 会递归 `src/main/ets`；HAP/HSP dependency ETS folder 也可被收集 | `.ets/.ts/.js` | `CompileEntryOrigin::SourceRootCollect` |
| mock/test/preview | preview、ohosTest、local test 会把 mock source 加入入口 | `.ets/.ts/.js` | `CompileEntryOrigin::Mock` |
| startup | `ProcessStartupConfig` 产出的 `configEntry` 和 startup task `srcEntryAbsolutePath` | `.ets/.ts/.js` | `CompileEntryOrigin::Startup` |
| router map | loader router map 中每个 `pageSourceFile` | `.ets/.ts/.js` | `CompileEntryOrigin::RouterPage` |
| dynamic import sources | `dynamicImport.runtimeOnly.sources`，文件直接加入，目录递归加入 | `.ets/.ts/.js` | `CompileEntryOrigin::RuntimeOnlySource` |
| dynamic import packages | runtime-only package 的 main/types、HAR package source folder、非 HSP/SO package entry | `.ets/.ts/.js` | `CompileEntryOrigin::RuntimeOnlyPackage` |
| bundled dependencies | bytecode HAR bundled dependency workers、runtimeOnly、router pageSourceFiles | `.ets/.ts/.js` | `CompileEntryOrigin::BundledDependency` |
| bytecode HAR | bytecode HAR module info generated file | `.ets/.ts/.js` | `CompileEntryOrigin::BytecodeHarModuleInfo` |
| HAR dependency ability | HAR dependency `UI_ABILITY`、`EXTENSION_ABILITY` `srcEntry` | `.ets/.ts/.js` | `CompileEntryOrigin::DependencyAbility` |
| worker | loader `workers` 经 SDK config 进入 Rollup graph 和 ABC record 生成 | `.ets/.ts/.js` | `CompileEntryOrigin::Worker` |
| form/card | widget/card entry object | `.ets/.ts/.js` | `CompileEntryOrigin::Card` |

`processCodeFiles` 会过滤 `.d.ts` 和 `.d.ets`，做去重、存在性检查和 realpath。arkdown 需要在 `arkdown-profile::loader_json` 保存原始 origin，同时在 `arkdown-ets-transform::SourceClassifier` 中再次拒绝 declaration 文件，保证 hook 侧不会误转声明文件。

### `otherCompileFiles`

`otherCompileFiles` 主要来自 normalized OHM URL、bytecode HAR、dependency extra compile entrances。SDK `init_config.js` 会把它和 `entryObj` 合并为 Rollup input。

arkdown 中它不能是 “附加字符串”。它必须进入 `TransformFilePlan`，因为它影响：

- 是否需要参与 resolver graph。
- 是否产生 ABC module source。
- HAR/HSP 的 incremental output 是 `.js` 还是 `.ts`。
- fingerprint 是否覆盖 bytecode HAR dependency metadata。

### `declarationEntry`

`declarationEntry` 是声明文件和 bytecode HAR metadata 的输入，不走 ArkUI/ETS transform。它影响 HAR 声明输出、normalized OHM URL 记录名、ABC compile context，但 `.d.ts/.d.ets` 必须始终从 `visualTransform/etsTransform/apiTransform` 的源码转换路径排除。

## 文件类型分流矩阵

| 文件类型 | 是否来自 loader input | `visualTransform` | `etsTransform` | `apiTransform` | `genAbc` | arkdown 处理 |
| --- | --- | --- | --- | --- | --- | --- |
| `.ets` | 是，来自 `compileEntry/otherCompileFiles/card/workers` | 只在 `.ets` 且存在 `.visual`；watch 或非 esmodule 生效 | 是，排除 `.d.ets`；执行 ArkUI 校验和 lowering | ESMODULE 下仅当 `shouldEmitJs`、crossplatform 或 widget 需要回写 code；JSBUNDLE 总是改写 code | 是；可能以 JS 或保留 TS/ETS module source 进入 ABC | `SourceType::ets()` parse，ArkUI pass，version/mode 决策输出 |
| `.ts` | 是 | 否 | 是，排除 `.d.ts`；不跑 `validateEts`，但会跑 TS emit transformers | 同 `.ets`，但 crossplatform 只在 filter 覆盖时进入 | 是；可能 emitted JS 或 keep TS | OXC TS parser + import/kit/obfuscation pass |
| `.js` | 可能来自 loader、import graph、runtimeOnly package | 否 | 否 | 正常模式仅当 output code 已是 JS；crossplatform 会显式纳入 `.js` | 是；兼容 API <= 10 时 Babel class-properties transform | OXC JS parser for import/API/libso/lazy-import；不做 ArkUI lowering |
| `.json` | import graph 中可能出现 | 否 | 否 | 否 | 是，作为 module source 进入 ABC | JSON module source plan，不走 ETS transform |
| `.d.ts/.d.ets` | 可能在 declarationEntry 或 dependency metadata 中 | 否 | 否，正则 `(?<!\.d)` 排除 | 否 | 不作为 source module；只影响声明/compile context | `DeclarationInput`，不能进 transform hook |
| `.visual` | 不是 Rollup input；由 `.ets` 查找 | 被读取并生成 ETS 片段 | 生成后的 `.ets` 继续走 ETS transform | 后续同 `.ets` | 后续同 `.ets` | `VisualFileSnapshot` 由 session 读取，transform crate 不再查文件 |
| `lib*.so` import | 出现在 `.ets/.ts/.js` import 中 | 否 | 不处理 | 改写或收集 `globalThis.requireNapi`/ES import | 影响 module info/package native entries | `api_transform::libso` + `arkdown-pack::native_libs` |
| resource refs | 出现在 `.ets/.ts` 调用中 | 否 | 资源引用 lowering/校验 | 不负责资源 ID 分配 | 影响 JS/ABC 和 source map | `resource_transform` 读取 `ResourceInventory` facts |

## `.ets` 到 TS/JS 的上游分支

### Visual 预处理

`visualTransform` 的 filter 是 `/(?<!\.d)\.ets$/`。它只有在以下条件之一满足时返回 code：

- `process.env.watchMode === "true"`。
- `projectConfig.compileMode !== "esmodule"`。

`process_visual.js` 会按 `.ets` 路径寻找同名 `.visual`，用 `codegen_ets.js` 生成 ETS 片段。若存在 visual 文件，入口 component 的 `build()` 必须为空，否则报错。成功后注入：

- import 片段。
- visual 生成的变量和函数。
- `build` body。
- `aboutToAppear` body，若源文件没有该方法则新建。
- super visual source map。

arkdown 迁移要求：

- `arkdown-session` 读取 `.visual` 为 `VisualFileSnapshot`，记录 mtime/hash。
- `arkdown-ets-transform::visual::VisualTransform` 只消费 snapshot，不在 hook 内找路径。
- 注入使用 OXC AST 或 span edit；source map 必须把注入行和原 `.ets` 行关系保留下来。

### JSBUNDLE 模式

上游条件是 `projectConfig.compileMode !== "esmodule"`。

执行顺序：

```text
.ets only: preprocessExtend -> preprocessNewExtend -> validateUISyntax
.ets/.ts: TypeScript transpileModule
  before transformer: processUISyntax
output: JS code + source map
```

关键差异：

- `.ets` 会先跑 `validateUISyntax`；`.ts` 不跑这个校验。
- `processUISyntax` 是 TypeScript transformer，负责 ArkUI DSL lowering。
- JSBUNDLE 一定返回 `{ code, map }`，后续 `apiTransform` 必须回写 JS。

OXC 迁移：

- `.ets` 使用 `SourceType::ets()`，保留 `StructStatement` 和 ArkUI component expression。
- `.ts` 使用 OXC TS parser；若没有 ArkUI AST 节点，只执行 import path、kit、obfuscation facts。
- JS emit 使用 `oxc_transformer` + `oxc_codegen`，ArkUI lowering 先把 OXC ArkUI AST 降成标准 TS/JS AST 或稳定 IR，再 codegen。

### ESMODULE 模式

上游条件是 `projectConfig.compileMode === "esmodule"`。

执行顺序：

```text
TS Program / builder program
  -> get/check source file
  -> .ets only validateEts
  -> getShouldEmitJs(projectConfig.shouldEmitJs, sourceFile, id)
  -> if shouldEmitJs:
       program.emit with before transformers:
         processUISyntax
         expandAllImportPaths
         processKitImport
         collectReservedNameForObf
       returns JS code/map/meta shouldEmitJs
     else:
       transformNodes with same transformers
       returns tsAst meta or printed source
```

`getShouldEmitJs` 的关键分支：

- 默认 `true`。
- `projectConfig.processTs` 打开时，HAR、`UseTsHar`、`byteCodeHar`、`shouldEmitJs`、`@ts-nocheck/@ts-ignore`、`checkHasKeepTs` 都会改变是否输出 JS。
- HAR 增量输出中 `.ets/.ts` 会同时映射到 `.d.ets/.d.ts`，并根据 `useTsHar` 决定 `.js` 还是 `.ts`。

OXC 迁移：

- `TransformOutput` 不能只有 `String code`。必须是：
  - `TransformOutput::Js { code, map, facts }`
  - `TransformOutput::TsForAbc { code_or_ir, facts }`
  - `TransformOutput::DeclarationOnly`
  - `TransformOutput::Skipped`
- 当上游会返回 TS AST 给 `genAbc` 时，arkdown 不能丢弃 AST/facts。若 ABC 后端仍是 SDK `es2abc`，需要先落成上游等价的 TS/JS module source；若用 Rust/arkdown ABC backend，则 `Ts2abcPlan` 直接消费 OXC IR/facts。

## `.ts`、`.js` 和 `.json` 的差异

`.ts`：

- 进入 `etsTransform`，但不执行 `.ets` 专属的 `validateEts` 和 `jsBundlePreProcess`。
- ESMODULE 下仍会跑 `processUISyntax`、`expandAllImportPaths`、`processKitImport`、`collectReservedNameForObf`。
- 在 `processTs/UseTsHar/byteCodeHar` 组合下可能不 emit JS。

`.js`：

- 不进入 `etsTransform`。
- 在 `apiTransform` 中，正常模式只有当它已经是 JS output 或跨平台/widget 分支需要时才会改写。
- `genAbc::transformForModule` 会处理 lazy import。
- `compatibleSdkVersion <= 10` 时上游会用 Babel class-properties plugin 处理 JS，然后保存 source map。

`.json`：

- 不进入 ETS/API transform。
- ESMODULE 下 `genAbc` 会把 JSON 作为 module source 记录，用于 ABC 打包。

## API 和 `lib*.so` 改写

`apiTransform` 有两个 filter：

```text
filterCrossplatform = /(?<!\.d)\.(ets|ts|js)$/
filter = /(?<!\.d)\.(ets|ts)$/
```

分支规则：

- `id.endsWith(".js") || shouldEmitJsFlagById(id) || compileMode !== "esmodule"` 为真时，最终返回 `{ code, map }`。
- `isCrossplatform` 为真时，`.js` 也进入 filter。
- JSBUNDLE 使用 `processSystemApi` + `processLibso`，会把 `@ohos.*/@system.*/@kit.*` import 改成 `globalThis.requireNapi` 或 `globalThis.requireNativeModule`。
- ESMODULE 使用 `processSystemApiAndLibso`，对 `import = require(...)` 改写成 ES import，同时收集 module/component collection。
- `libfoo.so` 会被记录为 use OS file，并在需要时写 `module_collection.json` / preview useOSFiles。

OXC 迁移：

- import declaration、TS import-equals、`require()` 调用优先用 AST 重写。
- 只在上游正则能匹配但 AST 无法表达的边界上使用 span patch。
- `appImportModuleCollection`、`kitModules`、use OS files 不能是 plugin 全局 mutable state；应成为 `TransformFacts`，由 `arkdown` 合并后交给 profile/package。

## ABC 生成分支

SDK `genAbc` 在 Rollup plugin 链内执行，但 arkdown 拆成 `arkdown-ts2abc` 和 `arkdown-abc` 时必须保留这些阶段：

| 上游 hook | 作用 | arkdown |
| --- | --- | --- |
| `buildStart` | 初始化 Ark project config、compiler cache、source map generator、preload modules | `Ts2abcPlan::from_bundle_plan` |
| `transform` | ESMODULE 下收集 TS/ETS/JS/JSON module source；处理 lazy import；API <= 10 JS Babel | `module_source.rs` |
| `moduleParsed` | `singleFileEmit` 时处理单文件 module source | `SingleFileEmitPlan` |
| `beforeBuildEnd` | single file 写声明；否则检查 JS import ArkTS、coverage module source | `PreAbcValidation` |
| `buildEnd` | module ABC 输出 | `ModuleAbcCompiler` |
| `generateBundle` | bundle ABC 输出 | `BundleAbcCompiler` |
| `cleanUp` | 清理 source map、kit import、module mode、shared modules、bytecode obfuscator、preload state | plan-local owned state，禁止全局残留 |

## 版本和模式差异

| 差异项 | 上游条件 | 行为变化 | arkdown owner |
| --- | --- | --- | --- |
| `compileMode=jsbundle` | 默认或 FA/JS bundle | `.ets/.ts` 经 `transpileModule` 直接产 JS；API transform 回写 JS | `arkdown-ets-transform::pipeline` |
| `compileMode=esmodule` | Stage/module 模式 | `.ets/.ts` 可能 emit JS，也可能保留 TS AST/source 给 ABC | `arkdown-ets-transform` + `arkdown-ts2abc` |
| `compileMode=moduleJson` | 上游 rawfile/resource cache 分支 | `etsTransform::buildStart/shouldInvalidCache` 读取 resource cache 信息 | `arkdown-cache` + `ResourceInventory` |
| `minAPIVersion >= 9` | `partialUpdateController` | 开启 partial update；影响 ViewPU 构造参数和 builder check | `VersionGate::partial_update` |
| `minAPIVersion < 10` | partial update config | 关闭 optimize component | `VersionGate::optimize_component` |
| `compatibleSdkVersion <= 10` | `genAbc::transformForModule` | `.js` 经 Babel class-properties plugin | `arkdown-ts2abc::js_compat` |
| metadata `ArkTSPartialUpdate=false` | module metadata | 关闭 partial update，后续可能废弃但必须兼容 | `VersionGate` |
| metadata `ArkTSBuilderCheck=false` 或 local.properties | metadata/local.properties | 关闭 builder check | `CheckerOptions` |
| metadata `Api11ArkTSCheck*` | metadata | 改变 ArkTS linter/兼容检查 | `CheckerOptions` |
| metadata `ArkoalaPlugin=true` | metadata | 使用 Arkoala TS program | `ArkuiBackend::Arkoala` |
| metadata `ArkTSVersion` | metadata | 影响 ArkTS linter state/cache | `VersionGate::arkts_version` |
| metadata `OPTLazyForEach=true` | metadata | LazyForEach 优化 | `UiTransformOptions` |
| metadata `UseTsHar=true` 且 HAR | metadata | HAR 增量输出 `.ts`，不强制 `.js` | `ShouldEmitJsPolicy` |
| metadata `OptTryCatchFunc=false` | metadata | es2abc 增加 `--opt-try-catch-func=false` | `Ts2abcPlan` |
| `byteCodeHar` | loader/build profile | 增量/声明/compile context 和 `otherCompileFiles` 变化 | `arkdown-profile` + `arkdown-ts2abc` |
| `widgetCompile` | widget/card | API transform 组件 collection、独立 cache、ignore warning | `CardTransformPlan` |
| `isCrossplatform` | build option | API transform 覆盖 `.js`，crossplatform checks | `ApiTransformOptions` |
| `isPreview/watchMode/hotReload/coldReload` | stage mode | visual transform、cache invalidation、preview page entry、ABC mode 变化 | `arkdown-cli` stage + `arkdown-watcher` |
| `needCoverageInsert/singleFileEmit` | coverage/single file | `genAbc` module source和声明输出分支 | `arkdown-ts2abc` |

## OXC 迁移顺序

`arkdown-ets-transform` 的 pass 顺序应和 SDK plugin 顺序一致：

```text
SourceClassifier
  -> VisualTransform(.ets only, conditional)
  -> CheckerPrepass(.ets only validate + shared TS/ArkTS rules)
  -> OxcParse(SourceType::ets/ts/js)
  -> ArkuiSyntaxPrepass
  -> ComponentCollector
  -> UiLoweringPass
  -> ImportPathExpandPass
  -> KitImportPass
  -> ResourceReferencePass
  -> ApiTransformPass(system api + libso)
  -> ObfuscationReservedNamePass
  -> LazyImportPass(for module source / ABC)
  -> OxcCodegen or TsForAbc handoff
  -> SourceMapBuilder
```

关键约束：

- OXC parser/AST 是 transform 的唯一主路径；raw string replace 只允许作为上游 regex 兼容的 span patch。
- `.ets` 必须使用 workspace-pinned OXC fork 的 `SourceType::ets()`，保留 `StructStatement` 和 ArkUI component expression。
- TS 语义检查不能靠 transform hook 临时创建全局 Program。`ModuleCompileContext` 必须传入 resolver graph、SDK declaration snapshot、system API table、resource facts、version gate。
- 所有 pass 输出 `TransformFacts`，由 `arkdown` 合并后交给 `arkdown-profile`、`arkdown-ts2abc`、`arkdown-pack`。禁止 plugin 全局 set/map 成为事实来源。
- allocator 生命周期只在单文件 transform 内；跨文件 facts 必须 owned。

## 分支覆盖要求

实现或审计时必须逐项覆盖：

1. module type：HAP、HSP、HAR、shared HSP、bytecode HAR、ohosTest、unit test、preview、reload。
2. source type：`.ets`、`.ts`、`.js`、`.json`、`.d.ts`、`.d.ets`、`.visual`、`lib*.so` import。
3. compile mode：JSBUNDLE、ESMODULE、moduleJson、singleFileEmit。
4. version gate：min API 9/10、compatible API <= 10、ArkTSVersion、UseTsHar、Arkoala、OPTLazyForEach、OptTryCatchFunc。
5. transform output：emitted JS、kept TS/ETS source for ABC、declaration-only、skipped declaration。
6. side outputs：component/module collection、use OS files、source maps、HAR incremental files、intent JSON、reserved obfuscation names、coverage module source。

这些分支对应的文件级 API 在 [crates/arkdown-ets-transform/files.md](crates/arkdown-ets-transform/files.md)、[crates/arkdown-plugin-ets-transform/files.md](crates/arkdown-plugin-ets-transform/files.md)、[crates/arkdown-ts2abc/files.md](crates/arkdown-ts2abc/files.md) 中定义。
