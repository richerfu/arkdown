# Upstream Coverage Manifest

本文定义本轮 SPEC-048 文档审计的上游覆盖口径。之前的文档只给了主链路，不能声称完整覆盖；从本文开始，所有 crate 子文档必须能追溯到这里列出的入口、文件图和分支矩阵。

## 覆盖口径

本次覆盖锁定到本机 DevEco Studio 6.0.0.858：

```text
/Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app
```

“完整覆盖”在本文中有精确定义：

1. 对每个打包阶段入口，枚举从入口可达的上游可执行 JS 文件。
2. 对每个可执行 JS 文件，将职责归属到 SPEC-048 目标 crate 或删除目标。
3. 对运行时由配置触发的分支，用分支矩阵列出触发条件、上游位置和 arkdown owner。
4. 对 JSON/schema/declaration/SDK 数据表，不按可执行文件计数，但必须作为 `SdkSnapshot`、`ProfileStore`、`ResourceInventory` 或 `DependencyGraphSnapshot` 输入列入 plan。
5. DevEco 或 SDK 版本变更后，必须重新生成本 manifest；旧 manifest 不能继续用于声称覆盖完整。

## 可执行文件图

| 图 | 入口 | 覆盖文件数 | 覆盖状态 |
| --- | --- | --- | --- |
| hvigor task/plugin graph | `task-initializer.js`、`app-plugin.js`、`plugin-builder.js`、`ark-compile.js`、`generate-loader-json.js`、`process-resource.js`、`compile-resource.js`、`package-hap.js`、`package-hsp.js`、`package-har.js`、`sign-hap.js` | 385 | 已按 profile/resource/ArkTS/package/sign/native/test 分组映射 |
| hvigor ArkTS compose/base graph | `arkts-pack.js`、`build.js` | 76 | 已按 rolldown/bundle/cache/plugin 顺序映射 |
| SDK ets-loader compile plugin graph | `compile_plugin.js` | 84 | 已按 ArkTS/ArkUI transform、API transform、ABC、checker、visual、cache 分支映射 |

hvigor task/plugin graph 的分类计数：

| 上游目录 | 文件数 | 主要归属 |
| --- | --- | --- |
| `tasks*` | 198 | `arkdown-profile`、`arkdown-resource`、`arkdown-pack`、`arkdown-sign`、`arkdown-cli` stages |
| `utils` | 62 | `arkdown-session`、`arkdown-path`、domain crate helpers |
| `model` | 20 | `arkdown-core` snapshot/plan models |
| `builder` | 12 | `arkdown-cli::BuildPlanBuilder` |
| `sdk*` | 25 | `arkdown-config`、`arkdown-session::SdkSnapshot` |
| `project*` | 11 | `arkdown-ohpm`、`arkdown-resolver` |
| `plugin*` | 14 | CLI/plugin boundary docs only；不复制 hvigor class hierarchy |
| `common/const/enum/validate/version/options/distribution` | 43 | schema、typed enums、validation plans |

## SDK compile plugin 84 文件归属

带 `*` 的行表示扫描输出中该目录下的全部文件统一归属到同一 crate 或替代机制，不是未审计省略。

| 文件 | SPEC-048 归属 |
| --- | --- |
| `compile_plugin.js` | `arkdown-cli::BuildPlanBuilder` 固化 plugin 顺序 |
| `main.js` | `arkdown-session::SdkSnapshot`、`ModuleCompileContext` |
| `lib/fast_build/common/init_config.js` | `arkdown-profile::loader_json`、`BuildPlanBuilder` |
| `lib/fast_build/common/process_project_config.js` | `arkdown-session::ProjectSnapshot`、`SdkSnapshot` |
| `lib/fast_build/common/rollup-plugin-watch-change.js` | `arkdown-watcher`、`arkdown-cache` |
| `lib/fast_build/ets_ui/rollup-plugin-ets-checker.js` | `arkdown-ets-transform::checker` |
| `lib/fast_build/ets_ui/rollup-plugin-ets-typescript.js` | `arkdown-ets-transform::pipeline` |
| `lib/fast_build/ets_ui/arkoala-plugin.js` | `arkdown-ets-transform::arkoala` gate |
| `lib/fast_build/visual/rollup-plugin-visual.js` | `arkdown-ets-transform::visual` |
| `lib/process_visual.js`、`codegen/codegen_ets.js` | `arkdown-ets-transform::visual` |
| `lib/validate_ui_syntax.js`、`lib/log_message_collection.js` | `arkdown-ets-transform::checker` |
| `lib/process_ui_syntax.js` | `arkdown-ets-transform::ui_syntax` |
| `lib/process_component_build.js` | `arkdown-ets-transform::component::build_fn` |
| `lib/process_component_class.js` | `arkdown-ets-transform::component::class` |
| `lib/process_component_constructor.js` | `arkdown-ets-transform::component::constructor` |
| `lib/process_component_member.js` | `arkdown-ets-transform::component::member` |
| `lib/process_custom_component.js` | `arkdown-ets-transform::component::custom_component` |
| `lib/process_struct_componentV2.js` | `arkdown-ets-transform::viewv2` |
| `lib/process_sendable.js` | `arkdown-ets-transform::sendable` |
| `lib/process_import.js` | `arkdown-ets-transform::import_path` |
| `lib/import_path_expand.js` | `arkdown-ets-transform::import_path` + `arkdown-resolver` |
| `lib/process_kit_import.js` | `arkdown-ets-transform::kit_import` |
| `lib/process_lazy_import.js` | `arkdown-ets-transform::lazy_import`、`arkdown-ts2abc::module_source` |
| `lib/process_module_files.js`、`lib/process_module_package.js` | `arkdown-resolver`、`arkdown-ts2abc::records` |
| `lib/component_map.js`、`lib/external_component_map.js` | `arkdown-session::SdkSnapshot` + `arkdown-ets-transform::component::metadata` |
| `lib/ark_utils.js`、`lib/utils.js` | split into `arkdown-path`、`arkdown-core`、domain helpers |
| `lib/constant_define.js`、`lib/pre_define.js`、`lib/compile_info.js` | `arkdown-core` constants and `SdkSnapshot` |
| `lib/create_ast_node_utils.js` | OXC AST builder helpers inside `arkdown-ets-transform` |
| `lib/do_arkTS_linter.js`、`lib/ets_checker.js` | `arkdown-ets-transform::checker` |
| `lib/userIntents_parser/*` | `arkdown-profile::insight_intent` + transform facts writer |
| `lib/fast_build/system_api/*` | `arkdown-ets-transform::api_transform` |
| `lib/fast_build/meomry_monitor/*` | not copied; replaced by tracing/metrics in `arkdown-cli` |
| `lib/performance.js` | not copied; replaced by Rust tracing spans |
| `lib/hvigor_error_code/*` | `arkdown-core::diagnostics` |
| `lib/gen_aot.js` | `arkdown-ts2abc::aot` |
| `lib/fast_build/ark_compiler/common/*` | `arkdown-ts2abc::plan`、`arkdown-obfuscate` |
| `lib/fast_build/ark_compiler/cache.js` | `arkdown-cache` |
| `lib/fast_build/ark_compiler/rollup-plugin-gen-abc.js` | `arkdown-ts2abc::compiler` |
| `lib/fast_build/ark_compiler/transform.js` | `arkdown-ts2abc::module_source` |
| `lib/fast_build/ark_compiler/generate_module_abc.js` | `arkdown-ts2abc::module_abc` |
| `lib/fast_build/ark_compiler/generate_bundle_abc.js` | `arkdown-ts2abc::bundle_abc` |
| `lib/fast_build/ark_compiler/generate_sourcemap.js` | `arkdown-ts2abc::source_map` |
| `lib/fast_build/ark_compiler/module/*` | `arkdown-ts2abc::module_mode` |
| `lib/fast_build/ark_compiler/bundle/*` | `arkdown-ts2abc::bundle_mode` |
| `lib/fast_build/ark_compiler/check_import_module.js`、`check_shared_module.js` | `arkdown-ts2abc::preflight` |
| `lib/fast_build/ark_compiler/bytecode_obfuscator.js` | `arkdown-obfuscate` + `arkdown-ts2abc::bytecode_obfuscation` |
| `lib/fast_build/ark_compiler/terser-plugin.js` | `arkdown-obfuscate` |
| `lib/fast_build/ark_compiler/babel-plugin.js` | `arkdown-ts2abc::js_compat` |
| `lib/fast_build/ark_compiler/process_decorator.js` | `arkdown-ets-transform::decorator` |
| `lib/fast_build/ark_compiler/process_mock.js` | `arkdown-cli::TestStage` + `arkdown-session` mock inventory |
| `lib/fast_build/ark_compiler/module/module_preload_file_utils.js` | `arkdown-pack::native_libs` + `arkdown-profile::startup` |
| `lib/fast_build/ark_compiler/logger.js`、`error_code.js`、`utils.js`、`url_config.json` | `arkdown-core` diagnostics, `arkdown-path`, `arkdown-ts2abc::tool_options` |

## ArkTS compose/base 76 文件归属

| 文件组 | 文件数 | SPEC-048 归属 |
| --- | --- | --- |
| `hvigor-arkts-compose/arkts-pack.js` | 1 | `arkdown-cli::BundlePlanBuilder` |
| compose built-in plugins | 26 | `arkdown` + plugin crates |
| compose node-resolve/resolve implementation | 19 | `arkdown-resolver` + `arkdown-plugin-ohos-resolve` |
| compose util/log/hash/source-map replacement | 10 | `arkdown-core`、`arkdown-path`、`arkdown-cache` |
| compose analyzer/perf/data | 6 | `arkdown-cli` tracing + diagnostics |
| `hvigor-arkts-base/build.js` | 1 | `arkdown::BundlerDriver` |
| base build steps | 5 | `arkdown-cache`、`arkdown-cli` cleanup/importee handling |
| base rollup cache handlers | 7 | `arkdown-cache` |
| base util/log/msgpack/project path | 11 | `arkdown-cache`、`arkdown-path`、`arkdown-core` |

## 分支矩阵

| 分支 | 上游触发点 | 覆盖文档 | arkdown owner |
| --- | --- | --- | --- |
| HAP/HSP/HAR task DAG | `task-initializer.js`、manager tasks | `hvigor-flow-audit.md` | `arkdown-cli` |
| startup config 读取 | project/module context、profile loaders | `hvigor-flow-audit.md` | `arkdown-session` |
| `hvigorfile.ts` project/module script | hvigor core project/module script loader、`projectOhosConfigManager.getOverrides/getHooks`、`BuildOptionPath` | `rolldown-oxc-parity-gap-audit.md` | `arkdown-cli::hvigorfile_bridge` + `arkdown-session::hvigor_script` |
| loader entry 收集 | `GenerateLoaderJson` | `arkts-transform-audit.md` | `arkdown-profile::loader_json` |
| resource compile | `ProcessResource`、`CompileResource`、`ProcessCompiledResources` | `hvigor-flow-audit.md` | `arkdown-resource` |
| ArkTS visual | `visualTransform`、`process_visual.js` | `arkts-transform-audit.md` | `arkdown-ets-transform::visual` |
| ArkUI `.ets` lowering | `etsTransform`、`process_ui_syntax.js` | `arkts-transform-audit.md` | `arkdown-ets-transform` |
| `.ts` keep/emitted JS | `getShouldEmitJs`、HAR/UseTsHar/bytecode HAR | `arkts-transform-audit.md` | `arkdown-ets-transform` + `arkdown-ts2abc` |
| `.js` API/Babel/ABC | `apiTransform`、`genAbc::transformForModule` | `arkts-transform-audit.md` | `arkdown-ets-transform::api_transform` + `arkdown-ts2abc` |
| declaration files | `declarationEntry`、HAR incrementals | `arkts-transform-audit.md` | `arkdown-ets-transform::declaration`、`arkdown-pack::har` |
| system API / kit / libso | `rollup-plugin-system-api.js`、`process_kit_import.js` | `arkts-transform-audit.md` | `arkdown-ets-transform::api_transform` |
| ABC bundle/module/single-file | `rollup-plugin-gen-abc.js` | `arkts-transform-audit.md` | `arkdown-ts2abc`、`arkdown-abc` |
| compatible API <= 10 JS transform | `ark_compiler/transform.js` | `arkts-transform-audit.md` | `arkdown-ts2abc::js_compat` |
| partial update / ArkTSVersion | `main.js::partialUpdateController` | `arkts-transform-audit.md` | `arkdown-ets-transform::version` |
| bytecode HAR | `GenerateLoaderJson`、`genAbc` | `hvigor-flow-audit.md`、`arkts-transform-audit.md` | `arkdown-profile`、`arkdown-ts2abc`、`arkdown-pack` |
| native libs | native manager tasks、package tasks、libso transform | `hvigor-flow-audit.md` | `arkdown-cli` + `arkdown-pack` |
| package HAP/HSP/HAR | `BasePackHapTask`、`ProcessHarArtifacts` | `hvigor-flow-audit.md` | `arkdown-pack` |
| sign | `SignHap`、`SignHsp`、`PackageSignHar` | `hvigor-flow-audit.md` | `arkdown-sign` |
| preview/test/reload/coverage | preview/unit/ohosTest/reload/coverage tasks | `hvigor-flow-audit.md` | `arkdown-cli` stages |
| Rollup lifecycle/custom compile plugins | `ArkCompile.getHvigorPluginFile`、`arkts-pack.js::doWrapper/loadCustomCompilePlugins/getPlugins` | `rolldown-oxc-parity-gap-audit.md` | `arkdown` + `arkdown-cli` |
| copy code resource | `copy_resources_util.js::getCopyOptions`、`ArkCompile.copyCodeResource` | `rolldown-oxc-parity-gap-audit.md` | `arkdown-resource` |
| AOT/legacy ABC fallback | `lib/gen_aot.js`、legacy `gen_abc_plugin.js` | `rolldown-oxc-parity-gap-audit.md` | `arkdown-ts2abc` |

## 复核命令

这些命令用于重新生成文件数。输出变化时，必须更新本 manifest 和受影响 crate 文档。

```bash
# SDK compile plugin graph
node meta/tasks/SPEC-048-crate-refactor-plan/scripts/audit-require-graph.js \
  --root /Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/sdk/default/openharmony/ets/build-tools/ets-loader \
  --entry compile_plugin.js

# hvigor ArkTS compose/base graph
node meta/tasks/SPEC-048-crate-refactor-plan/scripts/audit-require-graph.js \
  --root /Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src \
  --root /Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src \
  --entry arkts-pack.js \
  --entry build.js

# hvigor task/plugin graph
node meta/tasks/SPEC-048-crate-refactor-plan/scripts/audit-require-graph.js \
  --root /Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/hvigor-ohos-plugin/src \
  --entry utils/task-initializer.js \
  --entry plugin/app-plugin.js \
  --entry plugin/factory/plugin-builder.js \
  --entry tasks/ark-compile.js \
  --entry tasks/generate-loader-json.js \
  --entry tasks/process-resource.js \
  --entry tasks/compile-resource.js \
  --entry tasks/package-hap.js \
  --entry tasks/hsp/package-hsp.js \
  --entry tasks/har/package-har.js \
  --entry tasks/sign-hap.js
```
