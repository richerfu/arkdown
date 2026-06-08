# Hvigor Build Flow Audit

本文从上游 hvigor 打包流程重新梳理 arkdown 的 SPEC-048 拆分是否自洽。结论是：主拆分方向成立，但现有子文档必须补齐若干非 happy path 和并行汇合点，否则实现时会把 profile/resource/loader/ArkTS/native/package 的边界重新混在一起。

## 审计结论

当前 crate 拆分的主线是自洽的：

```text
arkdown-cli
  -> arkdown-session
  -> typed plans
  -> profile/resource/bundle/ets-transform/abc/pack/sign/cache
```

但必须修正四类缺口：

1. `GenerateLoaderJson` 和 `CompileResource` 不是父子关系。上游 `ArkCompile` 同时依赖这两个任务，arkdown 的 `BundlePlan` 必须在执行 ArkTS 阶段时汇合 loader_json 与 restool 输出。
2. ArkTS/ArkUI 到 JS/ABC 是一条 SDK plugin 链，不是单独的 `es2abc` 调用。`arkdown-ets-transform`、`arkdown`、`arkdown-ts2abc`、`arkdown-abc` 可以分层，但必须保留 `arkts-pack.js::getBuiltInPlugins` 与 `compile_plugin.js` 中 `etsChecker/etsTransform/apiTransform/genAbc` 的相对顺序。
3. HAP/HSP/HAR 打包前还有 `MakePackInfo`、`CacheNativeLibs`、`GenerateMetadata`、`SyscapTransform`、`ProcessRouterMap`、`ProcessShareConfig`、`ProcessStartupConfig`、`ProcessModulePrivacyProfile`、`ProcessIntegratedHsp`、`ProcessCompiledResources` 等辅助产物任务。这些不是可选文档附录，而是 package plan 的输入。
4. Native、preview、unit test、ohosTest、hot/cold reload、coverage、debug symbol、shared tgz 分支必须有明确归属。SPEC-048 不新增 `arkdown-native`，因此 native 外部工具编排归 `arkdown-cli` stage，native inventory 归 `arkdown-session`，包内 native entries 归 `arkdown-pack`。

## 上游入口

本审计以本机 DevEco Studio 6.0.0.858 内置 hvigor 为准：

```text
/Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/hvigor-ohos-plugin/
  src/plugin/
  src/utils/task-initializer.js
  src/tasks/
  src/tasks/manager/
  node_modules/@ohos/hvigor-arkts-compose/dist/src/
  node_modules/@ohos/hvigor-arkts-base/dist/src/
```

关键文件：

| 上游文件 | 作用 |
| --- | --- |
| `src/plugin/factory/plugin-builder.js` | stage HAP/HSP/HAR plugin 绑定 project/module model、module.json5 运行时配置名、build option map |
| `src/plugin/app-plugin.js` | 创建 `ProjectModelImpl`、初始化 dependency manager 和 project task service |
| `src/plugin/context/impl/ohos-project-context-impl.js` | 暴露 project context API，读取 AppScope/app.json5、project build-profile、oh-package dependencies、SDK details |
| `src/plugin/context/impl/ohos-module-context-impl.js` | 暴露 module context API，读取 module build-profile、module.json5、oh-package dependencies、compilePlugin path |
| `src/utils/task-initializer.js` | 按 HAP/HSP/HAR/module target 注册任务 DAG |
| `src/tasks/manager/arkts-task-initializer.js` | 注册 ArkTS/JS 编译、preview、hot/cold reload、unit/ohosTest 编译任务 |
| `src/tasks/manager/resource-task-initializer.js` | 注册 `ProcessResource`、`CompileResource`、增量 resource 编译 |
| `src/tasks/manager/native-task-initializer.js` | 注册 CMake/Ninja/ProcessLibs/Strip/CacheNativeLibs |
| `src/tasks/manager/global-task-creation-manager.js` | 定义 hook/CA 任务依赖边，例如 assemble、package、sign、process compiled resources |
| `src/tasks/ark-compile.js` | 构造 ArkTS 编译配置，收集 loader/resource/profile/router/obfuscation/bytecode HAR/cache 输入 |
| `src/tasks/worker/run-ark.js` | 提交 ArkTS 编译 work，调用 compose/base 编译链 |
| `node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js` | 构造 Rollup config 和内置 plugin 链 |
| `node_modules/@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js` | 加载 SDK `compile_plugin.js`，取 `etsChecker/etsTransform/apiTransform/genAbc` |
| `node_modules/@ohos/hvigor-arkts-base/dist/src/build.js` | 执行 Rollup read cache、rollup、write/generate、process importees、write cache |

## 启动阶段

Hvigor 启动阶段不是马上编译源码，而是先构建 project/module/target model、dependency manager、task service 和 task DAG。

### 读取的配置和输入

启动阶段会涉及这些输入：

| 输入 | 上游来源 | arkdown 归属 |
| --- | --- | --- |
| project `build-profile.json5` | project build profile loader、project context | `arkdown-config` normalize，`arkdown-session::ProjectSnapshot` 保存 |
| module `build-profile.json5` | module build profile loader、module context | `arkdown-config` normalize，`arkdown-session::ModuleSnapshot` 保存 |
| `AppScope/app.json5` | `OhosProjectContextImpl::getAppJsonOpt` | `arkdown-session::ProjectSnapshot`、`ProfileStore` |
| `src/main/module.json5` | `OhosModuleContextImpl::getModuleJsonOpt` | `arkdown-session::ModuleSnapshot`、`ProfileStore` |
| legacy `src/main/config.json` | legacy task path | `arkdown-session` legacy profile snapshot |
| project/module `oh-package.json5` | `ohPackageLoader` | `arkdown-ohpm` parse，`arkdown-session::PackageSnapshot` 保存 |
| lockfile/oh_modules | dependency manager、OHPM loader | `arkdown-ohpm` + `arkdown-session::DependencyGraphSnapshot` |
| SDK metadata/tools | `SdkInfo`、schema paths、restool、package tool、ets-loader | `arkdown-config` resolve，`arkdown-session::SdkSnapshot` |
| source roots | module source set model、target sourceRoots | `arkdown-session::SourceStore` |
| resource roots/profile refs | module/app resources, base/profile refs | `arkdown-session::ResourceInventory`、`ProfileStore` |
| signing config/material paths | project build profile signing config | `arkdown-config` normalize，`arkdown-session::SigningInputSet` |
| native build config | `externalNativeOptions`、CMake/Ninja dirs | `arkdown-session::NativeBuildInventory`，CLI native stage 执行 |
| mock/test/preview/reload config | mock config、preview injected config、ohosTest/unitTest source roots | `arkdown-session` inventory + `arkdown-cli` specialized stage |

### 对 arkdown 的要求

`arkdown-session::BuildSessionBuilder` 必须一次性读取并缓存上述输入，产出：

```text
ProjectSnapshot
ModuleSnapshot[]
PackageSnapshot[]
DependencyGraphSnapshot
SdkSnapshot
ProfileStore
ResourceInventory
SourceStore
NativeBuildInventory
ArtifactLayout
InputFingerprintSet
ModuleCompileContext[]
```

后续 domain crate 只能消费 typed plan，不能重新查找 `oh_modules`、`src/main/module.json5`、`AppScope/app.json5`、SDK schema、restool、package tool 或 signing material。

## 任务注册 DAG

上游 `TaskInitializer` 按模块类型注册任务：

```text
initializeHap
  -> AssembleHapCA
  -> commonHap
  -> initializeTargetTasks

initializeHsp
  -> AssembleHspCA
  -> GenOnDeviceTestHapCA
  -> commonHap
  -> initializeTargetTasks

initializeHar
  -> AssembleHarCA
  -> LibraryBuildPreviewerResCA
  -> GeneratedBuildProfileCA(isHar=true)
  -> common hook tasks
  -> GenOnDeviceTestHapCA
  -> initializeTargetTasks
```

所有 target 都会注册 common target tasks：

```text
ArkTS/JS compile tasks
Resource tasks
Native tasks
PreBuild
GenerateLoaderJson
ProcessProfile
MergeProfile
BuildProfileTask
Preview/unit/ohosTest helpers
SignProjectRemoteHsp / SignModuleRemoteHsp
PreCheckSyscap
GenerateUnitTestTemplate / GenerateOhosTestTemplate
ProcessRouterMap
ProcessShareConfig
ProcessStartupConfig
GenerateDeviceCoverage
GeneratePkgContextInfo
ProcessIntegratedHsp
GeneratePkgModuleJson
ProcessCompiledResources
CollectDebugSymbol
ProcessModulePrivacyProfile
```

Artifact-specific tasks：

| 模块类型 | 上游注册 | 主产物链 |
| --- | --- | --- |
| Entry/Feature HAP | `PackingCheck`, `PackageHap`, `SignHap` 或 `FastPackageHap` | `PackageHap -> SignHap -> AssembleHap` |
| Shared HSP | `PackageSharedHar`, `PrepareSharedHarRes`, `PackageSharedTgz`, `PackageHsp`, `SignHsp`, `PackingCheck` | `PackageHsp -> SignHsp -> AssembleHsp`，另有 shared tgz 分支 |
| HAR | `ProcessHarArtifacts`, `PackageHar`, `PackageSignHar`, `ProcessObfuscationFiles`, package json tasks, `CreateHarModuleInfo`, `PackingCheckHar` | `ProcessHarArtifacts -> PackageHar/PackageSignHar -> AssembleHar` |

arkdown 不应复制 hvigor 的 task class hierarchy；CLI 应将命令解析为 `BuildPlan`，用 dependency wave 执行模块 pipeline。

## Profile 和元信息链路

这一段负责把原始配置变成后续 resource、ArkTS、package 可消费的中间 JSON 和元信息。

| 上游任务 | 主要输入 | 主要输出 | arkdown 归属 |
| --- | --- | --- | --- |
| `PreBuild` | project/module build-profile、module.json5/config.json、syscap.json、mock、hvigor config、SDK schema | validation result、normalized target context | `arkdown-session` read，`arkdown-config` normalize，`arkdown-ohos-schema` validate |
| `BuildProfileTask` / `HarBuildProfileTask` / `GeneratedBuildProfile` | build profile、target/product、dependency build profiles | generated build-profile intermediates | `arkdown-profile::generated_build_profile` |
| `GeneratePkgContextInfo` | dependency graph、package manifest、bundleName/bundleType、native `.so` inventory | `pkgContextInfo.json` | `arkdown-profile::pkg_context_info`，native inventory 来自 `arkdown-session` |
| `MergeProfile` | AppScope profile、module profile、target overrides、dependency profile | merged module profile | `arkdown-profile::merge_profile` |
| `ProcessProfile` | merged module profile、target device types、SDK Ark VM version | processed `module.json`/legacy `config.json` | `arkdown-profile::process_profile` |
| `ProcessRouterMap` | module router map、local/remote HAR router map、OHM URL context | temp router map + loader router map | `arkdown-profile::router_map` |
| `ProcessShareConfig` | module share config、dependency share config | temp share config | `arkdown-profile::process_share_config` |
| `ProcessStartupConfig` | module startup profile、local/remote HAR startup profile、pkg context、preload `.so` | temp startup config | `arkdown-profile::process_startup_config` |
| `ProcessModulePrivacyProfile` | PAC/privacy profile, SDK PAC schema | processed PAC profile | `arkdown-profile::process_privacy_profile` |
| `SyscapTransform` / `PreCheckSyscap` | syscap.json、target device info、SDK syscap schema | `rpcid.sc` or syscap validation | `arkdown-profile::syscap` |
| `GenerateMetadata` | processed profile、distro filter、related entry modules | metadata files for pack | `arkdown-profile::metadata` |
| `MakePackInfo` | merged/processed profile、module/package summary | `pack.info` | writer in `arkdown-pack::pack_info` fed by profile output |
| `CreateModuleInfo` / `CreateHarModuleInfo` | module profile, package metadata, bytecode HAR metadata | generated module info | `arkdown-profile::module_info` and bundle facts from `arkdown-plugin-module-meta` |
| `GeneratePkgModuleJson` | processed profile + packageHapDepends outputs | package `module.json` | `arkdown-profile::pkg_module_json` writes, `arkdown-pack` consumes |

关键自洽点：

- 读取归 `arkdown-session`，写 profile/meta artifacts 归 `arkdown-profile` 或 `arkdown-pack::pack_info`。
- `GeneratePkgContextInfo` 必须看到 native `.so`，但不应该自己扫描 CMake/Ninja 输出；应从 session/native stage 传入 `NativeLibInventory`。
- Router/share/startup 已经不是 rolldown 插件私有逻辑；它们是 profile artifact 和 fingerprint 的一部分。

## Resource 链路

上游资源处理分两步：

```text
ProcessProfile/MergeProfile
  -> ProcessResource
  -> CompileResource
  -> ProcessCompiledResources
  -> PackageHap/PackageHsp
```

`ProcessResource` 做 restool 配置生成：

- 加入 AppScope resources。HAP 必加，HSP 在 `includeAppScopeRes` 未关闭时加入，ohosTest 模板也可加入。
- 加入 module target resources。
- 加入 HAR/HSP dependency resources。
- 写 `resConfig.json` 和 `optCompression.json`。
- 配置 `resCompileThreads`、`ignoreResourcePattern`、ids_map、resource table path、defined IDs、icon check。

`CompileResource` 调用 restool：

- command 形态是 `restool -l <resConfigFile>` 或 preview/incremental compile-link 命令。
- 输出 `intermediates/res`、`restool_build_resources`、`resources.index`、`ResourceTable`/R 头文件、ids_map。
- 对 preview 会持久化 preview build params，并支持 form widget/module 特殊处理。

`ProcessCompiledResources`：

- 将 compiled resource 输出整理到 package 需要的位置。
- 在 fast build 或 preload system so json 生效时复制已有 resource 目录。

arkdown 归属：

- `arkdown-session::ResourceInventory` 发现资源根、profile refs、dependency resource roots。
- `arkdown-resource::plan/command/compiler/compiled` 生成 restool config、执行 resource compile、整理 compiled resources。
- `arkdown-pack` 只消费 `ResourcePackageEntry`，不重新扫描 resource tree。

## ArkTS/ArkUI 到 JS/ABC 链路

上游分三层：

```text
GenerateLoaderJson
  -> ArkCompile task
     -> worker/run-ark.js
        -> hvigor-arkts-compose::runArkPack
           -> hvigor-arkts-base::build(rollup config)
             -> SDK compile_plugin.js plugins
```

本节只保留主链路。源码入口选择、`.ets/.ts/.js/.json/.d.ts/.d.ets/.visual/lib*.so` 分流、JSBUNDLE/ESMODULE/UseTsHar/bytecode HAR/ArkTSVersion 等细节见 [arkts-transform-audit.md](arkts-transform-audit.md)。上游文件覆盖口径见 [upstream-coverage-manifest.md](upstream-coverage-manifest.md)。

### GenerateLoaderJson

`GenerateLoaderJson` 只依赖 `PreBuild`，输出 `aceLoaderJson` / widget loader json。它会写入：

- `workers`
- `modulePathMap`
- `compileMode`
- `projectRootPath`
- `nodeModulesPath`
- `byteCodeHarInfo`
- `declarationEntry`
- `moduleName`
- `hspNameOhmMap`
- `harNameOhmMap`
- `packageManagerType`
- `compileEntry`
- `otherCompileFiles`
- `dynamicImportLibInfo`
- `routerMap`
- `hspResourcesMap`
- `updateVersionInfo`
- AOT fields
- preview/local test `buildConfigPath`
- hot/cold reload `patchConfig`

注意：`GenerateLoaderJson` 并不依赖 `CompileResource`。上游 `ArkCompile` 同时声明依赖 `GenerateLoaderJson` 和 `CompileResource`，因此二者在 ArkTS 阶段汇合。

### ArkCompile

`ArkCompile` 依赖：

```text
GenerateLoaderJson
CompileResource
CreateBuildProfile / CreateHarBuildProfile (API >= 9)
dependent HAR build profiles
```

`ArkCompile.initDefaultArkCompileConfig` 汇总：

- package brief/source roots，包括 target `sourceRoots`、`src/main`、generated build profile dir。
- project compile model、project package hash、all module paths。
- router map config。
- obfuscation options and hash。
- bytecode HAR dependency keys。
- mock params。
- case sensitive check。
- copyCodeResource options。
- UI transform optimization。
- SDK path、ets-loader path、cache path。
- preload system so file path。
- normalized OHM URL / pkg context related inputs。

### Compose/Base 编译链

`arkts-pack.js::generateConfig` 会：

- 调用 `loadSdkFunctions` 加载 SDK `compile_plugin.js`。
- 构造 Rollup input/output/cache/plugin config。
- 对 ESMODULE 模式启用 `generateModuleGraphOnly`、`disableGenerateBundle`。
- 写入 `isByteCodeHar`、declared files path、bundled dependencies、direct importee 收集、source hash 等选项。

`arkts-pack.js::getBuiltInPlugins` 的顺序：

```text
previewResourceWatcher
copy
common
invalidCache
checkDuplicatedFileNames
moduleMetaInfo
resolve
routerMap
newOhmUrl
sensitiveCheck
commonjs
gc
json
wrapperChunk
waitAsync
collectImporters
SDK plugins
delete cleanConfig
```

`load-sdk-functions.js`：

- 加载 SDK `ets/build-tools/ets-loader/compile_plugin.js`。
- 读取 `entryObj`、`widgetEntryObj`、`globalModulePaths`、`cleanConfig`、`resolveFileExtensions`。
- 若 SDK 提供 `sdkPlugins`，直接使用 SDK plugin set，并可定位 `genAbc` plugin。
- 否则 fallback 为：

```text
watchChangeFiles
etsChecker
etsTransform
apiTransform
coverage buildInstrument
genAbc
terser (release/self obfuscation)
babel (JSBUNDLE)
```

`hvigor-arkts-base::build`：

- 读取 Rollup cache。
- 调用 `rollup(config)`。
- `write` 或 `generate` bundle output。
- `processImportees`。
- 移除 declared/error cache 文件。
- 写回 Rollup cache。

### arkdown 拆分映射

| 上游动作 | arkdown crate |
| --- | --- |
| loader_json 字段组装 | `arkdown-profile::loader_json` |
| ArkTS/ArkUI transform、checker、API transform | `arkdown-ets-transform` via `arkdown-plugin-ets-transform` |
| Rollup/Rolldown bundle | `arkdown` |
| resolve/new OHM/module meta/duplicate/HAR dedup plugins | plugin crates + `arkdown-resolver`/`arkdown-session` fact views |
| SDK `genAbc` 从 TS/JS 生成 ABC | `arkdown-ts2abc` + `arkdown-abc` |
| invalid cache | `arkdown-cache` fed by `InputFingerprintSet` |
| obfuscation config/rules | `arkdown-obfuscate` plus bundle/ts2abc options |

这个拆分成立的前提：

- `BundlePlan` 必须携带 loader_json、resource outputs、build profile outputs、SDK compile plugin info、obfuscation options、bytecode HAR metadata。
- `arkdown-ts2abc` 不能只从 CLI args 推导 record/path；production 必须消费 loader metadata 和 bundle output。
- `arkdown-ets-transform` 不能在 transform hook 内重新读 SDK/plugin/package/profile 文件。

## Native 链路

上游 native tasks：

```text
CmakeConfigure
BuildNativeWithCmake
BuildNativeWithNinja
ProcessLibs
PrepareQuickfix
DoNativeStrip
CacheNativeLibs
CollectDebugSymbol
```

Native 产物影响三个下游：

1. `GeneratePkgContextInfo` 需要 `.so` 信息，为 normalized OHM URL 下的 preload/startup/so context 提供映射。
2. `BasePackHapTask` / `PackageHsp` 需要 stripped libs 目录作为 package tool `--lib-path`。
3. `CollectDebugSymbol` 和 HAR metadata 需要知道是否存在未 strip/已 strip native debug symbol。

SPEC-048 不新增 `arkdown-native`，因此归属为：

| 职责 | arkdown 归属 |
| --- | --- |
| externalNativeOptions 读取、native source/lib inventory | `arkdown-session::NativeBuildInventory` |
| CMake/Ninja/strip/cache libs 外部命令编排 | `arkdown-cli::NativeStage` |
| `.so` package entries、ABI 校验、native debug metadata | `arkdown-pack::native_libs` |
| debug symbol 收集和最终报告 | `arkdown-cli::artifacts` + `arkdown-pack::debug_symbol` plan |

## HAP/HSP 打包链路

上游 `PackageHap` 与 `PackageHsp` 都继承 `BasePackHapTask`。入口文件很薄，真实输入收集和 package tool 参数在 base class。

`BasePackHapTask` 的主要输入：

- stripped libs dir。
- package `module.json`。
- `restool_build_resources`。
- `resources.index`。
- `rpcid.sc`。
- JS assets dir。
- ETS assets dir。
- node_modules assets。
- source maps。
- AOT `.an` output。
- `pack.info`。
- integrated HSP。
- signing config fingerprint。

`packageHapDepends` 的依赖：

```text
MakePackInfo
CacheNativeLibs
GenerateMetadata
SyscapTransform
ProcessRouterMap
ProcessShareConfig
CompileArkTS / BuildArkTS
CompileJS / BuildJS
ProcessIntegratedHsp
OhosTestCopyMockConfigJson (ohosTest only)
HotReloadArkTS / ColdReloadArkTS (reload only)
```

Task creator 还有额外边：

```text
GeneratePkgModuleJson -> ProcessCompiledResources -> PackageHap
PackageHsp -> SignHsp
PackageHap -> SignHap
```

HSP 特有分支：

| 上游任务 | 作用 | arkdown 归属 |
| --- | --- | --- |
| `PrepareSharedHarRes` | 为 interface HAR 准备 oh-package/module.json/resource_str/PAC/obfuscation/local deps/libs | `arkdown-pack::shared_har` plan，resource/profile 输入来自前序 stage |
| `PackageSharedHar` | 通过 npm pack 生成 `.har` | `arkdown-pack::shared_har` |
| `PackageSharedTgz` | release 或 `generateSharedTgz` 下，将签名 HSP 与 shared HAR 打成 `.tgz` | `arkdown-pack::shared_tgz` |
| `PackageHsp` | package tool mode `hsp` | `arkdown-pack::hsp` |
| `SignHsp` | 签名 HSP | `arkdown-sign` |

## HAR 打包链路

HAR 主链路：

```text
HarArkCompile / HarBuildArkTS
ProcessObfuscationFiles
CacheNativeLibs
ProcessOhPackageJson / ProcessPackageJson
ProcessIntegratedHsp
  -> ProcessHarArtifacts
  -> PackageHar
  -> PackageSignHar (when needed)
```

`ProcessHarArtifacts` 负责：

- 拷贝编译后的 ABC 或 ArkTS output。
- 拷贝 declared files。
- 拷贝 bundled dependency resources。
- 拷贝 generated files、PAC json、preload so file。
- 写 oh-package metadata，包括 dependency versions、global dependencies、extra importees、compiled package names、nativeDebugSymbol、customTypes。
- 处理 runtimeOnly、workers、bytecode HAR、dynamic import metadata。

`PackageHar` 继承 `AbstractPackageHar`，最终走 `npm pack` 生成 `.har`。这说明 `arkdown-pack::har` 不能只做 zip writer，还要有 `HarArtifactProcessor` 模拟 `ProcessHarArtifacts` 的 staging directory。

## 签名链路

上游签名：

| 上游任务 | 依赖 | 归属 |
| --- | --- | --- |
| `SignHap` | `PackageHap` | `arkdown-sign` |
| `SignHsp` | `PackageHsp` | `arkdown-sign` |
| `PackageSignHar` | `PackageHar` output and sign config | `arkdown-sign` |
| `SignProjectRemoteHsp` / `SignModuleRemoteHsp` | remote HSP dependency info | `arkdown-sign` with dependency info from session |

`BaseSign` 只应读取 `SignPlan` 中已经解析好的 signing config、certificate、private key、profile、password/material paths。sign crate 不应自己推导 project root 或 build profile path。

## 测试、预览、热更、覆盖率链路

这些分支不是主产物链，但会改变输入和 stage：

| 分支 | 上游任务 | arkdown 归属 |
| --- | --- | --- |
| Preview | `BuildPreviewerRes`, `LibraryBuildPreviewerRes`, `PreviewBuild`, `PreviewArkTS`, `PreviewCompileResource`, `CopyPreviewProfile`, `ReplacePreviewerPage` | `arkdown-cli::PreviewStage` + `arkdown-resource` preview options + `arkdown-watcher` |
| Hot/cold reload | `HotReloadBuild`, `BuildHotReloadResource`, `HotReloadArkTS`, `ColdReloadArkTS` | `arkdown-watcher` invalidation + `arkdown-cli::ReloadStage` |
| Unit test | `UnitTestBuild`, `BuildUnitTestHook`, `UnitTestProcessProfile`, `UnitTestArkTS`, `UnitTestCopyMockConfigJson`, `GenerateUnitTestResult` | `arkdown-cli::TestStage` + profile/resource specialized plans |
| ohosTest | `GenerateOhosTestTemplate`, `OhosTestArkCompile`, `OhosTestCopyMockConfigJson`, `GenerateDeviceCoverage` | `arkdown-cli::TestStage` + coverage stage |
| Coverage | `ReplaceSourceMapInCoverageMode`, `BeforeGenerateDeviceCoverage`, `GenerateDeviceCoverage`, SDK `buildInstrument` | `arkdown-ets-transform` coverage instrumentation + `arkdown-cli::CoverageStage` |

这些分支要求 `BuildPlan` 的 `ArtifactRequest` 不能只是 `hap/hsp/har` 三态，还要携带 `mode = normal | preview | reload | unit_test | ohos_test | coverage`。

## 自洽性矩阵

| 流程 | 上游事实 | 当前拆分状态 | 修正要求 |
| --- | --- | --- | --- |
| 启动配置读取 | project/module context 会读 build-profile、app/module json、oh-package、SDK、dependency graph | 自洽 | `arkdown-session` 必须增加 native/test/preview inventory，read-once |
| task DAG | `TaskInitializer` 注册 common target tasks + artifact tasks | 自洽 | `BuildPlan` 文档补齐 HAP/HSP/HAR/preview/test/reload 分支 |
| loader_json | 只依赖 `PreBuild`，ArkTS 阶段与 resource 输出汇合 | 需要修正表述 | `arkdown-profile::loader_json` 不应等待 resource，但 `BundlePlan` 必须同时消费 loader/resource |
| resource | `ProcessResource -> CompileResource -> ProcessCompiledResources` | 自洽 | `arkdown-resource::compiled` 输出要成为 package plan 输入 |
| ArkTS/ArkUI -> JS | SDK `etsTransform/apiTransform` + Rollup plugin chain | 自洽但风险高 | 插件顺序和 SDK support extensions 必须由 plan 固化 |
| TS/JS -> ABC | SDK `genAbc` 是 Rollup plugin 链一环 | 拆分可行 | `arkdown-ts2abc` 需要从 bundle output + loader metadata 生成 plan |
| obfuscation/cache | ArkCompile 解析 obfuscation options/hash 并影响 cache | 部分自洽 | `InputFingerprintSet` 加入 obfuscation rules/hash、SDK plugin version |
| native | native tasks 影响 pkg context、package libs、debug symbol | 有缺口 | 增加 `NativeBuildInventory`、`NativeStage`、`native_libs`、`debug_symbol` docs |
| HAP/HSP package | `BasePackHapTask` 依赖多项辅助任务，task creator 还有 `ProcessCompiledResources` 边 | 自洽但文档不够细 | pack files 增加 shared_har/shared_tgz/packing_check/debug_symbol |
| HAR package | `ProcessHarArtifacts` staging + `npm pack` | 自洽但文档不够细 | `HarArtifactProcessor` 必须覆盖 oh-package metadata 和 bundled deps |
| signing | package 后签名，sign config 来自 build profile | 自洽 | remote HSP signing plan 需补齐 |
| preview/test/reload | 不是独立小功能，会改 resource/profile/ArkTS/coverage 输入 | 有缺口 | CLI 增加 Preview/Test/Reload/Coverage stages |

## 建议的 arkdown 阶段图

单 module pipeline 应表达为：

```text
Session context
  -> PreBuildValidate
  -> BuildProfile/GeneratedBuildProfile
  -> GeneratePkgContextInfo
  -> MergeProfile
  -> ProcessProfile
  -> ProcessRouterMap / ProcessShareConfig / ProcessStartupConfig / Privacy / Syscap / Metadata
  -> ProcessResource
  -> CompileResource
  -> GenerateLoaderJson
  -> BundlePlan(loader_json + resource outputs + build profile + sdk compile plugin + resolver index)
  -> EtsTransform/Rolldown bundle/plugin facts
  -> Obfuscation
  -> Ts2abc/ABC
  -> NativeStage/CacheNativeLibs
  -> GeneratePkgModuleJson
  -> ProcessCompiledResources
  -> MakePackInfo
  -> ProcessIntegratedHsp
  -> PackageHap/PackageHsp/PackageHar
  -> SignHap/SignHsp/PackageSignHar
  -> PackingCheck
  -> CollectDebugSymbol
```

注意上图是逻辑约束，不是严格串行。可并行的点：

- `GenerateLoaderJson` 与 `ProcessResource/CompileResource` 在 `PreBuild` 后可并行，ArkTS 阶段汇合。
- dependency wave 中不同 module 可并行，但 HAR/HSP dependency 必须先产出 build profile/module info/bytecode HAR metadata。
- package/sign 阶段建议保持模块内顺序，避免同一 archive/signing material 并发写。

## 文档和实现调整清单

必须补到 crate 子文档：

1. `arkdown-session/files.md` 增加 `NativeBuildInventory`、preview/test/reload inventory 字段。
2. `arkdown-profile/files.md` 增加 `generated_build_profile.rs`、`pkg_module_json.rs`、`syscap.rs`、`metadata.rs`、`module_info.rs`。
3. `arkdown-resource/files.md` 明确 `ProcessCompiledResources` 的输出被 `PackageHapCA` 消费。
4. `arkdown-pack/files.md` 增加 `shared_har.rs`、`shared_tgz.rs`、`packing_check.rs`、`debug_symbol.rs`、`package_tool.rs`/`tool_options.rs` 对 `BasePackHapTask` 的完整输入覆盖。
5. `arkdown-sign/files.md` 增加 remote HSP signing plan。
6. `arkdown-cli/files.md` 增加 `native.rs`、`preview.rs`、`test.rs`、`reload.rs`、`coverage.rs`，并声明 CLI 是这些分支唯一编排层。
7. `upstream-hvigor-map.md` 增加本审计链接，作为 crate 文档的总校验入口。

实现时的验收标准：

- 任意 stage plan 都能指出自己对应的 hvigor task、输入 artifact、输出 artifact。
- 没有 domain crate 重新读取 project/module/package/SDK 配置。
- `BuildPlan` 能解释 HAP、HSP、HAR、shared tgz、preview、unit test、ohosTest、coverage、reload 的任务边。
- 修改 `module.json5`、startup/share/router profile、resource、source、SDK compile plugin、obfuscation rules、native lib 时，fingerprint 能定位到受影响 stage。
