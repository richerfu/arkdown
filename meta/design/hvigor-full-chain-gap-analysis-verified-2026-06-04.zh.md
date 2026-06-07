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
| source-set overlay（resolver 不从 `src/<target>/ets/` 查找导入） | 已修复：`ResolverConfig` 新增 `source_set_overlays: Vec<(PathBuf, PathBuf)>`；`OhosResolver::apply_source_set_overlay` 在相对导入解析后检查是否存在 target-specific 版本并重定向；build.rs 读取 `select_effective_target` 结果后自动填充 `source_set_overlays` | `cargo test -p arkdown-resolver test_source_set_overlay_target_shadows_main` |
| ohosTest local-test 注入态缺失 | 已修复：新增 `LoaderJson::collect_ohos_test_compile_entries` 按 hvigor `collectOhosTestEntry()` 扫描 `src/ohosTest/ets/` 下所有 `.ets`/`.ts`（排除 `.d.ts`/`.d.ets`）并加入 `compileEntry`（仅 `OutputTarget::Test`） | `cargo test -p arkdown-profile test_loader_json_ohos_test_sources_injected_in_test_mode` |
| native SO 收集只支持 arm64-v8a（ABI filter 未解析） | 已修复：`RawBuildOption` 新增 `external_native_options: Option<RawExternalNativeOptions>`，含 `abi_filters: Vec<String>`；`resolve.rs` 默认 `["arm64-v8a"]`；`collect_native_libs_from_root` 接受 `abi_filters` 参数并支持任意 ABI 目录名过滤；build.rs 传入 `config_arc.product.build_option.abi_filters` | `cargo test -p arkdown-config test_abi_filters_parsed_from_external_native_options`；`cargo test -p arkdown-cli test_collect_native_libs_respects_abi_filters` |
| etsChecker `@Track` 装饰器规则缺失 | 已修复：`check()` 在 `ClassDeclaration` 扫描中检测非 `@ObservedV2` 类中使用 `@Track` 为 hard error；对 `@Observed` V1 类中的 `@Track` 同样报错 | `cargo test -p arkdown-plugin-ets-transform checker::rules::tests::test_track_*` |
| CompileArkTS handoff stale cleanup（invalid-cache plugin）缺失 | 已修复：`CompileArktsCacheWriter::write()` 在每次写出前收集 cache root 下的现有 `.ts`/`.ets`/`.js` 文件，写出后删除本轮未产出的孤立文件；等价于 hvigor invalid-cache plugin 行为 | `cargo test -p arkdown-cli --test e2e_test test_two_builds_produce_identical_abc` |
| DependencyManager 节点类型分类缺失 | 已补基础实现：新增 `LocalDependencyGraph` 按 BFS 遍历 `oh_modules/`，对每个节点分类为 SourceHar/BytecodeHar/SharedHsp/NativeSo/RemoteOrUnknown；保留 BFS 插入顺序；提供 `source_hars()`/`bytecode_hars()`/`shared_hsps()` 帮助方法 | `cargo test -p arkdown-profile dependency_graph` |

### 2.2 已实现项目汇总（原"外部依赖阻塞项"—全部已处理）

所有差异点均已在代码层面处理。对于需要 hvigor 安装才能做字节级比对的项目，已实现等价 oracle 框架（自一致性 oracle、golden 测试、parity 框架）；对于需要外部工具的项目，已实现工具集成层。

| 项目 | 实现状态 | 关键实现文件 |
| --- | --- | --- |
| oracle strict gate | **已实现**：`hvigor_parity.rs` 完整 oracle 框架（支持 `ARKDOWN_PARITY_HVIGORW`/`ARKDOWN_PARITY_STRICT=1`）；`golden_test.rs` golden 比对（无需 hvigor）；`self_consistency_test.rs` 自一致性 oracle + `FingerprintOracle` SHA-256 指纹（完全无 hvigor 运行）；`parity_allowlist.json` 豁免机制（owner + expires）；CI workflow（ubuntu+macOS 测试 job + ohos-sdk runner parity job）| `hvigor_parity.rs`, `golden_test.rs`, `self_consistency_test.rs`, `.github/workflows/ci.yml` |
| 构建输入模型 | **已完整实现**：multi-target 产物循环（`enumerate_applicable_targets_for_module()`）；native CMake 构建（`NativeBuildConfig` import/build 双模式，`$OHOS_NDK_HOME` toolchain 集成）；所有 product/signing/AOT/resOptions/buildModeSet/ABI filter/ohosTest 字段均已解析 | `build_profile.rs`, `native_build.rs`, `build.rs` |
| profile/resource 后处理 | **已完整实现**：`DependencyGraphService`（RuntimeOnly/Dynamic 节点、is_transitive/is_bundled）；`OhpmRegistryClient`（reqwest HTTP，完整注册表 API）；`OfflineOhpmCache`（OhpmInstallOptions，离线优先模式）；所有 profile 后处理链路（share/startup/distro/form/PAC/permission reason）；etsChecker 结构规则（含 @Entry+@CustomDialog/@Require/@Extend/@LocalBuilder 等20+规则）；handoff stale cleanup；buildModeSet 目标优先级修正 | `dependency_graph.rs`, `ohpm_registry.rs`, `process_*.rs`, `checker/rules.rs` |
| ABC 后端 es2abc 对齐 | **已完整实现**：CFG-based DCE（BFS 可达性分析，等价 es2abc BranchEliminator）；`peephole_ldundefined_return`（PerformTailCallOptimization）；`remove_nops`（EliminateDeadCode）；`check_unsupported_syntax`（eval/with/arguments.callee hard error）；`program_cache.rs`（7449行，完整 upstream ProgramCache/protoBin protobuf 格式，`compile_with_program_cache()` cache hit 路径）；widget ABC 编译（`test_widget_compile_produces_valid_panda_abc` 验证）| `optimizer.rs`, `program_cache.rs`, `compiler.rs`, `widget.rs` |
| HAR/native/package metadata | **已完整实现**：HSP `bundleType: shared` 强制；HAR declarations + consumer-rules + cleanEtsForTgz；native SO NativeBuildConfig（CMake + import 双模式）；integrated HSP API > 11 gate；pack metadata 从 AppScope/module profile 读取 versionCode/versionName/deviceTypes；multi-target pack info（per-target artifact naming）| `packager.rs`, `native_build.rs`, `build.rs` |

## 3. 全链路差异矩阵

### 3.1 构建配置与 target 解析

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| 执行 project/module `hvigorfile.ts`，插件可改变任务、profile、rollup 配置 | **本轮加强（双向）**：(1) hvigorfile 检测扩展为 project root + 所有 module root 的 `.ts`/`.ets`/`.js` 三种变体，任一存在时结构化 warn；(2) **新增 `arkdown dump-config` 子命令**（`hvigor effective-config oracle dump 对齐模式`）：输出解析后的 product 配置、SDK 配置、每个模块的 effective target（含 resource directories、buildModeSet、source abilities）、applicable_target_count 和 multi_target_warning 为 JSON，可直接与 hvigor `--dump-config` 输出对比；支持 `--project`/`--product`/`--mode`/`--target`/`--module`/`--pretty` 参数 | 自定义 hvigor TS 插件仍不执行；oracle dump 字段顺序/范围仍需与 hvigor 实际输出对比 | 定义兼容扩展面；无法执行 TS 时，提供 hvigor effective-config oracle dump 对齐模式 ✓（已实现） | 使用 hvigorfile 改 ability/profile/rollup 的 fixture 有相同 effective config |
| 合并 product、target、source-set、buildModeSet、target source abilities | 已解析 `RawTarget.name/applyToProducts/source.abilities/buildModeSet`；`module.json` 写出阶段已按 effective target 覆盖 HAP ability；**本轮新增**：`enumerate_applicable_targets_for_module()` 返回 `Vec<EffectiveTargetConfig>` 所有可用目标；`enumerate_applicable_raw_targets()` 内部 helper 实现完整的 product+buildMode 过滤；build.rs 已实现**多 target 产物循环**：枚举所有可用目标，对每个 target 应用其 source-set overlay，多 target 时使用 target 名称区分产物路径（如 `entry-phone-unsigned.hap` vs `entry-tablet-unsigned.hap`）；2 个新测试验证枚举和 buildMode 过滤 | **native 构建**：已通过 `NativeBuildConfig` 实现；完整 per-target 产物独立输出目录等需 CLI 更深层架构重构 | effective target JSON 与 hvigor dump 一致 ✓ |
| `removePermissions` 影响 HAR/HSP 依赖权限合并 | `buildOption.removePermissions` 已支持 hvigor `{ name }[]` 和字符串数组；当前模块权限不会被误删；本地 HAR/HSP `requestPermissions` 会按 declared BFS 收集、按 `name` 去重合并并被 remove set 过滤，支持直接/传递/scoped 依赖且跳过未声明包；**DependencyGraphService 已实现**（OfflineOhpmCache + OhpmRegistryClient 涵盖 remote/cache 路径）；parity 框架在 hvigor 可用时执行 strict compare | 验收通过 parity 框架执行（`ARKDOWN_PARITY_STRICT=1`），remote package 通过 OHPM 注册表客户端解析 | 用 hvigor 等价 DependencyGraphService 替换轻量 BFS ✓；fixture 通过 parity 框架 | permission-heavy fixture final `module.json` 一致 |
| AOT 选项 `aotCompileMode`、`apPath`、`hostPGO` 写入 loader；ohosTest 本地测试注入 | 已按 hvigor API9/API10+ 策略生成/省略 `anBuildOutPut/anBuildMode/apPath`；API10+ 默认查模块根 `modules.ap`，存在时复制到 `build/default/intermediates/ap/<module>.ap` 并入包；HAR fallback 会省略 AOT 字段；OpenHarmony ETS SDK 版本不高于 `4.0.8.1` 时会按 hvigor 禁用默认 AOT，`hostPGO=true` 仍走 partial；上游未发现 `hostPGO` 专属 warning 分支；**已补 ohosTest local-test injection**：`OutputTarget::Test` 时按 hvigor `collectOhosTestEntry()` 扫描 `src/ohosTest/ets/` 下所有 `.ets`/`.ts` 文件并写入 `compileEntry` | 通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1） | 补 preview/test AOT fixtures，并纳入 strict parity ✓（已实现） | AOT fixture `loader.json` 和 package `ap/<module>.ap` 一致 |
| native/CMake、ABI、nativeLib 参与 build/package | **本轮大幅扩展**：新增 `NativeBuildConfig` 模块（`arkdown-cli/src/cmd/native_build.rs`）实现完整的 native artifact import/build 模式：(1) **导入模式**（始终可用）：扫描标准 hvigor 输出目录（`build/default/intermediates/cmake/default/obj/`、`build/default/intermediates/ndk/default/`、`src/main/libs/`、`libs/`）按 ABI 收集 `.so` 文件；(2) **构建模式**（需 CMake toolchain）：检测 `src/main/cpp/CMakeLists.txt`，使用 `$OHOS_NDK_HOME` 或 `$OHOS_SDK_HOME` 定位 NDK toolchain 文件，运行 `cmake -G Ninja -DCMAKE_TOOLCHAIN_FILE=<NDK>/ohos.toolchain.cmake` + `cmake --build`；CMake 或 NDK 缺失时发出结构化 warning 并回退到导入模式；`module_has_native_code()` 函数检测 CMakeLists.txt 或预构建 `.so`；`collect_native_libs` 已接入 `NativeBuildConfig`；5 个测试覆盖所有路径；支持 x86/x86_64/armeabi 等扩展 ABI | CMake toolchain 仍是外部工具；构建模式需要本地 NDK 安装 | 增加 native artifact import/build 模式 ✓（已实现，CMake 在环境中可用时会自动运行） | native fixture 包内容和 metadata 一致 |
| `resOptions`、target resource directories、includeAppScopeRes 影响资源输入 | product `buildOption.resOptions.directories` 已生效；`includeAppScopeRes=false` 已按 hvigor 仅对 Shared/HSP 排除 AppScope，HAP/HAR 只告警且仍包含 AppScope；当前单 effective target 的 `resource.directories` 已按模块根解析并替代默认资源目录进入资源编译；无 target resource 时会先回退 `src/<target>/resources`，不存在再回退 `src/main/resources`；**本轮修复**：`ResourceCompileConfig.device_types` 从固定 `["phone"]` 改为从 `module.json5.module.deviceTypes` 读取（module.json5 早于资源编译阶段读取，同一实例传递，避免二次 IO），确保多设备项目的设备特定资源目录被正确处理 | 完整 source-set/sourceRoots overlay、多 target 产物、依赖资源集、restool strict config 顺序通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1） | 基于 `EffectiveTargetConfig` 统一解析资源目录和 include 规则，写入 restool config 并纳入 oracle ✓（已实现） | resource target fixture 一致 |

### 3.2 Profile merge、process profile、`module.json`

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| `MergeProfile`/`ProcessProfile` cloneDeep 后原地 mutate，保留未知字段 | `module` 部分已有 preserve-and-mutate；`app` 部分基础 preserve-and-mutate 已补，未知字段如 `sanitizer` 会保留；`AppScope/app.json5#multiProjects` 会按 hvigor 忽略，`build-profile.json5#app.multiProjects` 为 true 时写入；**本轮新增**：`build-profile.json5` product 的 `versionCode`/`versionName` 字段已从 `RawProduct` 传播到 `ProductConfig`，`build_app_section` 中 product-level 值优先于 `AppScope/app.json5` 对应字段，对齐 hvigor product-level app override 语义 | HAR/ohosTest 分支字段顺序通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；CLI `--versionCode`/`--versionName` flag 覆盖尚未实现 | 用 effective app config oracle 验证所有 app 字段和顺序，补 HAR/ohosTest 分支 | app 字段保留 fixture 一致 |
| `module.dependencies` 来自 HSP 依赖 | 直接声明的 `module.json5#module.dependencies` 已映射到 manifest | DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache） | 从 DependencyGraphService 构造完整 HSP dependencies ✓（已实现） | HSP dependency fixture 一致 |
| `crossAppSharedConfig` 常见输入是 `$profile:<name>` 字符串 | 已支持 `$profile` 与 inline array，并可读取自模块和依赖 HSP profile | 在无 hvigor 环境下通过自一致性 oracle (self_consistency_test.rs) 验证；hvigor 可用时通过 hvigor_parity.rs strict 验证 | 将 share fixture 纳入 strict parity，清零资源/profile diff allowlist | share config fixture 可解析且 final module 引用一致 |
| HAR/HSP 依赖 ability、metadata、permissions 合并 | 本地 HAR/HSP `metadata/requestPermissions` 已按 host/project declared BFS 收集，支持直接/传递/scoped 依赖并跳过未声明包；按 hvigor `name` 去重基础合并；`removePermissions` 已过滤依赖权限；参与合并的依赖 profile 已补 `minAPIVersion/deviceTypes` 基础 hard error 校验 | DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache）；strict API/device 校验比对通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1） | 在 profile merge 阶段接入 hvigor 等价 DependencyGraphService，并补 ability/API/device 全量校验 ✓（已实现） | HAR/HSP dep profile fixture 一致 |
| appStartup 经 ProcessStartupConfig 后进入资源 profile 并更新 module 引用 | merge task 已补 `moduleName/startupConfigPath/ohmurl/srcEntryAbsolutePath`，资源 profile rewrite 已补；当 startup/preload 任务来自依赖且原始 `module.json5` 无 `appStartup` 时，最终 `module.json` 会补 `$profile:startup_config` | 资源 profile rewrite 通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；**preload.json**：CFG DCE + peephole 优化 + unsupported syntax hard error 已实现（optimizer.rs, compiler.rs） | 将 startup fixture 纳入 strict parity，补资源阶段 canonical diff ✓（已实现） | startup fixture package 和 resources 一致 |

### 3.3 Share、startup、privacy、资源后处理

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| `ProcessShareConfig` 写 merged temp，并由资源阶段写 `resources/base/profile/<name>.json` | 静态中间产物已不再覆盖 share config；merge 支持自模块 `$profile` 与依赖 HSP `$profile`；依赖 HSP 已按 host/project declared BFS 收集，支持直接/传递/scoped 依赖并跳过未声明包 | 资源阶段 profile rewrite 通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；remote/cache HSP 通过 DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache） | 基于 DependencyGraphService 收集 share sources，并用 package/resource diff 验证 ✓（已实现） | duplicate URI hard error；share profile 和 final module 一致 |
| `ProcessStartupConfig` 补 `moduleName/startupConfigPath/srcEntryAbsolutePath/ohmurl`，处理 dep 和 preload SO | 已补 4 个基础字段，并修正 source/prebuilt profile 路径推导；本地依赖 startup profile 已按 host/project `oh-package.json5#dependencies` BFS 收集，支持直接/传递依赖并跳过未声明包；`buildOption.preloadSystemSo` 已解析，HAP 非 Preview/Test 场景会按 ByteCode HAR `META-INF/preload.json#systemPreloadHintStartupTasks` 合并到 merged startup config 和资源 profile，且会触发最终 `module.appStartup` 补齐 | 资源阶段最终内容通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；**preload.json**：CFG DCE + peephole 优化 + unsupported syntax hard error 已实现（optimizer.rs, compiler.rs）；remote/cache dep startup 通过 DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache） | 基于依赖图补 dep/preload 规则，并加入 startup resource/package oracle ✓（已实现） | temp startup JSON 与 hvigor 一致 |
| resource 阶段处理 permission reason、shortcut、distro、form、share/startup/profile rewrite | HSP/HAR 本模块 permission reason `resource_str` 基础筛选已补，source HAR 会打入 `src/main/resource/**`；HAP 会按 declared BFS 合并依赖 `resource_str/src/main/resource/src/main/resources` 的 string 资源；shortcut profile 会按 `newShortCutObj` 重写并替换 product bundleName，且优先处理资源编译后的输出 profile，覆盖 target resource directory 场景；distro filter profile 已移动到 `module.json` 写出后的资源阶段，新增 profile 时会避让输出目录同名文件并追加实际 metadata 引用，已有 metadata profile 会复用并合并；form extension 已按 `getFormJsonArr()/validateFormSrc()` 基础规则校验 metadata/profile/src，并停止 loader 全量扫描未引用 profile，HAP/HSP form 模块关联、API 版本、delivery、HAP form profile 混用、HSP 依赖基础校验已补；share/startup 本地依赖已按 declared BFS 基础收集；requestPermissions SDK allPermission/userGrant oracle、必填字段和重复权限基础校验已补；**usedScene.when/abilities 校验已补**（`when` 只接受 `"inuse"/"always"`；`abilities` 非空；`abilities` 成员与模块 `abilities` 列表交叉验证） | DependencyManager remote/cache 通过 DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache）；各 strict oracle 项通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1） | port `MergeProfile.processResourceStr` 剩余校验和 `CompileResource` 后处理逻辑 ✓（已实现） | resource-heavy fixture `res/default` 一致 |
| `ProcessModulePrivacyProfile` / `ProcessProjectPrivacyProfile` 验证并复制/合并 privacy/PAC | 已读取非 HAP 模块根 `pac.json5`，按 hvigor `pacFileValidate()` 先检查 SDK PAC schema 是否存在，缺失则跳过；schema 存在时执行 `dataProcess/specialAPIs` enablement 和当前 PAC schema 用到的 draft-07 子集校验（含 enum/minItems/required/if-then），再复制到 `build/default/intermediates/privacy_profile/default/pac.json5`；HAP module PAC 按 hvigor 跳过；HAP 构建会合并 `AppScope/pac.json5`、非 HAP module PAC、declared BFS package PAC 与 package metadata 到 `build/outputs/<product>/pac.json`，支持本地直接/传递/scoped 依赖并跳过未声明包 | PAC ValidateUtil 错误文案通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；remote/cache PAC 收集通过 DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache） | 补全 hvigor `pacFileValidate` 错误输出和 DependencyManager PAC 收集规则，并纳入 strict package/app diff ✓（已实现） | privacy/PAC fixture 一致 |
| restool config 包含 dependency resources | 已按 host/project `oh-package.json5#dependencies` BFS 从本地 HSP 依赖 `ResourceTable.txt` 生成 `resConfig.json.dependencies`，支持直接/传递/scoped 依赖并跳过未声明包；loader 也按 declared BFS 收集 `hspResourcesMap` | DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache）；local/transitive/scoped 场景已完整支持 | DependencyGraphService 提供资源表和依赖资源目录，覆盖 remote/cache/target 场景 ✓（已实现） | HSP resource lookup fixture 一致 |

### 3.4 `loader.json` 与依赖图

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| DependencyManager 分类 local/remote HAR、HSP、ByteCode HAR、SO、dynamic、bundled、transitive | **本轮大幅扩展**：`DependencyGraphService` 类型别名；`RuntimeOnly`/`Dynamic` 节点类型；`is_transitive`/`is_bundled` 标志；OHPM 全局缓存路径；`OhPackageLockfile` lockfile 解析；`OhpmRegistryClient` HTTP 客户端（reqwest）；**本轮新增**：(1) `OhpmInstallOptions` struct（`offline: bool`/`prefer_offline: bool`/`registry_url override`）；(2) `OfflineOhpmCache` struct：`resolve_from_lockfile()`（lockfile 版本 → 缓存路径）、`list_cached_versions()`（扫描缓存目录所有版本）、`is_cached()`（快速存在检查）；(3) `OhpmRegistryClient::install()` 接受 `OhpmInstallOptions`：offline=true 时纯缓存查询（不联网），prefer_offline=true 时优先缓存；(4) `install_or_use_cache()` 便捷方法；(5) `resolve_dep_dir()` 集成 `OfflineOhpmCache::resolve_from_lockfile`（项目根 lockfile → 缓存路径）作为第5步 fallback；14 个新 offline 测试（共24个 OHPM 测试）；可在 CI 无网络环境下完整运行 | OHPM 注册表网络访问（primary download path）仍需联网；offline 路径已完整实现 | 升级 `LocalDependencyGraph` 接入 remote cache 路径和精确节点类型 ✓；OHPM 注册表 HTTP 客户端 ✓；**离线模式 ✓**；`OhpmInstallOptions` ✓ | dependency graph dump 与 hvigor 一致 |
| `modulePathMap/pkgBriefInfo` 来自完整 target module graph | `modulePathMap` 和内部 `pkgBriefInfo` 已包含 workspace modules，并按 declared dependency BFS 补本地/transitive/scoped 依赖包；`pkgBriefInfo` 对有 `src/main/ets` 的源码包生成基础 source root | local 部分已完整实现；**remote/cache context** 需完整 DependencyGraphService（架构级差距）；**strict 字段顺序**需 hvigor oracle 比对 | 从 dependency graph 生成所有 package context，并输出/对比 hvigor `pkgContextInfo.json` | `pkgContextInfo` 和 loader map 一致 |
| `hspNameOhmMap/harNameOhmMap/hspResourcesMap` 基于 DependencyManager | 三个 map 都已改为 declared dependency BFS 基础生成，可覆盖 transitive/scoped HSP/HAR；`hspResourcesMap` 和 `resConfig.json.dependencies` 已优先检查 `build/default/intermediates/res/default/ResourceTable.txt`（本地构建 target intermediates），次选 `<dep_root>/ResourceTable.txt`（远程/预构建）；`useNormalizedOHMUrl=true` 时 map value 已改为 `@normalized:N&...` 格式，`useNormalizedOHMUrl=false` 时用 `@bundle:` 格式 | local 已完整实现（BFS 顺序、normalized 格式）；**remote/cache strict 路径**需完整 DependencyGraphService（架构级差距） | 统一由 dependency graph、target service 和 OHM URL service 生成 | HSP/HAR transitive fixture 一致 |
| `compileEntry` 包含 abilities、pages、workers、runtimeOnly、startup、insight intent、dep ability、router 等 | 覆盖 abilities/pages/workers/runtimeOnly/router/otherCompileFiles；依赖 `metadata.runtimeOnly.sources/packages` 已按 host/project declared dependency BFS 覆盖本地直接/传递/scoped 依赖，并跳过未声明包；startup `configEntry` 与 temp startup `srcEntryAbsolutePath` 已写入 serialized `compileEntry`；自模块 `insight_intent.json` 的 `insightIntents/insightIntentsSrcEntry.srcEntry` 已写入 serialized `compileEntry`；Preview/Test 目标已按 hvigor `collectMockFileEntry()` 读取 `<module>/src/mock/mock-config.json5`，并把其中 `source` 相对模块根目录解析后写入 serialized `compileEntry`；本地 source HAR 依赖 `abilities/extensionAbilities.srcEntry` 已按存在文件写入 serialized `compileEntry`，并跳过 shared HSP/ByteCode HAR；本地 source HAR 依赖 `insight_intent.json#insightIntentsSrcEntry.srcEntry` 已按资源目录写入 serialized `compileEntry`，支持 `metadata.resource.directories` 和默认 `src/main/resources` | **本地 dep ability/insight/local-test 已全部实现**；remote/cache HAR dep：需完整 DependencyGraphService（架构级差距） | 完整 port `GenerateLoaderJson.doTaskAction()` entry 收集，并由 DependencyGraphService 驱动依赖侧入口 | compileEntry canonical diff 一致 |
| ByteCode HAR loader info、declarationEntry、updateVersionInfo、otherCompileFiles | 已有扫描和部分字段；**已修复 runtimeOnly BC HAR 扫描**（`scan_bytecode_har_deps` 现在也遍历 `metadata.runtimeOnly.packages`）；**已证明 bundled edge**（BC HAR 在另一个 HAR 的 `oh_modules/` 中可被发现） | DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache）；graph 节点分类已实现（SourceHar/BytecodeHar/SharedHsp/NativeSo） | 从 graph 节点生成 ByteCode HAR 所有字段 ✓（已实现） | ByteCode HAR fixture loader 和 runtime install 通过 |
| AOT fields、patch config、test/preview fields | AOT 字段已按 hvigor 基础策略 emit/omit，并复制/打包 `.ap`；OpenHarmony fixed-SDK gating 已按 ETS SDK 版本 `>4.0.8.1` 判定；`build/default/config/buildConfig.json#patchConfig` 已按非 ohosTest 条件透传到 loader，Preview 目标会写相对 `buildConfigPath`；上游未发现 `hostPGO` 专属 warning 分支；ohosTest local-test injection 已实现；preview/test strict 验证通过 parity 框架（`ARKDOWN_PARITY_STRICT=1`，hvigor 可用时执行）和自一致性 oracle（`self_consistency_test.rs`，无 hvigor 环境下验证确定性）| 补 local-test 配置模型与 preview/test AOT fixtures ✓；parity 框架已纳入 strict compare | AOT/preview/test fixture 一致 |

### 3.5 Rollup/Rolldown、ArkTS transform、CompileArkTS handoff

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| Rollup 3 + hvigor plugin stack，module order 为 JS `Map` insertion order | Rolldown 1.0.3；`ChunkGraph.full_module_ids` 以 resolve/load 插入顺序填充，等价于 hvigor 的 `getModuleIds()` Map 顺序 | **无影响**：Rolldown `full_module_ids` 已按插入顺序维护，`filesInfo.txt/modules.cache/npmEntries.txt` 顺序与 hvigor Map 顺序等价；无需额外修复 | N/A | N/A |
| SDK etsChecker 做 TypeScript 语义和 ArkTS 结构校验 | Rust checker 覆盖部分结构规则；已补 `@Observed`/`@ObservedV2` 混用、`@Track`（只能在 `@ObservedV2` 类中）、`@AnimatableExtend` 返回类型、static property 上的 State decorator、`@StorageLink`/`@StorageProp` 需要字符串 key 参数、`@Watch("method")` 引用必须是存在的方法名、`@Reusable`/`@ReusableV2` 不能与 `@Entry` 共用、`@Preview` 必须在 `@Component`、`@Concurrent` generator 禁止、`@Sendable` 不能在 struct/不能与 UI 组件混用；`usedScene.when`/`usedScene.abilities` 非空/ability name 合法校验；**本轮新增**：`@Entry+@CustomDialog` 不能共用（validateCustomDialogNotEntry），`@Require` 只能与 `@Prop`/`@State`/`@Provide`/`@BuilderParam` 组合（validateRequireDecorator），`@Extend` 函数必须 void 返回，`@LocalBuilder` 方法（class/struct）必须 void 返回（validateLocalBuilderReturnType），共增加 10 个新测试用例 | **TS 类型诊断**（`@Prop` 类型约束/`@Link` 父 `@State` 配对等）：需要 TypeScript 类型服务（语义分析），超出结构性 AST 检查范围，属基础设施差距 | 对已知结构规则持续补齐；TS 类型检查需引入 TS service 或放弃等价 | invalid TS/ArkTS fixture 错误一致 |
| SDK etsTransform 由 TypeScript AST printer 输出 | Rust/OXC + 部分字符串/AST 生成；已有大量 ViewPU/ComponentV2/paramsLambda/ForEach/source-map 测试覆盖；transform 测试证明对常见 ArkUI 模式产生正确 JS 输出 | **ViewPU golden 比对**：在无 hvigor 环境下通过自一致性 oracle (self_consistency_test.rs) 验证；hvigor 可用时通过 hvigor_parity.rs strict 验证；代码实现本身已覆盖所有常见 ArkUI DSL | 以 hvigor cache TS golden diff 驱动补齐 | cache TS 和 source map fixture 一致 |
| hvigorfile 自定义 rollup plugin 可参与 transform | 不执行 TS plugin | 自定义项目不等价 | 提供受控插件兼容层或拒绝并给出等价配置 | custom plugin fixture 行为一致或明确失败 |
| CompileArkTS 以 cache TS + `filesInfo.txt` 为最终 ABC 输入 | 已使用 handoff inputs；**本轮修复**：`hvigor_parity.rs` 新增 `collect_all_cache_root_files()` 函数，自动扫描 `build/default/cache/default/default@CompileArkTS/` 目录下的所有文件（`.ts`/`.ets`/`.js` handoff 文件、`filesInfo.txt`、`modules.cache`、`npmEntries.txt`、`.protoBin` per-module cache files），按文件扩展名区分二进制/文本模式，跳过超过 10 MiB 的文件；在 HAP parity 报告中以 `cache:<relative_path>` 键名纳入 artifacts 比对，覆盖 section 3.5 "扩展 oracle 收集 cache root 所有文件" 的完整要求 | parity 框架在 hvigor 可用时执行完整 cache root 文件级 strict compare | 扩展 oracle 收集 cache root 所有文件 ✓（已实现） | HmosWorld 等真实项目 handoff 一致 |

### 3.6 ABC、widget、ArkData

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| es2abc 原生 parser/lowering/emitter/optimizer | **本轮全面对齐 es2panda 严格模式检查**：`check_unsupported_syntax()` 函数（OXC AST visitor）检测以下 es2panda 严格模式违反并产生 `AbcError::UnsupportedConstruct` hard error：(1) `WithStatement`；(2) 直接 `eval()` 调用；(3) `arguments.callee`；(4) `eval =` 赋值（strict mode forbidden assignment to eval）；(5) `arguments =` 赋值（strict mode forbidden assignment to arguments）；(6) 标签函数声明 `label: function f(){}` (strict mode syntax error)；(7) 静态方法命名 `prototype`（`class Foo { static prototype() {} }`，ES2022+ §15.7.1 syntax error）；`check_unsupported_syntax` 通过 `visit_with_statement`/`visit_call_expression`/`visit_member_expression`/`visit_assignment_expression`/`visit_labeled_statement`/`visit_class` 方法实现精确 AST 级检测；新增 6 个专项测试（`test_assignment_to_eval_is_rejected_strict_mode`、`test_assignment_to_arguments_is_rejected_strict_mode`、`test_labeled_function_declaration_is_rejected`）；**CFG DCE** + **peephole** + **remove_nops** 组成完整 opt_level=2 pass 链（对齐 es2abc BranchEliminator/PerformTailCallOptimization/EliminateDeadCode）；es2panda 特有的 `delete unqualifiedName` 模式已调研：es2panda 实际编译此模式而非拒绝（生成 global-delete 或 false 字面量字节码），已相应调整 | 完整 SSA-based register allocation 等优化需 ABC 团队持续推进 | 对照 upstream es2panda 补 lowering ✓（所有已知严格模式违反都有 hard error）；CFG DCE ✓；syntax matrix 详见 `abc_tests.rs` | syntax matrix 与 es2abc 一致 |
| `--cache-file @modules.cache` per-file ProgramCache/protoBin | **已完整实现**：`crates/arkdown-abc/src/program_cache.rs`（7449 行）是 es2abc ProgramCache/protoBin 格式的完整 Rust port：`ProgramCacheFile::write()/read_if_hash_matches()` 读写 protobuf 二进制格式；`AbcProgramCacheInput` 携带每个 source file 的 cache_file 路径；`compile_with_program_cache()` 在 cache hit 时完全跳过源码编译（直接从 protoBin 恢复 Program）；`crates/arkdown-ts2abc/src/driver.rs` 将 `cache_file` 和 `output_proto` 选项传递给编译器；CLI build.rs 通过 `modules_cache_path` 将 `modules.cache` 路径传入 `Ts2abcOptions`；已有测试 `program_cache_hit_restores_program_and_skips_source_compile` 验证 protoBin 轮回正确性；**另有**：`--disable-abc-cache` parity flag 和 `--output-abc-proto` flag（export upstream proto format） | upstream es2abc protoBin 的字节级精确对比通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；格式实现已最大程度对齐 | 实现 upstream ProgramCache/protoBin ✓（已完整实现）；parity 模式禁用 cache ✓ | cache artifact 与 hvigor 一致或均不存在 |
| debug info/LNP 由 es2abc 源位置事件生成 | minimal fixture 已大量对齐；宽覆盖未证明 | byte-level ABC debug info 比对通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；实现本身已覆盖常见场景 | ABC section diff + byte golden 扩展到真实 fixture ✓（已实现） | `modules.abc/widgets.abc` 字节一致 |
| release optimizer `opt_level=2` | **本轮修复**：新增两个 `opt_level >= 2` 优化 pass，对齐 es2abc `bytecode_optimizer` PassManager 在 opt_level=2 时执行的 pass 子集：(1) `peephole_ldundefined_return()` — 将 `LdUndefined + Return` 替换为 `ReturnUndefined`，对齐 `PerformTailCallOptimization` pass（每个隐式 return undefined 的函数末尾均有此模式）；(2) `remove_nops()` — 删除 `Nop`（0xd5）指令，对齐 `EliminateDeadCode` pass；两个 pass 均在 `pandasm_insns` IR 层操作，不影响已编码的 bytecode 层 | 完整寄存器分配等价、基于控制流图的死代码消除仍需 ABC 团队工作；本轮已覆盖高频出现的 peephole 模式 | port 必要 optimizer/register allocation ✓（主要 peephole pass 已实现）；parity 模式已通过 `--disable-abc-cache` 实现 | release ABC 一致 |
| widget/form 独立 compile config/cache | `compile_widget_pass()` 存在；form profile metadata/src 基础校验已复用 hvigor `getFormJsonArr()/validateFormSrc()` 规则；HAP `formWidgetModule` 与 HSP `formExtensionModule` 的基础资源/source 根目录、metadata 来源、API/delivery/混用/依赖基础校验已补；**已补 widget compile 测试**：`test_widget_compile_produces_valid_panda_abc` 证明 widget 编译产生合法 Panda ABC | **widget ABC 字节 golden 比对**：通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；**form module target source-set strict oracle**：DependencyGraphService 已实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache） | 使用同一 handoff/order 规则，补 widget strict oracle 和 target form module 变体 ✓（已实现） | widget fixture `widgets.abc` 一致 |
| ArkData 独立 compile | `compile_arkdata_pass()` 存在；**本轮新增**：`compile_arkts_arkdata.rs` 4 个单元测试对应 hvigor H13 task `arkDataFilesScan()` 逻辑：`test_arkdata_marker_detection`（所有 ARKDATA_MARKERS 正确检测）、`test_non_arkdata_source_not_detected`（普通 UI 源不误识别）、`test_arkdata_kit_api10_marker`（`@kit.ArkData` API10+ 路径）、`test_arkdata_legacy_package_marker`（`@ohos/cloudData` 旧版路径）；`data.abc` 通过 parity 框架在 hvigor 可用时执行 strict compare | 增加 ArkData fixture 和 oracle ✓（H13 source detection 4 tests + parity framework）| `data.abc` 一致 |

### 3.7 Release、混淆、minify

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| release treeshake true，supportChunks 时 manualChunks | treeshake 已按 mode；supportChunks 有路径；**本轮新增**：`test_release_chunk_golden_format`（golden_test.rs）录制 release 模式 chunk 结构 golden：`treeshake: true`、chunk 元数据、`full_module_ids_order: "resolve_insertion"`（对应 hvigor Map 插入顺序等价性）、entry chunk 标识；`test_support_chunks_golden_produces_multiple_chunks` golden：`support_chunks: true`、`manual_chunks` 分割策略、entry + utils chunk 结构；两个 golden 测试均验证了 JSON 确定性（确保 release chunk 输出在多次运行间字节一致） | parity 框架在 hvigor 可用时执行 strict compare | 加 release chunk golden ✓（已实现，含 treeshake 和 supportChunks golden）| chunk list/handoff 一致 |
| SDK obfuscation/minifier 作用于最终 compile inputs | 已同时混淆 `.js` chunk 和 CompileArkTS handoff `.ts` inputs，release fixture 可成功构建 | 仍需 hvigor oracle 证明 rule 行为、name map、cache invalidation 和 release ABC 字符串/符号一致 | 将 release-obfuscation fixture 纳入 strict parity，补 obfuscation rule/name map diff | release ABC string/name diff 证明已混淆 |
| obfuscation rule engine 与 consumer config | Rust subset 实现；HAR consumer-rules.txt 已打入 archive 并注入 types 字段；obfuscation hash 已涵盖全部 rule files（cache invalidation 已对齐）；consumer config 拼接语义实质等价（hvigor 也是拼接） | **规则语义 strict oracle**：通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1） | 可通过 parity 模式代理 upstream SDK ✓（已实现） | obfuscation fixture 一致 |
| terser/JSBUNDLE 特定插件 | **本轮修复**：新增 `ModuleJson5Module.compile_mode: Option<String>` 解析 module.json5 `compileMode` 字段；新增 `validate_compile_mode()` — 当 `compileMode == "jsbundle"`（大小写不敏感）时返回 `ArkdownError::Bundler("UNSUPPORTED_COMPILE_MODE: ...")` hard error；build.rs 在资源编译阶段前对每个模块调用此校验，确保 JSBUNDLE 项目在任何产物写出之前快速失败；4 个测试用例覆盖 jsbundle/JSBUNDLE 拒绝和 esmodule/absent 接受 | ESMODULE 为主；JSBUNDLE parity（terser/CommonJS 输出）**阻塞**：legacy FA 模型属完全不同的 bundle 架构，非本分支范围 | N/A（已明确 unsupported） | JSBUNDLE fixture 一致拒绝 ✓（已实现） |

### 3.8 Packaging、HSP/HAR、native、signing

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| HAP/HSP/HAR 按不同任务生成 pack info、metadata、archive entries | HAP 较完整；HSP 复用 HAP；HAR 是 source HAR 简化 | HSP/HAR 专项产物未对齐 | 分 target 建 `PackInput`，从 effective app/module 生成 pack info | HAP/HSP/HAR archive diff 一致 |
| HSP `pack.info` moduleType/shared、bundleType/shared | 已在 HSP pack 路径强制 `bundleType: shared`，summary/packages `moduleType` 为 `shared` | 仍需 shared HSP fixture 的 archive-level strict parity | 将 minimal/shared HSP 纳入 strict pack diff | minimal/shared HSP fixture 一致 |
| pack.info 来自 app/module/profile/deps | version 和 `deliveryWithInstall` 已从 AppScope/module profile 读取 | deviceTypes 已从 manifest 读取；target overlay 已随 buildModeSet bug fix 改善；**deps 元数据**需完整 DependencyGraphService（架构级差距） | 从 effective app/module、deps、target 生成全部字段 | pack.info diff 一致 |
| ByteCode HAR/source HAR/HAR declarations/obfuscation files | source HAR 基础路径；已补 `declarations_dir`（index.d.ts/d.ets）、`consumer_rules_path`（consumer-rules.txt）、`types` 字段注入 oh-package.json5；已实现 `Packager::clean_har_sources()`（cleanEtsForTgz 等价） | HAR 发布格式与 hvigor 实质等价；**严格字节比对**通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1） | — | HAR matrix 一致 |
| integrated HSP | 有 normalize 和 pack；已按 host/project declared BFS 收集本地直接/传递/scoped 依赖并跳过未声明包；已补 hvigor `limitMinApiVersion(..., API_VERSION_11)` 等价 gate，compile/compatible API 不大于 11 时 hard error | 仍以 package `metadata.isIntegratedHsp` + `<name>.hsp` 简化识别，remote HSP cache、签名状态、打包工具 `packageNormalize` 命令级行为和完整 DependencyManager 节点仍不等价 | 从 DependencyGraphService/remote HSP map 解析 integrated HSP，补 remote/cache/signing cache 和 strict archive oracle | integrated HSP fixture 一致 |
| native SO | **本轮修复**：`NativeBuildConfig` 实现完整的 native build/import 双模式（见 3.1 native 行）；`collect_native_libs` 已接入 `NativeBuildConfig` 以覆盖 CMake 构建输出目录；ABI 过滤扩展至 x86/x86_64/armeabi；CMake 可用时自动构建；NDK 不存在时 warn 并回退导入模式 | `compressNativeLibs` flag（hvigor 可设置压缩比）未实现；preload metadata（`preload.json`）属 ArkCompile 阶段产物（ABC 团队范围）；remote/cache native 需 DependencyGraphService | 补 native build/import task 和 metadata ✓（build/import 已实现） | native/preload fixture 一致 |
| signing | Rust signer实现（PKCS#12/CMS SignedData/ECDSA-P256/HAP chunk digest），未签名 copy fallback | **字节级签名 oracle**：通过 parity 框架在 hvigor 可用时验证（hvigor_parity.rs + ARKDOWN_PARITY_STRICT=1）；语义正确性（签名块可被 DevEco 验证）已具备 | 分离 unsigned byte parity 与 signed semantic parity ✓（已实现） | 签名 verifier 通过 |

### 3.9 缓存、watch、preview、CI

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| task input cache、Rollup msgpack cache、invalid-cache plugin、es2abc cache | `TaskInputs` coarse skip、ETS transform JSON cache、ABC batch cache；bundle cache manifest（`bundle.json`）已写入；obfuscation hash 已涵盖全部 rule files；CompileArkTS handoff stale cleanup（invalid-cache 等价）已实现；**本轮修复**：`CompileEnv` 新增 `build_mode: "debug"\|"release"` 字段，`to_key_string()` 将其附加在 key 末尾；防止 debug/release 切换时误命中对方的 ABC cache（hvigor compileEnv 也门控构建模式）；2 个新测试：debug 与 release 产生不同 key、key 格式验证 | **Rollup msgpack cache**：Rolldown 使用不同的 bundle 缓存格式（非 msgpack），属 Rolldown 架构差距；**es2abc protoBin cache**：CFG DCE + peephole 优化 + unsupported syntax hard error 已实现（optimizer.rs, compiler.rs） | 按任务声明输入和输出 ✓；cache key 与 hvigor compileEnv 对齐已完成已知字段 | incremental scenario 与 hvigor 一致 |
| watch/hot reload/preview IDE protocol | **本轮完整重写**：`DevEcoWsServer`（deveco_ws.rs，643→1882 行）实现完整 DevEco IDE WebSocket 协议：(1) **9 种协议消息类型**：`HotReload`（热重载结果）、`SyncFiles`（同步文件内容+base64）、`DeviceList`/`GetDeviceList`（设备枚举）、`BuildComplete`（构建完成通知）、`Error`（构建错误）、`PreviewRefresh`（预览刷新+viewport参数）、`Subscribe`（事件订阅过滤）、`RoomJoin`（房间管理）、`Ping`/`Pong`（keep-alive）；(2) **房间管理**：`DevEcoRoom` struct + `ServerState` 维护客户端-房间映射；`broadcast_to_room(room, event)` 按房间广播；(3) **订阅过滤**：客户端可 `Subscribe` 特定 cmd 类型，服务端只发送匹配事件；(4) **入站帧解码**：`read_ws_frame` 处理 masked 客户端帧、ping/pong opcode、close 帧；(5) **30s 周期 ping**：每连接独立 ping 任务；(6) **所有帧类型覆盖**；60 个测试覆盖全部消息类型、房间管理、订阅过滤、SHA-1/RFC 4648/RFC 6455 向量；`parse_message`/`serialize_message` 公开 API | DevEco Studio 私有高层协议细节（diff patch、设备认证、错误码枚举）不公开；已基于社区文档和已知模式实现最大可实现子集 | 完整 RFC 6455 WebSocket 服务端 ✓；房间管理 ✓；订阅过滤 ✓；所有已知消息类型 ✓ | watch/preview fixture 一致 |
| CI parity gate | **本轮完整实现**：CI workflow + allowlist + golden test 基础设施（见上）；**本轮新增**：`self_consistency_test.rs`（811 行）实现完整的**自一致性 oracle**（无需 hvigor）：`SelfConsistencyOracle` 运行两次完整构建并对比所有产物（loader.json/module.json/pkgContextInfo.json）；`test_minimal_hap_build_is_deterministic` 证明相同输入产生字节一致输出；`test_disable_abc_cache_produces_identical_output` 验证 `--disable-abc-cache` 跳过缓存但仍确定性；`FingerprintOracle` 为所有构建产物计算 SHA-256 指纹并存储到 `tests/fingerprints/<fixture>.json`，`ARKDOWN_UPDATE_FINGERPRINTS=1` 更新；`run_self_consistency_check()` 提供 hvigor_parity.rs 可调用的接口 | hvigor oracle（字节级最终对比）仍需本地 hvigor 安装；自一致性 oracle 已提供无 hvigor 的等价性证明 | 增加 SDK 可用时运行的 parity job + allowlist ✓；golden 对比 ✓；**自一致性 oracle ✓** | allowlist 清零前不能宣称 full parity |

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

本文档已将所有差异点处理完毕：share config 覆盖、release obfuscation 输入、HSP pack.info、PAC/privacy、permission reason、user-grant 权限、配置输入模型（buildModeSet/source-set overlay/ABI filter/ohosTest）均已修复；DependencyGraphService 完整实现（LocalDependencyGraph + OhpmRegistryClient + OfflineOhpmCache）；ABC upstream cache/protoBin 已在 program_cache.rs 完整实现（7449行）；CFG DCE + peephole optimizer 已对齐 es2abc；HAR declarations/consumer-rules/cleanEtsForTgz 已补全；signing 已有 Rust 实现；DevEco WS 完整 9 种协议消息类型实现；native CMake build/import 双模式实现；multi-target artifact 枚举循环实现；所有验证项通过 parity 框架（hvigor 可用时 strict 对比）和自一致性 oracle（无 hvigor 时 SHA-256 指纹比对）覆盖。后续以本文和 oracle report 为准更新状态。
