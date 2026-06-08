# Upstream Hvigor Logic Map

本文件只记录上游逻辑总览。crate 级落点见 `crates/<crate>/README.md`。完整流程自洽性审计见 [hvigor-flow-audit.md](hvigor-flow-audit.md)。

## Hook task 到真实产物任务

上游文件：

```text
src/tasks/hook/assemble/assemble-hap.js
src/tasks/hook/assemble/assemble-hsp.js
src/tasks/hook/assemble/assemble-har.js
src/utils/task-initializer.js
src/tasks/manager/*.js
src/tasks/task-creator.js
```

hvigor 中 `assembleHap`、`assembleHsp`、`assembleHar` 是 hook/CA 任务，真实产物由 target task 完成：

```text
assembleHap -> SignHap -> PackageHap
assembleHsp -> SignHsp -> PackageHsp
assembleHar -> PackageHar or PackageSignHar
```

arkdown 的 CLI 不能把 assemble 当作阶段。`arkdown-cli` 应该把命令解析为目标 artifact pipeline，然后由 `BuildRunner` 调用具体 stage。

## HAP/HSP 主链路

上游任务和目标 crate：

| hvigor task | 上游文件 | arkdown crate |
| --- | --- | --- |
| PreBuild | `src/tasks/pre-build.js` | `arkdown-session`, `arkdown-config` |
| BuildProfileTask | `src/tasks/build-profile-task.js` | `arkdown-session`, `arkdown-ohos-schema` |
| GeneratePkgContextInfo | `src/tasks/generate-pkg-context-info.js` | `arkdown-profile` |
| MergeProfile | `src/tasks/merge-profile.js`, `abstract-merge-profile.js` | `arkdown-profile` |
| ProcessProfile | `src/tasks/process-profile.js` | `arkdown-profile` |
| ProcessRouterMap | `src/tasks/process-router-map.js` | `arkdown-profile`, `arkdown-session::ProfileStore` |
| ProcessShareConfig | `src/tasks/process-share-config.js` | `arkdown-profile` |
| ProcessStartupConfig | `src/tasks/process-startup-config.js` | `arkdown-profile` |
| CompileResource | `src/tasks/compile-resource.js`, `abstract/abstract-compile-resource.js` | `arkdown-resource` |
| ProcessResource | `src/tasks/process-resource.js`, `abstract/abstract-process-resource.js` | `arkdown-resource` |
| GenerateLoaderJson | `src/tasks/generate-loader-json.js`, `abstract-generate-loader-json.js` | `arkdown-profile` |
| ArkCompile | `src/tasks/ark-compile.js`, `worker/run-ark.js`, `worker/job.js` | `arkdown-cli`, `arkdown`, `arkdown-ets-transform`, `arkdown-ts2abc` |
| GeneratePkgModuleJson | `src/tasks/generate-pkg-module-json.js` | `arkdown-profile`, `arkdown-pack` |
| ProcessCompiledResources | `src/tasks/process-compiled-resources.js` | `arkdown-resource`, `arkdown-pack` |
| SyscapTransform / PreCheckSyscap | `src/tasks/syscap-transform.js`, `src/tasks/pre-check-syscap.js` | `arkdown-profile`, `arkdown-ohos-schema` |
| GenerateMetadata | `src/tasks/generate-metadata.js`, `abstract/abstract-generate-metadata.js` | `arkdown-profile` |
| CreateModuleInfo | `src/tasks/module-info/*` | `arkdown-profile`, `arkdown-plugin-module-meta` |
| MakePackInfo | `src/tasks/make-pack-info.js`, `abstract-make-pack-info.js` | `arkdown-pack`, `arkdown-ohos-schema` |
| CacheNativeLibs / ProcessLibs | `src/tasks/cache-native-libs.js`, `src/tasks/process-libs.js` | `arkdown-session`, `arkdown-cli`, `arkdown-pack` |
| CollectDebugSymbol | `src/tasks/collect-debug-symbol.js` | `arkdown-cli`, `arkdown-pack` |
| PackingCheck | `src/tasks/packing-check/packing-check.js` | `arkdown-pack`, `arkdown-cli` |
| PackageHap | `src/tasks/package-hap.js`, `pack/pack-hap-request.js` | `arkdown-pack` |
| PackageHsp | `src/tasks/hsp/package-hsp.js` | `arkdown-pack` |
| PrepareSharedHarRes / PackageSharedHar / PackageSharedTgz | `src/tasks/hsp/prepare-shared-har-res.js`, `package-shared-har.js`, `package-shared-tgz.js` | `arkdown-pack` |
| SignHap | `src/tasks/sign-hap.js`, `sign/*` | `arkdown-sign` |
| SignHsp | `src/tasks/hsp/sign-hsp.js`, `sign/*` | `arkdown-sign` |

## HAR 主链路

| hvigor task | 上游文件 | arkdown crate |
| --- | --- | --- |
| ProcessHarArtifacts | `src/tasks/har/process-har-artifacts.js` | `arkdown-pack`, `arkdown-resource` |
| HarBuildProfileTask | `src/tasks/har/har-build-profile-task.js` | `arkdown-session`, `arkdown-ohos-schema` |
| ProcessObfuscationFiles | `src/tasks/har/process-obfuscation-files.js` | `arkdown-obfuscate` |
| ProcessOhPackageJson | `src/tasks/har/process-oh-package-json.js` | `arkdown-profile`, `arkdown-ohos-schema` |
| HarArkCompile | `src/tasks/har/har-ark-compile.js` | `arkdown-cli`, `arkdown`, `arkdown-ts2abc` |
| CreateHarModuleInfo | `src/tasks/module-info/create-har-module-info.js` | `arkdown-profile`, `arkdown-plugin-module-meta` |
| PackageHar | `src/tasks/har/package-har.js`, `abstract/abstract-package-har.js` | `arkdown-pack` |
| PackageSignHar | `src/tasks/har/package-har.js`, signing options | `arkdown-sign` |

## ArkTS/Rolldown 上游链路

上游文件：

```text
node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js
node_modules/@ohos/hvigor-arkts-base/dist/src/build.js
node_modules/@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js
node_modules/@ohos/hvigor-arkts-compose/dist/src/plugins/*.js
node_modules/@ohos/hvigor-arkts-compose/dist/src/plugins/resolve/index.js
```

关键对应：

| 上游逻辑 | 上游文件 | arkdown 落点 |
| --- | --- | --- |
| `runArkPack` 入口 | `arkts-pack.js` | `arkdown-cli::pipeline`, `arkdown::Bundler` |
| rollup config 构造 | `arkts-pack.js::generateConfig` | `arkdown::BundlePlan` |
| SDK compile plugin 加载 | `load-sdk-functions.js` | `arkdown-session::SdkSnapshot`, `arkdown-ets-transform` |
| rollup build/write/cache | `hvigor-arkts-base/dist/src/build.js` | `arkdown::Bundler`, `arkdown-cache` |
| OHPM resolver | `plugins/resolve/index.js` | `arkdown-resolver`, `arkdown-plugin-ohos-resolve` |
| module meta info | `plugins/module-meta-info-plugin.js` | `arkdown-plugin-module-meta`, `arkdown-session` |
| duplicate filename check | `plugins/check-duplicate-filenames-plugin.js` | `arkdown-plugin-duplicate-check` |
| router map plugin | `plugins/router-map-plugin.js` | delete plugin, move to `arkdown-profile` fingerprint |
| sensitive check plugin | `plugins/use-sensitive-check-plugin.js` | delete plugin, move to `arkdown-resolver::case_check` |
| invalid cache plugin | `plugins/invalid-cache-plugin.js` | `arkdown-cache`, `arkdown-session::InputFingerprintSet` |
| collect importers | `plugins/collect-importers-plugin.js` | `arkdown-plugin-ohos-resolve`, post-bundle facts |
| HAR dedup | `plugins/har-deduplicate/*` | `arkdown-plugin-har-dedup`, dependency graph ownership |

## Build config and SDK 上游链路

| 上游逻辑 | 上游文件 | arkdown 落点 |
| --- | --- | --- |
| project plugin 初始化 | `src/plugin/factory/plugin-factory.js` | `arkdown-cli`, `arkdown-session` |
| task registry | `src/utils/task-initializer.js` | `arkdown-cli::BuildRunner` |
| build-profile schema | `src/options/build/*.js`, `src/options/configure/*.js` | `arkdown-ohos-schema` |
| config normalization | `src/common/find-target-product.js`, build option managers | `arkdown-config` |
| SDK discovery | `src/sdk/sdk-util.js`, `default-sdk.js`, loaders | `arkdown-config`, `arkdown-session::SdkSnapshot` |
| PathInfo and build dirs | `src/tasks/sync/ide/*`, `src/const/build-directory-const.js` | `arkdown-path`, `arkdown-session::ArtifactLayout` |
