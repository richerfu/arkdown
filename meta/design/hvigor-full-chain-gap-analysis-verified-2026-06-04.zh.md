# hvigor 全链路差异与对齐计划（代码核验版）

日期：2026-06-04

范围：当前仓库 `arkdown` 与 hvigor/OpenHarmony DevEco 构建链路在 HAP/HSP/HAR、ArkTS 转换、ABC、资源、打包、签名、缓存等阶段的差异。本文以当前源码和已有设计文档为准，不把历史文档里的“已完成”标记直接视为事实。

结论：当前实现还不能声明“编译结果与 hvigor 完全一致”。`minimal-hap` 等窄 fixture 已经有较多对齐工作，本轮已修复 share config 覆盖、`crossAppSharedConfig` `$profile` 输入、share/startup declared dependency BFS、startup 绝对路径、HSP `pack.info` shared 语义、资源依赖表、HSP resource map、pack metadata 输入来源、PAC/privacy module 和 project 基础处理链路、PAC SDK schema gate 和当前 PAC schema 子集校验、HAP module PAC 跳过语义、shortcut profile 基础 rewrite、distro filter 资源阶段唯一命名和 module.json metadata 更新、form extension metadata/profile/src 基础校验、formWidgetModule/formExtensionModule 基础模块关联和 API/delivery/混用/依赖基础校验、HSP/HAR permission reason `resource_str` 基础筛选、requestPermissions SDK allPermission/userGrant oracle、`user_grant` 权限必填字段和重复权限校验、依赖 HAR/HSP `metadata/requestPermissions` declared BFS 基础合并、`removePermissions` 依赖过滤、依赖 profile `minAPIVersion/deviceTypes` 基础校验等高优先级问题；但全链路仍存在构建输入模型、DependencyManager remote/cache 节点等价性、CompileArkTS handoff 顺序、ABC 后端、HAR/native/signing/package metadata 等关键缺口。要确保修复后与 hvigor 完全一致，必须先建立可重复的 hvigor oracle 对比门禁，并让 fixture 矩阵的 allowlist 清零。

## 1. 一致性目标

“完全一致”需要同时满足：

- 同一项目、产品、target、build mode、SDK、签名材料下，hvigor 与 arkdown 接受或拒绝相同输入，错误级别一致。
- 所有 hvigor 可观察中间产物一致：`merge_profile`、`process_profile`、`resConfig.json`、资源产物、`loader.json`、`loader_out`、CompileArkTS cache TS、`filesInfo.txt`、`modules.cache`、`npmEntries.txt`、`sourceMaps.map`、importee cache、package metadata。
- 未签名 HAP/HSP/HAR 的 zip entry 顺序、压缩方法、CRC、size、content hash 一致。
- `modules.abc`、`widgets.abc`、`data.abc` 在 debug/release 下按字节一致，或存在经过确认的规范化比较器。
- 签名产物至少通过语义一致校验；如要字节一致，需要控制证书、profile、时间戳、签名块元数据和 zip 对齐。

## 2. 当前高优先级阻塞

### 2.1 本轮已修复/已验证项

| 项目 | 状态 | 覆盖测试 |
| --- | --- | --- |
| `temp-share-config.json` 被静态中间产物覆盖 | 已修复：静态占位不再覆盖 share config；router map 占位也改为仅缺失时写入 | `cargo test -p arkdown-cli test_write_static_intermediates` |
| `crossAppSharedConfig` `$profile` 字符串被丢弃 | 已修复：输入模型区分 `$profile` 与 inline array，ProcessShareConfig 会读取自模块和依赖 HSP profile | `cargo test -p arkdown-core cross_app_shared_config`；`cargo test -p arkdown-profile merge_share_config` |
| release 混淆只作用于 `.js` chunk | 已核验：当前 build 流程同时混淆 CompileArkTS handoff `.ts` inputs | `cargo check -p arkdown-cli` |
| HSP `pack.info` 仍写 `bundleType: app` | 已修复：`pack_hsp()` 强制 `shared`，并验证 summary/packages 两处 `moduleType` | `cargo test -p arkdown-pack hsp` |
| `resConfig.json.dependencies` 和 `hspResourcesMap` 空缺 | 已补测试确认会从 declared HSP `ResourceTable.txt` 生成；`resConfig.json.dependencies` 和 loader `hspResourcesMap` 均支持本地/transitive/scoped HSP 基础场景，并跳过未声明包 | `cargo test -p arkdown-cli resource_config`；`cargo test -p arkdown-profile hsp_resources_map_collects_resource_table` |
| startup `srcEntryAbsolutePath` 路径错误 | 已修复：按 profile 布局推导 `src/main` 根目录，避免 `src/src/main`；重复 task 为 hard error | `cargo test -p arkdown-profile merge_startup_config` |
| profile writer 丢字段 | 已核验/补测试：`requestPermissions`、`definePermissions`、`proxyData`、`appStartup`、`crossAppSharedConfig` 和 module 未知字段保留；app 侧基础 preserve-and-mutate 已补，`sanitizer` 等未知字段保留，`AppScope/app.json5#multiProjects` 按 hvigor 忽略，`build-profile.json5#app.multiProjects` 生效 | `cargo test -p arkdown-profile preserves_unknown_fields_from_module_json5`；`cargo test -p arkdown-profile multi_projects` |
| `module.dependencies` 直接声明未进入 profile | 已补测试确认 `module.json5#module.dependencies` 中有效 HSP 依赖会写入 manifest | `cargo test -p arkdown-profile test_process_profile_maps_hsp_dependencies` |
| pack metadata 版本和 delivery 来源 | 已修复：pack version 读取 AppScope，integrated HSP normalize 使用同一 versionCode，`deliveryWithInstall` 读取 module.json5 | `cargo test -p arkdown-cli test_read_` |
| hvigor parity release 入口实际跑 debug | 已修复：parity harness 显式传递 debug/release mode，并补齐 share/startup/release-obfuscation fixture | `cargo test -p arkdown-cli --test hvigor_parity parity_`；`arkdown build` 验证 3 个新增 fixture |
| release obfuscation fixture 触发 ABC panic | 已修复：ABC 默认参数负数字面量在无 source-tracking 路径下不再对空 source 调 `Span::source_text` | `cargo test -p arkdown-abc constructor_default_negative_parameter`；`cargo run -p arkdown-cli -- build --target hap --mode release --project tests/fixtures/release-obfuscation-hap` |
| PAC/privacy 基础链路缺失 | 已补基础实现：非 HAP 模块读取模块根 `pac.json5`，先按 hvigor 要求检查 SDK `toolchains/paccheck/pacSchema.json` 是否存在，缺失则跳过；schema 存在时按 enabling fields `dataProcess/specialAPIs` 和当前 PAC schema 用到的 `type/enum/const/propertyNames/properties/required/minItems/items/allOf/if/then` 子集校验后复制到 `build/default/intermediates/privacy_profile/default/pac.json5`；HAP 构建额外合并 `AppScope/pac.json5`、非 HAP module PAC 和 `oh_modules` package PAC，输出 `build/outputs/<product>/pac.json`；HAP module PAC 不再作为包内 `module.privacy.json` 写入 | `cargo test -p arkdown-profile privacy_profile`；`cargo test -p arkdown-config pac_schema`；`cargo check -p arkdown-cli` |
| shortcut profile resource rewrite 缺失 | 已补基础实现：按 hvigor `CompileResource.customizeShortcut()` 扫描 `module.metadata[].resource` 指向的已编译输出 profile（缺失时回退默认源码 profile），将 `shortcuts` 中带 `wants` 的项写为 `newShortCutObj`，并把匹配 AppScope bundleName 的 want 重写为当前 product bundleName；target resource directories 中的 shortcut profile 可被处理 | `cargo test -p arkdown-profile shortcut`；`cargo check -p arkdown-cli` |
| target `source.abilities` 覆盖缺失 | 已补基础实现：在 `module.json` 写出阶段按当前 product 选择单一 effective target，将 HAP 的 `target.source.abilities[].icon/label/launchType` 覆盖到同名 ability，并用覆盖后的资源引用解析 `iconId/labelId`；HSP 按 hvigor 仅告警不改 | `cargo test -p arkdown-profile target_source_ability`；`cargo test -p arkdown-profile`；`cargo check -p arkdown-cli` |
| distro filter resource rewrite 缺失 | 已补基础实现：解析 target `config.distributionFilter/distroFilter`，按 API 9/10 字段名写入资源阶段 profile；若源 metadata 已引用含 distro filter 的 profile，则复用并合并；若需要新增 profile，会按 hvigor `uniqueFileName()` 语义避让输出 profile 目录中的同名文件，并在 `module.json` 写出后追加实际 `$profile:<name>` metadata，保证 `process_profile/res` 中间件引用同一文件 | `cargo test -p arkdown-profile distro_filter`；`cargo check -p arkdown-cli`；`cargo run -p arkdown-cli -- build --project tests/fixtures/minimal-hap` |
| `removePermissions` 误删当前模块权限 | 已修复：`module.json` 写出不再过滤当前模块 `requestPermissions`；符合 hvigor `getRemovePermissionSet()` 先从 remove set 移除当前模块已声明权限的行为 | `cargo test -p arkdown-profile current_module_permissions`；`cargo test -p arkdown-profile`；`cargo check -p arkdown-cli` |
| HAR/HSP 依赖 `metadata/requestPermissions` 基础合并缺失 | 已补基础实现：`module.json` 写出阶段按 host/project `oh-package.json5#dependencies` declared BFS 收集本地 HAR/HSP profile，支持直接/传递/scoped 依赖并跳过未声明包；按 hvigor `mergeArrayOptions()` 以 `name` 去重合并 `metadata/requestPermissions`；`removePermissions` 支持 hvigor `{ name }[]` 输入，并只过滤依赖权限，当前模块权限保留 | `cargo test -p arkdown-config remove_permissions`；`cargo test -p arkdown-profile dependency_profile_arrays`；`cargo test -p arkdown-profile higher_min_api`；`cargo test -p arkdown-profile device_type_mismatch` |
| HAR/HSP 依赖 API/device 校验缺失 | 已补基础实现：对参与本地 profile 合并的 HAR/HSP 依赖执行 hvigor `checkHarApiStatus()` 等价校验，`app.minAPIVersion` 缺失或高于当前 compatible API 为 hard error；对 HSP 依赖执行 `checkHarDeviceTypes()` 等价校验，`phone/default` 互通规则已覆盖 | `cargo test -p arkdown-profile higher_min_api`；`cargo test -p arkdown-profile device_type_mismatch` |
| permission reason `resource_str` 基础链路缺失 | 已补基础实现：HSP/HAR 从 `requestPermissions[].reason` 收集 `$string:<name>`，复制并筛选 `src/main/resources/*/element/string.json` 到 `build/default/intermediates/res/default/resource_str`；source HAR 入包为 `src/main/resource/**`；HAP 按 declared BFS 合并依赖 `resource_str/src/main/resource/src/main/resources` 下的 `string.json`，支持直接/传递/scoped 依赖并跳过未声明包，同名 string 保留先写入值 | `cargo test -p arkdown-profile permission_reason`；`cargo test -p arkdown-pack har_contains_permission_reason_resource_str`；`cargo check -p arkdown-cli` |
| `requestPermissions` 预校验缺失 | 已补基础实现：按 hvigor user-grant 清单和 `definePermissions.grantMode` 修正规则校验 HAP 必填 `reason/usedScene`、HSP/HAR 必填 `reason`；拒绝重复 `requestPermissions[].name`；API 12/HarmonyOS 且 SDK `previewer/common/resources/module.json#definePermissions` 可读取时，以 SDK `allPermission/userGrant` oracle 校验请求权限名和 user-grant 集合；SDK oracle 缺失时按 hvigor 回退为仅内置 user-grant 必填字段校验 | `cargo test -p arkdown-profile validate_permissions`；`cargo test -p arkdown-config permission_module`；`cargo check -p arkdown-cli` |
| form extension profile 收集/校验偏差 | 已补基础实现：按 hvigor `TargetTaskService.getFormJsonArr()` 只处理 `type == "form"`，要求 `metadata` 非空且存在带 `resource` 的 `ohos.extension.form`；所有 metadata resource 按 `$profile` 解析并校验 profile 存在；`forms[].src` 缺失、逃出模块根、源文件不存在均为 hard error；loader 不再全量扫描未引用 profile，widget 源收集复用同一逻辑；HAP `formWidgetModule` 会切到关联 HSP 的资源/profile/src 根目录，HSP `formExtensionModule` 会合并关联 HAP 的 form extension metadata，并校验基础反向 mapping、compatible API >= 20、两端 `deliveryWithInstall=true`、HAP form config 混用和卡片包依赖 HSP 基础场景 | `cargo test -p arkdown-profile form_profile`；`cargo test -p arkdown-profile loader_json`；`cargo check -p arkdown-cli` |
| BC HAR root-level `byteCodeHar` 检测缺失 + OHM URL 未生成 | 已修复：`OhPkgMinimal` 同时识别 root-level `byteCodeHar: true`（旧 hvigor 格式）和 `metadata.byteCodeHar: true`（新格式）；新增 `main_entry` 字段和 `ohm_url(bundle_name)` 方法生成 `@bundle:<bundleName>/<packageName>/<mainEntryStem>` 格式 URL；build.rs 将 BC HAR OHM URL 写入 `compiledImportees`；`loader_json.rs` `is_bytecode_har` 检测同步修复；BC HAR fixture `ets/modules.abc` 更新为合法的 60 字节 Panda ABC stub；修复后 2 个 BC HAR 测试去掉 `#[ignore]` | `cargo test -p arkdown-cli test_bc_har_compiled_importees_contains_ohm_url`；`cargo test -p arkdown-cli test_bc_har_loader_json_contains_bytecode_har_info`；`cargo test -p arkdown-profile test_scan_bytecode_har_detects_root_level_bytecodehar` |
| bundle cache manifest 未写入 | 已修复：新增 `BuildCache::write_bundle_manifest` / `read_bundle_manifest` / `bundle_manifest_path`；build.rs 每次打包完毕写 `build/default/cache/bundle/bundle.json`；cache_test.rs 的 `run_bundle_miss` 同步写入；去掉 2 个 bundle cache 测试的 `#[ignore]` | `cargo test -p arkdown-cli --test cache_test` |
| target `buildModeSet` 未解析/未用于目标选择 | 已修复：`RawTarget` 新增 `build_mode_set: Vec<RawBuildMode>` 字段；新建 `arkdown_config::build_profile::pick_effective_raw_target` 和 `select_effective_target`，优先级从"named default"首位改为"具体 buildModeSet 目标优先，catch-all default 其次"，解决 release-only 目标被 debug-all 目标遮蔽的问题；`build.rs`/`module_json_writer.rs`/`process_distro_filter.rs`/`loader_json.rs` 均改用统一实现，去除重复代码 | `cargo test -p arkdown-config test_pick_effective_raw_target_priority`；`cargo test -p arkdown-config test_target_build_mode_set_parsed` |
| target-specific ETS sourceRoots 未写入 pkgBriefInfo | 已修复：`LoaderJson::build_pkg_brief_info` 读取 `build-profile.json5`，当 effective target 非 `default` 且 `src/<target>/ets/` 存在时，将其前置到 `sourceRoots`；`originalSourceRoots` 仍保持 `src/main/ets` | `cargo test -p arkdown-profile test_loader_json_target_ets_src_added_to_source_roots` |
| `pkgContextInfo.json` 未生成 | 已修复：`write_loader_json` 写出 loader.json 后同步写 `build/default/intermediates/pkgContextInfo/default/pkgContextInfo.json`，内容为 `pkgNameToPkgBriefInfo` map；hvigor parity 报告可收集此文件做 stage 比对 | — |
| HAR archive 缺 declaration stubs 和 consumer rules | 已修复：`HarPackInput` 新增 `declarations_dir` 和 `consumer_rules_path` 字段；`pack_har` 将 `.d.ts`/`.d.ets` 从 declarations dir 和 `consumer-rules.txt` 打入 archive 根目录；build.rs 先调 `generate_har_declarations()` 再打包，并把 `consumer-rules.txt` 路径传入 | `cargo test -p arkdown-pack test_har_includes_declaration_stubs`；`cargo test -p arkdown-pack test_har_includes_consumer_rules` |
| `hspNameOhmMap`/`harNameOhmMap` normalized 格式错误 | 已修复：`useNormalizedOHMUrl=true` 时使用 `@normalized:N&<bundleName>&<moduleName>&<entryRelPath>&<version>` 格式；非 normalized 时仍使用 `@bundle:` 格式；新增 `dep_package_version` 读取 `oh-package.json5#version` | `cargo test -p arkdown-profile test_loader_json_hsp_har_ohm_maps_normalized_mode` |
| obfuscation 缓存只 hash 第一条 rule file | 已修复：`TaskInputs::compute_obfuscation_hash` 遍历所有 rule files 并以 null 字节分隔，确保任意 rule file 变更均能使 ABC compile cache 失效 | `cargo test -p arkdown-cli --lib task_cache` |
| `resConfig.json#moduleResources` 忽略 effective target resource dirs | 已修复：新增 `write_resource_config_files_with_dirs`；build.rs 调用时传入 `module_resource_dirs` 输出，使 `moduleResources` 与资源编译器实际使用的目录保持一致 | `cargo test -p arkdown-cli --lib cmd::build::intermediates` |
| Rolldown 模块顺序与 hvigor Map insertion order 不一致 | **无影响**：`ChunkGraph.full_module_ids` 已按 resolve/load 插入顺序维护，等价于 hvigor `getModuleIds()` 返回顺序；`filesInfo.txt`/`modules.cache`/`npmEntries.txt` 顺序一致，无需修复 | — |

### 2.2 仍未闭环项

| 优先级 | 阻塞项 | 当前证据 | 影响 |
| --- | --- | --- | --- |
| P0 | oracle strict gate 矩阵仍未完全闭环 | `crates/arkdown-cli/tests/hvigor_parity.rs` 已覆盖 minimal HAP debug/release、HAR、HSP、share、startup、release obfuscation；测试不再 `#[ignore]`，默认无 hvigor 配置时显式 skip，配置 `ARKDOWN_PARITY_HVIGORW`/`ARKDOWN_PARITY_RUN=1` 会产 JSON report，`ARKDOWN_PARITY_STRICT=1` 会将缺失 hvigor 或 diff 变成 hard failure | 已有基础门禁，但 CI job 仍需显式启用 strict env，fixture matrix 仍未覆盖 ByteCode HAR/runtimeOnly/native/integrated HSP 等全链路 |
| P0 | 构建输入模型不完整 | `arkdown-config` 只解析 product/signing/strictMode/部分 arkOptions/workers/preloadSo/preloadSystemSo/removePermissions/app.multiProjects；AOT `aotCompileMode/apPath/arkOptions.hostPGO` 已补基础解析；product `resOptions.directories`、`includeAppScopeRes`（HAP/HAR false 只告警且不生效，Shared/HSP false 生效）和单 effective target `resource.directories` 已进入资源编译输入 | 完整 target/source-set/effective target/buildModeSet/native 等输入仍有缺口 |
| P0 | profile/resource 后处理仍未完整等价 | share/startup/resource deps 已补；share/startup 依赖 profile 已按 host/project declared dependency BFS 覆盖本地直接/传递/scoped 依赖，并跳过未声明包；ByteCode HAR `META-INF/preload.json#systemPreloadHintStartupTasks` 已按 `preloadSystemSo` 基础合并到 merged startup、资源 profile，并在最终 `module.json` 缺少 `appStartup` 时补 `$profile:startup_config`；privacy/PAC module 和 project 基础链路已补，SDK PAC schema gate 和当前 PAC schema 子集校验已补，project PAC 依赖 PAC/metadata 已按 declared BFS 覆盖本地直接/传递/scoped 依赖并跳过未声明包；shortcut 基础 rewrite 已补；distro filter 已按资源阶段唯一命名并在 `module.json` 写出后追加实际 metadata 引用；form extension metadata/profile/src 基础校验、formWidget/formExtension 基础模块关联以及 API/delivery/混用/依赖基础校验已补；permission reason `resource_str` 基础生成/declared BFS 依赖合并已补；依赖 HAR/HSP `metadata/requestPermissions` 本地 declared BFS 基础合并、removePermissions 依赖过滤和参与合并依赖的 API/device 基础校验已补；requestPermissions SDK allPermission/userGrant oracle、重复权限和必填字段基础校验已补 | DependencyManager remote/cache 严格节点类型和顺序、distro 多 target/strict oracle、form module target source-set/DependencyManager strict oracle、requestPermissions 错误文案/错误级别 strict oracle、PAC ValidateUtil 错误文案/错误级别 oracle、PAC app/package strict oracle 等仍未完整 port，最终 `module.json`、资源 profile 和 package 内容仍可能不等价 |
| P0 | ABC 后端不是 es2abc 等价实现 | `arkdown-abc` 使用自定义 batch cache 和 Rust lowering | 大项目 ABC 字节、debug info、release optimizer、cache/protoBin 不保证一致 |
| P1 | HAR/native/package metadata 不完整 | HSP shared 和基本 pack metadata 已修；HAR 为 source HAR 简化路径；native SO 和 integrated HSP 依赖侧已按本地 declared BFS 补基础路径；integrated HSP 已补 API > 11 hard error gate | HSP/HAR/native/remote HSP/signing 产物与 hvigor 仍可能差异大 |

## 3. 全链路差异矩阵

### 3.1 构建配置与 target 解析

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| 执行 project/module `hvigorfile.ts`，插件可改变任务、profile、rollup 配置 | 只检测并 warning；支持 `arkdown.config.json` 少量 override | 自定义 hvigor 插件逻辑完全不执行 | 定义兼容扩展面；无法执行 TS 时，提供 hvigor effective-config oracle dump 对齐模式 | 使用 hvigorfile 改 ability/profile/rollup 的 fixture 有相同 effective config |
| 合并 product、target、source-set、buildModeSet、target source abilities | 已解析 `RawTarget.name/applyToProducts/source.abilities`；`module.json` 写出阶段已按单一 effective target 覆盖 HAP ability `icon/label/launchType` 并重算资源 ID | 多 target 产物执行模型、完整 `EffectiveTargetConfig`、buildModeSet/sourceOption/target config/source-set 合并仍缺 | 引入 `EffectiveTargetConfig`，按 hvigor 顺序合并所有层，并扩展 CLI/任务模型支持同 product 多 target 输出 | effective target JSON 与 hvigor dump 一致；多 target fixture 产物数和内容一致 |
| `removePermissions` 影响 HAR/HSP 依赖权限合并 | `buildOption.removePermissions` 已支持 hvigor `{ name }[]` 和字符串数组；当前模块权限不会被误删；本地 HAR/HSP `requestPermissions` 会按 declared BFS 收集、按 `name` 去重合并并被 remove set 过滤，支持直接/传递/scoped 依赖且跳过未声明包 | 仍缺 DependencyManager remote/cache 严格节点类型和顺序、全量 strict oracle | 用 hvigor 等价 DependencyGraphService 替换轻量 BFS，并对 permission-heavy fixture 清零 final `module.json` diff | permission-heavy fixture final `module.json` 一致 |
| AOT 选项 `aotCompileMode`、`apPath`、`hostPGO` 写入 loader | 已按 hvigor API9/API10+ 策略生成/省略 `anBuildOutPut/anBuildMode/apPath`；API10+ 默认查模块根 `modules.ap`，存在时复制到 `build/default/intermediates/ap/<module>.ap` 并入包；HAR fallback 会省略 AOT 字段；OpenHarmony ETS SDK 版本不高于 `4.0.8.1` 时会按 hvigor 禁用默认 AOT，`hostPGO=true` 仍走 partial；上游未发现 `hostPGO` 专属 warning 分支 | 仍缺 preview/test/local-test 分支 strict oracle | 补 preview/test AOT fixtures，并纳入 strict parity | AOT fixture `loader.json` 和 package `ap/<module>.ap` 一致 |
| native/CMake、ABI、nativeLib 参与 build/package | 当前模块 `src/main/libs`/`libs` 和 declared BFS 依赖包中的现有 `.so` 会被收集，支持本地直接/传递/scoped 依赖并跳过未声明包 | 无法构建 native 输出，remote/cache native、ABI 策略、compress/preload metadata 仍缺 | 增加 native artifact import/build 模式，按 ABI 收集 | native fixture 包内容和 metadata 一致 |
| `resOptions`、target resource directories、includeAppScopeRes 影响资源输入 | product `buildOption.resOptions.directories` 已生效；`includeAppScopeRes=false` 已按 hvigor 仅对 Shared/HSP 排除 AppScope，HAP/HAR 只告警且仍包含 AppScope；当前单 effective target 的 `resource.directories` 已按模块根解析并替代默认资源目录进入资源编译；无 target resource 时会先回退 `src/<target>/resources`，不存在再回退 `src/main/resources` | 完整 source-set/sourceRoots overlay、多 target 产物、依赖资源集、restool strict config 顺序仍不等价 | 基于 `EffectiveTargetConfig` 统一解析资源目录和 include 规则，写入 restool config 并纳入 oracle | resource target fixture 一致 |

### 3.2 Profile merge、process profile、`module.json`

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| `MergeProfile`/`ProcessProfile` cloneDeep 后原地 mutate，保留未知字段 | `module` 部分已有 preserve-and-mutate；`app` 部分基础 preserve-and-mutate 已补，未知字段如 `sanitizer` 会保留；`AppScope/app.json5#multiProjects` 会按 hvigor 忽略，`build-profile.json5#app.multiProjects` 为 true 时写入 | CLI override、完整 project app overrides、HAP/HAR/ohosTest 分支仍需 strict oracle | 用 effective app config oracle 验证所有 app 字段和顺序，补 CLI override 和 HAR/ohosTest 分支 | app 字段保留 fixture 一致 |
| `module.dependencies` 来自 HSP 依赖 | 直接声明的 `module.json5#module.dependencies` 已映射到 manifest | 由依赖图自动派生的 HSP metadata、transitive/order、removePermissions 交互仍缺 | 从 DependencyGraphService 构造完整 HSP dependencies | HSP dependency fixture 一致 |
| `crossAppSharedConfig` 常见输入是 `$profile:<name>` 字符串 | 已支持 `$profile` 与 inline array，并可读取自模块和依赖 HSP profile | 仍需 strict oracle 证明 package 内最终 profile rewrite 与 hvigor 一致 | 将 share fixture 纳入 strict parity，清零资源/profile diff allowlist | share config fixture 可解析且 final module 引用一致 |
| HAR/HSP 依赖 ability、metadata、permissions 合并 | 本地 HAR/HSP `metadata/requestPermissions` 已按 host/project declared BFS 收集，支持直接/传递/scoped 依赖并跳过未声明包；按 hvigor `name` 去重基础合并；`removePermissions` 已过滤依赖权限；参与合并的依赖 profile 已补 `minAPIVersion/deviceTypes` 基础 hard error 校验 | 依赖 ability、DependencyManager remote/cache 严格节点类型和顺序、全依赖节点 API/device 校验 strict oracle 仍缺 | 在 profile merge 阶段接入 hvigor 等价 DependencyGraphService，并补 ability/API/device 全量校验 | HAR/HSP dep profile fixture 一致 |
| appStartup 经 ProcessStartupConfig 后进入资源 profile 并更新 module 引用 | merge task 已补 `moduleName/startupConfigPath/ohmurl/srcEntryAbsolutePath`，资源 profile rewrite 已补；当 startup/preload 任务来自依赖且原始 `module.json5` 无 `appStartup` 时，最终 `module.json` 会补 `$profile:startup_config` | 仍需 strict oracle 证明资源 profile rewrite、package 内容和 hvigor 完全一致；ArkCompile 生成 `preload/default/preload.json` 的源入口/导入分析仍未 port | 将 startup fixture 纳入 strict parity，补资源阶段 canonical diff | startup fixture package 和 resources 一致 |

### 3.3 Share、startup、privacy、资源后处理

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| `ProcessShareConfig` 写 merged temp，并由资源阶段写 `resources/base/profile/<name>.json` | 静态中间产物已不再覆盖 share config；merge 支持自模块 `$profile` 与依赖 HSP `$profile`；依赖 HSP 已按 host/project declared BFS 收集，支持直接/传递/scoped 依赖并跳过未声明包 | remote/cache HSP、资源阶段最终 profile rewrite 仍缺 strict oracle 证明 | 基于 DependencyGraphService 收集 share sources，并用 package/resource diff 验证 | duplicate URI hard error；share profile 和 final module 一致 |
| `ProcessStartupConfig` 补 `moduleName/startupConfigPath/srcEntryAbsolutePath/ohmurl`，处理 dep 和 preload SO | 已补 4 个基础字段，并修正 source/prebuilt profile 路径推导；本地依赖 startup profile 已按 host/project `oh-package.json5#dependencies` BFS 收集，支持直接/传递依赖并跳过未声明包；`buildOption.preloadSystemSo` 已解析，HAP 非 Preview/Test 场景会按 ByteCode HAR `META-INF/preload.json#systemPreloadHintStartupTasks` 合并到 merged startup config 和资源 profile，且会触发最终 `module.appStartup` 补齐 | remote/cache dep startup、remote ByteCode HAR 标记、ArkCompile `needPreloadSoFileSet` 到 `preload/default/preload.json` 的 compiler 产物、preload 严格依赖顺序、资源阶段最终内容仍未完整证明 | 基于依赖图补 dep/preload 规则，并加入 startup resource/package oracle | temp startup JSON 与 hvigor 一致 |
| resource 阶段处理 permission reason、shortcut、distro、form、share/startup/profile rewrite | HSP/HAR 本模块 permission reason `resource_str` 基础筛选已补，source HAR 会打入 `src/main/resource/**`；HAP 会按 declared BFS 合并依赖 `resource_str/src/main/resource/src/main/resources` 的 string 资源；shortcut profile 会按 `newShortCutObj` 重写并替换 product bundleName，且优先处理资源编译后的输出 profile，覆盖 target resource directory 场景；distro filter profile 已移动到 `module.json` 写出后的资源阶段，新增 profile 时会避让输出目录同名文件并追加实际 metadata 引用，已有 metadata profile 会复用并合并；form extension 已按 `getFormJsonArr()/validateFormSrc()` 基础规则校验 metadata/profile/src，并停止 loader 全量扫描未引用 profile，HAP/HSP form 模块关联、API 版本、delivery、HAP form profile 混用、HSP 依赖基础校验已补；share/startup 本地依赖已按 declared BFS 基础收集；requestPermissions SDK allPermission/userGrant oracle、必填字段和重复权限基础校验已补；资源编译主要复制 profile/media/element，router 有单独处理 | DependencyManager remote/cache 严格节点类型和顺序、distro 多 target 校验/strict oracle、form module target source-set/DependencyManager strict oracle、requestPermissions/shortcut 宽场景 strict oracle 等仍缺 | port `MergeProfile.processResourceStr` 剩余校验和 `CompileResource` 后处理逻辑 | resource-heavy fixture `res/default` 一致 |
| `ProcessModulePrivacyProfile` / `ProcessProjectPrivacyProfile` 验证并复制/合并 privacy/PAC | 已读取非 HAP 模块根 `pac.json5`，按 hvigor `pacFileValidate()` 先检查 SDK PAC schema 是否存在，缺失则跳过；schema 存在时执行 `dataProcess/specialAPIs` enablement 和当前 PAC schema 用到的 draft-07 子集校验（含 enum/minItems/required/if-then），再复制到 `build/default/intermediates/privacy_profile/default/pac.json5`；HAP module PAC 按 hvigor 跳过；HAP 构建会合并 `AppScope/pac.json5`、非 HAP module PAC、declared BFS package PAC 与 package metadata 到 `build/outputs/<product>/pac.json`，支持本地直接/传递/scoped 依赖并跳过未声明包 | 仍缺 ValidateUtil 错误文案/错误级别 oracle、DependencyManager remote/cache 严格节点类型和顺序、`.app` package `--pac-json-path` strict oracle、HSP/HAR module PAC 包装 oracle | 补全 hvigor `pacFileValidate` 错误输出和 DependencyManager PAC 收集规则，并纳入 strict package/app diff | privacy/PAC fixture 一致 |
| restool config 包含 dependency resources | 已按 host/project `oh-package.json5#dependencies` BFS 从本地 HSP 依赖 `ResourceTable.txt` 生成 `resConfig.json.dependencies`，支持直接/传递/scoped 依赖并跳过未声明包；loader 也按 declared BFS 收集 `hspResourcesMap` | remote/cache 依赖路径、严格 DependencyManager 顺序和 target intermediates `ResourceTable.txt` 仍未由 hvigor 等价依赖图统一驱动 | DependencyGraphService 提供资源表和依赖资源目录，覆盖 remote/cache/target 场景 | HSP resource lookup fixture 一致 |

### 3.4 `loader.json` 与依赖图

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| DependencyManager 分类 local/remote HAR、HSP、ByteCode HAR、SO、dynamic、bundled、transitive | 主要通过 `oh_modules` 文件系统扫描和轻量 BFS | 类型、顺序、remote cache 路径、bundled subtleties 不完整 | 建立 hvigor 等价 `DependencyGraphService` | dependency graph dump 与 hvigor 一致 |
| `modulePathMap/pkgBriefInfo` 来自完整 target module graph | `modulePathMap` 和内部 `pkgBriefInfo` 已包含 workspace modules，并按 declared dependency BFS 补本地/transitive/scoped 依赖包；`pkgBriefInfo` 对有 `src/main/ets` 的源码包生成基础 source root | remote/cache package context、target-specific pkgContextInfo、sourceRoots/originalSourceRoots 严格语义和字段顺序仍不完整 | 从 dependency graph 生成所有 package context，并输出/对比 hvigor `pkgContextInfo.json` | `pkgContextInfo` 和 loader map 一致 |
| `hspNameOhmMap/harNameOhmMap/hspResourcesMap` 基于 DependencyManager | 三个 map 都已改为 declared dependency BFS 基础生成，可覆盖 transitive/scoped HSP/HAR；`hspResourcesMap` 和 `resConfig.json.dependencies` 可收集 HSP 根目录 `ResourceTable.txt` | 本地 HSP target intermediates `ResourceTable.txt`、remote/cache 严格路径、依赖顺序、normalized-off 模式仍未证明 | 统一由 dependency graph、target service 和 OHM URL service 生成 | HSP/HAR transitive fixture 一致 |
| `compileEntry` 包含 abilities、pages、workers、runtimeOnly、startup、insight intent、dep ability、router 等 | 覆盖 abilities/pages/workers/runtimeOnly/router/otherCompileFiles；依赖 `metadata.runtimeOnly.sources/packages` 已按 host/project declared dependency BFS 覆盖本地直接/传递/scoped 依赖，并跳过未声明包；startup `configEntry` 与 temp startup `srcEntryAbsolutePath` 已写入 serialized `compileEntry`；自模块 `insight_intent.json` 的 `insightIntents/insightIntentsSrcEntry.srcEntry` 已写入 serialized `compileEntry`；Preview/Test 目标已按 hvigor `collectMockFileEntry()` 读取 `<module>/src/mock/mock-config.json5`，并把其中 `source` 相对模块根目录解析后写入 serialized `compileEntry`；本地 source HAR 依赖 `abilities/extensionAbilities.srcEntry` 已按存在文件写入 serialized `compileEntry`，并跳过 shared HSP/ByteCode HAR；本地 source HAR 依赖 `insight_intent.json#insightIntentsSrcEntry.srcEntry` 已按资源目录写入 serialized `compileEntry`，支持 `metadata.resource.directories` 和默认 `src/main/resources` | remote/cache HAR dep ability/insight intent、bundled deps、local-test 注入态、严格依赖顺序等仍未完整 | 完整 port `GenerateLoaderJson.doTaskAction()` entry 收集，并由 DependencyGraphService 驱动依赖侧入口 | compileEntry canonical diff 一致 |
| ByteCode HAR loader info、declarationEntry、updateVersionInfo、otherCompileFiles | 已有扫描和部分字段 | 仍非 hvigor DependencyManager；own declaration、runtimeOnly ByteCode HAR、bundled edge 未证明 | 从 graph 节点生成 ByteCode HAR 所有字段 | ByteCode HAR fixture loader 和 runtime install 通过 |
| AOT fields、patch config、test/preview fields | AOT 字段已按 hvigor 基础策略 emit/omit，并复制/打包 `.ap`；OpenHarmony fixed-SDK gating 已按 ETS SDK 版本 `>4.0.8.1` 判定；`build/default/config/buildConfig.json#patchConfig` 已按非 ohosTest 条件透传到 loader，Preview 目标会写相对 `buildConfigPath`；上游未发现 `hostPGO` 专属 warning 分支 | local-test 注入状态、preview/test strict oracle 仍不完整 | 补 local-test 配置模型与 preview/test AOT fixtures，并纳入 strict parity | AOT/preview/test fixture 一致 |

### 3.5 Rollup/Rolldown、ArkTS transform、CompileArkTS handoff

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| Rollup 3 + hvigor plugin stack，module order 为 JS `Map` insertion order | Rolldown 1.0.3；`ChunkGraph.full_module_ids` 以 resolve/load 插入顺序填充，等价于 hvigor 的 `getModuleIds()` Map 顺序 | **无影响**：Rolldown `full_module_ids` 已按插入顺序维护，`filesInfo.txt/modules.cache/npmEntries.txt` 顺序与 hvigor Map 顺序等价；无需额外修复 | N/A | N/A |
| SDK etsChecker 做 TypeScript 语义和 ArkTS 结构校验 | Rust checker 覆盖部分结构规则 | TS 类型诊断不等价 | 对已知规则补齐；必要时引入 TS service/oracle 模式 | invalid TS/ArkTS fixture 错误一致 |
| SDK etsTransform 由 TypeScript AST printer 输出 | Rust/OXC + 部分字符串/AST 生成 | ViewPU、ComponentV2、paramsLambda、printer/comment/source-map 宽覆盖未证明 | 以 cache TS golden diff 驱动补齐；减少手写字符串 | cache TS 和 source map fixture 一致 |
| hvigorfile 自定义 rollup plugin 可参与 transform | 不执行 TS plugin | 自定义项目不等价 | 提供受控插件兼容层或拒绝并给出等价配置 | custom plugin fixture 行为一致或明确失败 |
| CompileArkTS 以 cache TS + `filesInfo.txt` 为最终 ABC 输入 | 已使用 handoff inputs | 需要证明所有 fixture 的 handoff 内容、路径、order、map 均一致 | 扩展 oracle 收集 cache root 所有文件 | HmosWorld 等真实项目 handoff 一致 |

### 3.6 ABC、widget、ArkData

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| es2abc 原生 parser/lowering/emitter/optimizer | `arkdown-abc` 自定义 Rust backend | 未完全替代 es2abc；仍有 unsupported syntax 和 silent-risk | 对照 upstream es2panda 补 lowering；unsupported 必须 hard error | syntax matrix 与 es2abc 一致 |
| `--cache-file @modules.cache` per-file ProgramCache/protoBin | 自定义 batch ABC cache，`ARKD` protoBin | cache/protoBin 不兼容 hvigor | 实现 upstream ProgramCache/protoBin 或 parity 模式禁用 cache | cache artifact 与 hvigor 一致或均不存在 |
| debug info/LNP 由 es2abc 源位置事件生成 | minimal fixture 已大量对齐；宽覆盖未证明 | 大项目 debugger/source map 可能差异 | ABC section diff + byte golden 扩展到真实 fixture | `modules.abc/widgets.abc` 字节一致 |
| release optimizer `opt_level=2` | 传 opt_level，但后端优化不等价 | release ABC 差异 | port 必要 optimizer/register allocation，或 release parity 模式调用兼容 backend | release ABC 一致 |
| widget/form 独立 compile config/cache | `compile_widget_pass()` 存在；form profile metadata/src 基础校验已复用 hvigor `getFormJsonArr()/validateFormSrc()` 规则；HAP `formWidgetModule` 与 HSP `formExtensionModule` 的基础资源/source 根目录、metadata 来源、API/delivery/混用/依赖基础校验已补 | widget loader/cache/ABC 字节未证明；form module target source-set/DependencyManager strict oracle 未覆盖 | 使用同一 handoff/order 规则，补 widget strict oracle 和 target form module 变体 | widget fixture `widgets.abc` 一致 |
| ArkData 独立 compile | `compile_arkdata_pass()` 存在 | 真实 ArkData schema/diagnostic 未证明 | 增加 ArkData fixture 和 oracle | `data.abc` 一致 |

### 3.7 Release、混淆、minify

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| release treeshake true，supportChunks 时 manualChunks | treeshake 已按 mode；supportChunks 有路径 | 仍需 hvigor fixture 证明 side-effect/module chunk 行为 | 加 release chunk golden | chunk list/handoff 一致 |
| SDK obfuscation/minifier 作用于最终 compile inputs | 已同时混淆 `.js` chunk 和 CompileArkTS handoff `.ts` inputs，release fixture 可成功构建 | 仍需 hvigor oracle 证明 rule 行为、name map、cache invalidation 和 release ABC 字符串/符号一致 | 将 release-obfuscation fixture 纳入 strict parity，补 obfuscation rule/name map diff | release ABC string/name diff 证明已混淆 |
| obfuscation rule engine 与 consumer config | Rust subset，HAR consumer rules 只是拼接 | 规则语义、order、cache invalidation 不等价 | port SDK rule 行为或 parity 模式代理 upstream | obfuscation fixture 一致 |
| terser/JSBUNDLE 特定插件 | ESMODULE 为主，JSBUNDLE 不支持 | JSBUNDLE/legacy 项目不等价 | 明确 unsupported 或实现 JSBUNDLE parity | JSBUNDLE fixture 一致或拒绝一致 |

### 3.8 Packaging、HSP/HAR、native、signing

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| HAP/HSP/HAR 按不同任务生成 pack info、metadata、archive entries | HAP 较完整；HSP 复用 HAP；HAR 是 source HAR 简化 | HSP/HAR 专项产物未对齐 | 分 target 建 `PackInput`，从 effective app/module 生成 pack info | HAP/HSP/HAR archive diff 一致 |
| HSP `pack.info` moduleType/shared、bundleType/shared | 已在 HSP pack 路径强制 `bundleType: shared`，summary/packages `moduleType` 为 `shared` | 仍需 shared HSP fixture 的 archive-level strict parity | 将 minimal/shared HSP 纳入 strict pack diff | minimal/shared HSP fixture 一致 |
| pack.info 来自 app/module/profile/deps | version 和 `deliveryWithInstall` 已从 AppScope/module profile 读取 | deviceTypes、target overlay、deps 元数据等仍未完整 | 从 effective app/module、deps、target 生成全部字段 | pack.info diff 一致 |
| ByteCode HAR/source HAR/HAR declarations/obfuscation files | source HAR 基础路径；声明 stub 简化 | HAR 发布不等价 | 区分 source HAR 与 ByteCode HAR；补 declaration、consumer obfuscation、cleanEtsForTgz | HAR matrix 一致 |
| integrated HSP | 有 normalize 和 pack；已按 host/project declared BFS 收集本地直接/传递/scoped 依赖并跳过未声明包；已补 hvigor `limitMinApiVersion(..., API_VERSION_11)` 等价 gate，compile/compatible API 不大于 11 时 hard error | 仍以 package `metadata.isIntegratedHsp` + `<name>.hsp` 简化识别，remote HSP cache、签名状态、打包工具 `packageNormalize` 命令级行为和完整 DependencyManager 节点仍不等价 | 从 DependencyGraphService/remote HSP map 解析 integrated HSP，补 remote/cache/signing cache 和 strict archive oracle | integrated HSP fixture 一致 |
| native SO | 收集当前模块本地 `arm64-v8a/*.so`，依赖侧按 declared BFS 收集本地直接/传递/scoped 包内现有 `.so` 并跳过未声明包 | native build、remote/cache native、ABI、compressNativeLibs、preload metadata 不完整 | 补 native build/import task 和 metadata | native/preload fixture 一致 |
| signing | Rust signer实现，未签名 copy fallback | 字节级签名不保证；hvigor hap-sign-tool 细节未 oracle | 分离 unsigned byte parity 与 signed semantic parity | 签名 verifier 通过；可选字节一致 |

### 3.9 缓存、watch、preview、CI

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| task input cache、Rollup msgpack cache、invalid-cache plugin、es2abc cache | `TaskInputs` coarse skip、ETS transform JSON cache、ABC batch cache | skip 语义和 stale cleanup 不等价 | 按任务声明输入和输出；cache key 与 hvigor compileEnv 对齐 | incremental scenario 与 hvigor 一致 |
| watch/hot reload/preview IDE protocol | watch 有全量 rebuild 和简单 importer map；preview 写文件 | DevEco IDE WebSocket/资源热更新协议缺失 | 明确 scope 或补 IDE protocol | watch/preview fixture 一致 |
| CI parity gate | ignored tests，需要本地手动跑 | 不能阻断回归 | 增加 SDK 可用时运行的 parity job + allowlist | allowlist 清零前不能宣称 full parity |

## 4. 修复与对齐计划

### Phase 0：先建 hvigor oracle 门禁

交付：

- 已完成基础：`hvigor_parity.rs` 默认参与 `cargo test`，无本地 hvigor 配置时 skip；设置 `ARKDOWN_PARITY_HVIGORW`/`ARKDOWN_PARITY_RUN=1` 同时 build hvigor 和 arkdown 并输出 report；设置 `ARKDOWN_PARITY_STRICT=1` 后差异或 hvigor 缺失均 hard fail。
- 收集 stage artifacts：profile、resource、loader、CompileArkTS cache、source maps、ABC、zip、pack metadata、signing metadata。
- 提供 JSON canonical diff、zip diff、ABC section diff、cache TS diff、resource table diff。
- fixture matrix：minimal HAP debug/release、HSP、source HAR、ByteCode HAR、runtimeOnly、dynamic import、router、share、startup、privacy/PAC、widget/form、ArkData、native/preload SO、integrated HSP、normalized OHM on/off、release obfuscation、supportChunks、target overlay、removePermissions。

验收：CI 显式设置 `ARKDOWN_PARITY_STRICT=1` 后能输出阶段级 diff；allowlist 有 owner 和过期时间。

### Phase 1：修配置和 profile 架构

任务：

- 已完成基础：HAP `target.source.abilities` 的 `icon/label/launchType` 单 target 覆盖和资源 ID 重算。
- 建 `EffectiveTargetConfig`，合并 product/target/source-set/buildModeSet，并支持同 product 多 target 产物；已完成基础：单 effective target `resource.directories` 作为当前模块资源输入。
- 已完成基础：module profile preserve-and-mutate；app profile 基础 preserve-and-mutate，`multiProjects` 来源按 hvigor 从 build-profile 顶层 app 读取并忽略 AppScope 同名字段。
- 剩余：完整 project app overrides、CLI override、HAR/ohosTest app 分支和字段顺序 strict oracle。
- 支持 `crossAppSharedConfig` 的 `$profile` 字符串和 inline array。
- 已完成基础：`removePermissions` 支持 hvigor `{ name }[]` 输入，当前模块权限不误删，本地 HAR/HSP 依赖 `requestPermissions/metadata` 按 declared BFS 直接/传递/scoped 收集、按 `name` 去重合并并过滤依赖权限，并对参与合并的依赖 profile 校验 `minAPIVersion/deviceTypes`。
- 剩余：完整 `EffectiveTargetConfig`、DependencyManager 驱动的 HSP dependencies、HAR ability、全依赖节点 API/device 校验和 remote/cache dep profile merge。

验收：profile-heavy fixture 的 `merge_profile/process_profile/module.json/ark_module.json` 与 hvigor 一致。

### Phase 2：修 resource-phase 后处理

任务：

- 已完成：删除 `temp-share-config.json` 被静态空文件覆盖的问题。
- 已完成基础：将 share/startup merged temp 写入 profile 流程，并补 startup `ohmurl/srcEntryAbsolutePath/moduleName/startupConfigPath`；share/startup 依赖 profile 已按 declared dependency BFS 覆盖本地直接/传递/scoped 依赖；按 `preloadSystemSo` 合并 ByteCode HAR `META-INF/preload.json#systemPreloadHintStartupTasks`。
- 已完成基础：非 HAP 模块根 `pac.json5` 处理到 hvigor-style PAC intermediate；SDK PAC schema 缺失时按 hvigor 跳过，schema 存在时执行 enablement 和当前 PAC schema 子集校验；HAP 模块按 hvigor 跳过 module PAC；HAP 构建合并 project-level PAC 到 `build/outputs/<product>/pac.json`，依赖 package PAC/metadata 按 declared BFS 支持本地直接/传递/scoped 依赖并跳过未声明包。
- 已完成基础：shortcut profile 按 hvigor `newShortCutObj` 规则重写，并将匹配 AppScope bundleName 的 want 替换为 product bundleName；重写源优先使用资源编译输出 profile，可覆盖 target resource directory 输入。
- 已完成基础：target `config.distributionFilter/distroFilter` 写入 distro filter resource profile；已有 metadata profile 会复用并合并；新增 profile 会按 hvigor `uniqueFileName()` 语义避让输出目录同名文件，并在 `module.json` 写出后的资源阶段追加实际 `$profile` 引用。
- 已完成基础：form extension profile 按 hvigor `getFormJsonArr()` 要求校验 `metadata`、`ohos.extension.form`、profile 文件存在，并按 `validateFormSrc()` 校验 `forms[].src` 缺失/逃逸/不存在；loader 不再扫描未引用 profile，widget 源收集复用同一套规则；HAP `formWidgetModule` 和 HSP `formExtensionModule` 的基础资源根切换、metadata 合并、反向 mapping、API >= 20、deliveryWithInstall、HAP form config 混用、HSP 依赖 HSP 基础校验已补。
- 已完成基础：permission reason `$string` 资源筛选到 `resource_str` intermediate，source HAR 中打入 `src/main/resource/**`，HAP 按 declared BFS 合并依赖 string 资源且同名保留先写入值，支持本地直接/传递/scoped 依赖并跳过未声明包。
- 已完成基础：`requestPermissions` user-grant 必填字段、重复权限、`definePermissions` grantMode 修正，以及 SDK `previewer/common/resources/module.json#definePermissions` 驱动的 allPermission/userGrant oracle 校验。
- 已完成基础：`removePermissions` 不再误删当前模块 `requestPermissions`；本地 `oh_modules` 依赖权限已在 profile 写出时过滤。
- 剩余：用 strict oracle 证明 share/startup merged profile、final `module.json` 和 package 内容与 hvigor 一致。
- 补 DependencyManager remote/cache 严格节点类型和顺序、distro 多 target 校验/strict oracle、form module target source-set/DependencyManager strict oracle、requestPermissions/shortcut strict oracle、PAC ValidateUtil 错误文案/strict app package oracle、ArkCompile `needPreloadSoFileSet`/`preload.json` strict oracle。
- 已完成基础：`resConfig.json.dependencies` 按 declared dependency BFS 从本地 HSP `ResourceTable.txt` 生成，支持直接/传递/scoped 依赖并跳过未声明包；剩余 remote/cache/target intermediates 由 DependencyGraphService 统一驱动。

验收：share/startup/privacy/resource-heavy fixtures 的资源目录、profile 引用和 package 内容一致。

### Phase 3：建立 hvigor 等价依赖图并重写 loader 生成

任务：

- 引入 `DependencyGraphService`，节点区分 local/remote HAR、HSP、ByteCode HAR、SO、dynamic、runtimeOnly、bundled、transitive。
- 已完成基础：startup `configEntry/srcEntryAbsolutePath`、自模块 insight intent srcEntry、Preview/Test mock-config source、本地 source HAR dep ability/extensionAbility srcEntry、本地 source HAR dep insight intent srcEntry 进入 serialized `compileEntry`。
- 所有依赖侧 `loader.json` map、dep compile entries、pkg brief info、ByteCode HAR fields 都从该图生成；AOT fields 继续由 effective build option + SDK API 策略生成，并接入 oracle。
- 提供 normalized 和 old OHM URL 两套 golden。

验收：`loader.json/pkgContextInfo/importee/filesInfo` 在 dependency matrix 中一致。

### Phase 4：稳定 CompileArkTS handoff

任务：

- 显式记录 hvigor Rollup Map 等价的模块插入顺序。
- 以 cache TS/source map golden diff 补齐 ViewPU、ComponentV2、paramsLambda、printer/comment、diagnostics。
- 已完成基础：把 release obfuscation 同时作用到最终 handoff inputs 和临时 chunks。
- 剩余：对齐 SDK rule engine、name map、consumer config、cache invalidation，并用 release ABC diff 证明。

验收：真实项目如 HmosWorld 的 `filesInfo.txt/modules.cache/npmEntries.txt/cache TS/sourceMaps.map` 一致。

### Phase 5：ABC 后端闭环

任务：

- 实现 upstream ProgramCache/protoBin 或 parity mode 禁用 cache。
- 补 `merge_abc=false`、release optimizer、unsupported syntax、ABC validation。
- 扩展 byte-level golden：main ABC、widget ABC、ArkData ABC、release ABC。

验收：fixture matrix 的 `modules.abc/widgets.abc/data.abc` 字节一致，或规范化比较器无 runtime-relevant diff。

### Phase 6：打包、native、签名

任务：

- HAP/HSP/HAR 分 target 生成 pack info 和 archive entry。
- 已完成基础：HSP `bundle_type/moduleType` 修为 shared。
- ByteCode HAR/source HAR 两条路径拆开。
- integrated HSP/native 从 dependency graph 和 native task 产物进入 packager；native SO 依赖侧已完成 declared BFS 基础收集；integrated HSP 已完成本地 declared BFS 基础收集和 API > 11 hard error gate；preload SO 不应被当作 native lib 直接入包，剩余是 ArkCompile 生成 `preload/default/preload.json` 与资源 startup profile/package 的 strict oracle。
- 签名阶段增加 semantic verifier；unsigned parity 作为主门禁。

验收：未签名 archive 的 entry 顺序、方法、CRC、size、content hash 一致；签名产物语义一致。

### Phase 7：CI 宣称标准

只有当以下条件满足时，才能声明“修复后与 hvigor 完全一致”：

- fixture matrix 全部运行；
- strict parity report 无 diff；
- allowlist 为空；
- 每个非字节稳定字段都有明确规范化规则；
- release/debug、HAP/HSP/HAR、normalized OHM on/off 都覆盖。

## 5. 立即建议的修复顺序

1. 建最小 oracle CI：先让已有 minimal HAP debug/release、HSP、source HAR、share、startup、release obfuscation parity 测试在 SDK 可用环境下定期跑，并输出 strict diff。
2. 将 fixture allowlist 按 stage 拆分到 profile/resource/loader/cache TS/ABC/archive/signing，逐项清零。
3. 建 hvigor 等价 `DependencyGraphService`，统一驱动 HSP/HAR/ByteCode HAR/SO、resource dependencies、loader maps、integrated HSP。
4. 补剩余 resource-phase 后处理：DependencyManager remote/cache 严格节点类型和顺序、distro 多 target strict oracle、form module target source-set/DependencyManager strict oracle、requestPermissions/shortcut strict oracle、PAC ValidateUtil 错误文案/strict app package oracle、ArkCompile `needPreloadSoFileSet`/`preload.json` strict oracle、dep startup/share strict oracle。
5. 对齐 release obfuscation rule engine、name map、consumer config 和 cache invalidation。
6. 推进 ViewPU/cache TS/source map/CompileArkTS handoff 顺序宽场景 golden。
7. 推进 ABC 后端与 es2abc 的 byte-level parity，包括 ProgramCache/protoBin、release optimizer、widget、ArkData。

## 6. 当前不能直接采信的历史结论

仓库中存在一份 `meta/design/hvigor-full-gap-analysis.md`，末尾写着 “95/95 100% addressed”。按当前源码核验，这个结论过强：share config 覆盖、release obfuscation 输入、HSP pack.info、PAC/privacy module/project 基础链路、permission reason `resource_str` 基础链路、user-grant 权限基础校验等问题已在本轮修复或补测试，但配置输入模型、DependencyManager 等价层、resource/profile 宽场景、ABC upstream cache/protoBin、HAR/native/signing 仍未闭环。后续应以本文和 oracle report 为准更新状态。
