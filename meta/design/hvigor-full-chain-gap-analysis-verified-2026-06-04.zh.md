# hvigor 全链路差异与对齐计划（代码核验版）

日期：2026-06-04

范围：当前仓库 `arkdown` 与 hvigor/OpenHarmony DevEco 构建链路在 HAP/HSP/HAR、ArkTS 转换、ABC、资源、打包、签名、缓存等阶段的差异。本文以当前源码和已有设计文档为准，不把历史文档里的“已完成”标记直接视为事实。

结论：当前实现还不能声明“编译结果与 hvigor 完全一致”。`minimal-hap` 等窄 fixture 已经有较多对齐工作，但全链路仍存在输入模型、资源/profile 后处理、DependencyManager 等价性、CompileArkTS handoff 顺序、release 混淆、ABC 后端、HSP/HAR/native/package metadata 等关键缺口。要确保修复后与 hvigor 完全一致，必须先建立可重复的 hvigor oracle 对比门禁，并让 fixture 矩阵的 allowlist 清零。

## 1. 一致性目标

“完全一致”需要同时满足：

- 同一项目、产品、target、build mode、SDK、签名材料下，hvigor 与 arkdown 接受或拒绝相同输入，错误级别一致。
- 所有 hvigor 可观察中间产物一致：`merge_profile`、`process_profile`、`resConfig.json`、资源产物、`loader.json`、`loader_out`、CompileArkTS cache TS、`filesInfo.txt`、`modules.cache`、`npmEntries.txt`、`sourceMaps.map`、importee cache、package metadata。
- 未签名 HAP/HSP/HAR 的 zip entry 顺序、压缩方法、CRC、size、content hash 一致。
- `modules.abc`、`widgets.abc`、`data.abc` 在 debug/release 下按字节一致，或存在经过确认的规范化比较器。
- 签名产物至少通过语义一致校验；如要字节一致，需要控制证书、profile、时间戳、签名块元数据和 zip 对齐。

## 2. 当前高优先级阻塞

| 优先级 | 阻塞项 | 当前证据 | 影响 |
| --- | --- | --- | --- |
| P0 | oracle 覆盖不足 | `crates/arkdown-cli/tests/hvigor_parity.rs` 只有 minimal HAP/HAR/HSP，且全部 `#[ignore]` | 无法证明全链路一致，也无法防回归 |
| P0 | 构建输入模型不完整 | `arkdown-config` 只解析 product/signing/strictMode/部分 arkOptions/workers/preloadSo | target/source-set/ability override/removePermissions/AOT/native 等输入丢失 |
| P0 | share config 被覆盖 | CLI 写入 `temp-share-config.json` 后又调用 `write_static_intermediate_files()` 写空数组 | `crossAppSharedConfig` 实际不会进入后续链路 |
| P0 | profile/resource 后处理不等价 | startup/share/privacy 函数存在，但没有完整 resource-phase rewrite | 最终 `module.json`、资源 profile 和 compile entries 不等价 |
| P0 | release 混淆作用对象错误 | obfuscation 修改临时 `.js` chunk，ABC 使用 `CompileArkTS` handoff inputs | release ABC 可能没有被混淆，无法与 hvigor release 对齐 |
| P0 | ABC 后端不是 es2abc 等价实现 | `arkdown-abc` 使用自定义 batch cache 和 Rust lowering | 大项目 ABC 字节、debug info、release optimizer、cache/protoBin 不保证一致 |
| P1 | HSP/HAR/package metadata 不完整 | HSP 复用 HAP pack，CLI 传 `bundle_type: "app"`；HAR 为 source HAR 简化路径 | HSP/HAR 产物与 hvigor 差异大 |

## 3. 全链路差异矩阵

### 3.1 构建配置与 target 解析

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| 执行 project/module `hvigorfile.ts`，插件可改变任务、profile、rollup 配置 | 只检测并 warning；支持 `arkdown.config.json` 少量 override | 自定义 hvigor 插件逻辑完全不执行 | 定义兼容扩展面；无法执行 TS 时，提供 hvigor effective-config oracle dump 对齐模式 | 使用 hvigorfile 改 ability/profile/rollup 的 fixture 有相同 effective config |
| 合并 product、target、source-set、buildModeSet、target source abilities | `RawTarget` 只解析 name/applyToProducts，未用于 effective target | target overlay、ability icon/label/launchType 覆盖缺失 | 引入 `EffectiveTargetConfig`，按 hvigor 顺序合并所有层 | effective target JSON 与 hvigor dump 一致 |
| `removePermissions` 影响 HAR/HSP 依赖权限合并 | 未建模 | requestPermissions 可能多或少 | 解析并在 profile merge 中按 hvigor 规则移除依赖权限 | permission-heavy fixture final `module.json` 一致 |
| AOT 选项 `aotCompileMode`、`apPath`、`hostPGO` 写入 loader | loader AOT 字段固定/default | AOT 项目 loader 不一致 | 解析 AOT 字段，复制 `.ap` 并按 hvigor 生成 `anBuildOutPut/anBuildMode/apPath/fallbackAnBuild` | AOT fixture `loader.json` 一致 |
| native/CMake、ABI、nativeLib 参与 build/package | 仅扫描现有 `.so` | 无法构建 native 输出，preload 只写 list | 增加 native artifact import/build 模式，按 ABI 收集 | native fixture 包内容和 metadata 一致 |
| `resOptions`、target resource directories、includeAppScopeRes 影响资源输入 | 资源输入固定 AppScope + modules `src/main/resources` | target 资源集、依赖资源集不等价 | 解析资源目录和 include 规则，写入 restool config | resource target fixture 一致 |

### 3.2 Profile merge、process profile、`module.json`

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| `MergeProfile`/`ProcessProfile` cloneDeep 后原地 mutate，保留未知字段 | `module` 部分已有 preserve-and-mutate；`app` 部分由 `build_app_section()` 白名单重建 | app 未知字段、sanitizer、multiProjects 等可能丢失 | app 也改为 preserve-and-mutate，仅覆盖 hvigor 会变更的字段 | app 字段保留 fixture 一致 |
| `module.dependencies` 来自 HSP 依赖 | `process_profile()` 中 `dependencies: vec![]` | HSP dependency metadata 缺失 | 从 DependencyGraphService 构造 HSP dependencies | HSP dependency fixture 一致 |
| `crossAppSharedConfig` 常见输入是 `$profile:<name>` 字符串 | `ModuleJson5Module.cross_app_shared_config: Vec<_>` | 字符串会解析失败或无法驱动任务 | 改成 `ProfileRefOrInlineShareConfig`，同时支持 `$profile` 和数组 | share config fixture 可解析且 final module 引用一致 |
| HAR/HSP 依赖 ability、metadata、permissions 合并 | 当前主要处理本 module | 依赖能力、权限、metadata 缺失 | 在 profile merge 阶段纳入 dep manifests | HAR/HSP dep profile fixture 一致 |
| appStartup 经 ProcessStartupConfig 后进入资源 profile 并更新 module 引用 | 原始 `appStartup` 可被 raw passthrough 保留；merged temp startup 不回写资源 | runtime startup profile 不等价 | resource 阶段写 merged startup profile，清理中间字段，更新 `module.appStartup` | startup fixture package 和 resources 一致 |

### 3.3 Share、startup、privacy、资源后处理

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| `ProcessShareConfig` 写 merged temp，并由资源阶段写 `resources/base/profile/<name>.json` | merge 函数存在，但 `write_static_intermediate_files()` 会覆盖 temp 为 `{"crossAppSharedConfig":[]}` | share config 当前实际丢失 | 先移除覆盖；随后实现 resource rewrite 和 final `$profile:<name>` | duplicate URI hard error；share profile 和 final module 一致 |
| `ProcessStartupConfig` 补 `moduleName/startupConfigPath/srcEntryAbsolutePath/ohmurl`，处理 dep 和 preload SO | 只补 `moduleName/startupConfigPath`，没有 `ohmurl/srcEntryAbsolutePath` 等完整字段 | startup compile entries 和 runtime metadata 不一致 | 按 hvigor 计算 OHM URL、绝对 srcEntry、remote ByteCode HAR 标记 | temp startup JSON 与 hvigor 一致 |
| resource 阶段处理 permission reason、shortcut、distro、form、share/startup/profile rewrite | 当前资源编译主要复制 profile/media/element，router 有单独处理 | 多个 profile/resource rewrite 缺失 | port `CompileResource` 后处理逻辑 | resource-heavy fixture `res/default` 一致 |
| `ProcessModulePrivacyProfile` 验证并复制 privacy/PAC | packager 发现 `privacy.json` 时写 `module.privacy.json` | 缺少 hvigor 的 validation/intermediate path/PAC 处理 | 增加隐私 profile 处理任务，产物从 intermediates 入包 | privacy/PAC fixture 一致 |
| restool config 包含 dependency resources | `resConfig.json.dependencies` 固定 `[]` | HSP/HAR 资源依赖缺失 | DependencyGraphService 提供资源表和依赖资源目录 | HSP resource lookup fixture 一致 |

### 3.4 `loader.json` 与依赖图

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| DependencyManager 分类 local/remote HAR、HSP、ByteCode HAR、SO、dynamic、bundled、transitive | 主要通过 `oh_modules` 文件系统扫描和轻量 BFS | 类型、顺序、remote cache 路径、bundled subtleties 不完整 | 建立 hvigor 等价 `DependencyGraphService` | dependency graph dump 与 hvigor 一致 |
| `modulePathMap/pkgBriefInfo` 来自完整 target module graph | `modulePathMap` 为 workspace modules；`pkgBriefInfo` 只含当前 module | dep package context 不完整 | 从 dependency graph 生成所有 package context | `pkgContextInfo` 和 loader map 一致 |
| `hspNameOhmMap/harNameOhmMap/hspResourcesMap` 基于 DependencyManager | 有实现，但依赖直接/transitive 扫描，`hspResourcesMap` 只看 module `oh_modules` | remote/transitive/order/normalized-off 模式未证明 | 统一由 dependency graph 和 OHM URL service 生成 | HSP/HAR transitive fixture 一致 |
| `compileEntry` 包含 abilities、pages、workers、runtimeOnly、startup、insight intent、dep ability、router 等 | 覆盖 abilities/pages/workers/runtimeOnly/router/otherCompileFiles 的一部分 | startup/insight intent/dep ability/bundled deps 等缺失 | 完整 port `GenerateLoaderJson.doTaskAction()` entry 收集 | compileEntry canonical diff 一致 |
| ByteCode HAR loader info、declarationEntry、updateVersionInfo、otherCompileFiles | 已有扫描和部分字段 | 仍非 hvigor DependencyManager；own declaration、runtimeOnly ByteCode HAR、bundled edge 未证明 | 从 graph 节点生成 ByteCode HAR 所有字段 | ByteCode HAR fixture loader 和 runtime install 通过 |
| AOT fields、patch config、test/preview fields | 多数固定/default | preview/test/AOT 产物不一致 | 按 hvigor emit 条件写字段 | AOT/preview/test fixture 一致 |

### 3.5 Rollup/Rolldown、ArkTS transform、CompileArkTS handoff

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| Rollup 3 + hvigor plugin stack，module order 为 JS `Map` insertion order | Rolldown 1.0.3；用 `ModuleIdx` 排序接近稳定顺序 | 大项目 `filesInfo.txt/modules.cache/npmEntries.txt` 顺序风险 | 显式记录 resolve/load insertion order，并按 hvigor 顺序写 handoff | handoff 文件顺序一致 |
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
| widget/form 独立 compile config/cache | `compile_widget_pass()` 存在 | widget loader/cache/ABC 字节未证明 | 使用同一 handoff/order 规则，补 form validation | widget fixture `widgets.abc` 一致 |
| ArkData 独立 compile | `compile_arkdata_pass()` 存在 | 真实 ArkData schema/diagnostic 未证明 | 增加 ArkData fixture 和 oracle | `data.abc` 一致 |

### 3.7 Release、混淆、minify

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| release treeshake true，supportChunks 时 manualChunks | treeshake 已按 mode；supportChunks 有路径 | 仍需 hvigor fixture 证明 side-effect/module chunk 行为 | 加 release chunk golden | chunk list/handoff 一致 |
| SDK obfuscation/minifier 作用于最终 compile inputs | 只改临时 `.js` chunk，ABC 使用 handoff TS inputs | release ABC 可能未混淆 | 将 obfuscation 移到 handoff 生成前，或直接混淆 handoff inputs | release ABC string/name diff 证明已混淆 |
| obfuscation rule engine 与 consumer config | Rust subset，HAR consumer rules 只是拼接 | 规则语义、order、cache invalidation 不等价 | port SDK rule 行为或 parity 模式代理 upstream | obfuscation fixture 一致 |
| terser/JSBUNDLE 特定插件 | ESMODULE 为主，JSBUNDLE 不支持 | JSBUNDLE/legacy 项目不等价 | 明确 unsupported 或实现 JSBUNDLE parity | JSBUNDLE fixture 一致或拒绝一致 |

### 3.8 Packaging、HSP/HAR、native、signing

| hvigor 行为 | arkdown 当前实现 | 缺失/差异 | 修复建议 | 验收 |
| --- | --- | --- | --- | --- |
| HAP/HSP/HAR 按不同任务生成 pack info、metadata、archive entries | HAP 较完整；HSP 复用 HAP；HAR 是 source HAR 简化 | HSP/HAR 专项产物未对齐 | 分 target 建 `PackInput`，从 effective app/module 生成 pack info | HAP/HSP/HAR archive diff 一致 |
| HSP `pack.info` moduleType/shared、bundleType/shared | CLI HAP/HSP 分支传 `bundle_type: "app"` | HSP metadata 错误 | HSP target 传 shared，并按 hvigor HSP pack info 生成 | minimal/shared HSP fixture 一致 |
| pack.info 来自 app/module/profile/deps | 版本号等多处固定 `1000000`/`1.0.0`/phone | 真实项目信息不一致 | 从 AppScope、profile、deps、target 生成 | pack.info diff 一致 |
| ByteCode HAR/source HAR/HAR declarations/obfuscation files | source HAR 基础路径；声明 stub 简化 | HAR 发布不等价 | 区分 source HAR 与 ByteCode HAR；补 declaration、consumer obfuscation、cleanEtsForTgz | HAR matrix 一致 |
| integrated HSP | 有 normalize 和 pack，但发现逻辑依赖 metadata + `<name>.hsp`，缺失则 warning skip | remote HSP 路径/metadata/order 不等价 | 从 DependencyGraphService 解析 integrated HSP，缺失时按 hvigor 错误级别处理 | integrated HSP fixture 一致 |
| native SO | 扫描现有 `arm64-v8a/*.so` | native build、ABI、compressNativeLibs、preload metadata 不完整 | 补 native build/import task 和 metadata | native/preload fixture 一致 |
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

- 一个固定命令同时 clean build hvigor 和 arkdown。
- 收集 stage artifacts：profile、resource、loader、CompileArkTS cache、source maps、ABC、zip、pack metadata、signing metadata。
- 提供 JSON canonical diff、zip diff、ABC section diff、cache TS diff、resource table diff。
- fixture matrix：minimal HAP debug/release、HSP、source HAR、ByteCode HAR、runtimeOnly、dynamic import、router、share、startup、privacy/PAC、widget/form、ArkData、native/preload SO、integrated HSP、normalized OHM on/off、release obfuscation、supportChunks、target overlay、removePermissions。

验收：CI 能输出阶段级 diff；allowlist 有 owner 和过期时间。

### Phase 1：修配置和 profile 架构

任务：

- 建 `EffectiveTargetConfig`，合并 product/target/source-set/buildModeSet。
- app/module profile 都改为 preserve-and-mutate 存储，typed struct 只做 view。
- 支持 `crossAppSharedConfig` 的 `$profile` 字符串和 inline array。
- 实现 removePermissions、ability overrides、HSP dependencies、HAR ability/permission/metadata merge。

验收：profile-heavy fixture 的 `merge_profile/process_profile/module.json/ark_module.json` 与 hvigor 一致。

### Phase 2：修 resource-phase 后处理

任务：

- 删除 `temp-share-config.json` 被静态空文件覆盖的问题。
- 将 share/startup merged temp 写入 `resources/base/profile` 并更新 final `module.json`。
- 补 startup `ohmurl/srcEntryAbsolutePath/moduleName/startupConfigPath`。
- 补 permission reason、shortcut、distro、form、privacy/PAC、preload SO 后处理。
- `resConfig.json.dependencies` 从 dependency graph 生成。

验收：share/startup/privacy/resource-heavy fixtures 的资源目录、profile 引用和 package 内容一致。

### Phase 3：建立 hvigor 等价依赖图并重写 loader 生成

任务：

- 引入 `DependencyGraphService`，节点区分 local/remote HAR、HSP、ByteCode HAR、SO、dynamic、runtimeOnly、bundled、transitive。
- 所有 `loader.json` map、compile entries、pkg brief info、ByteCode HAR fields、AOT fields 都从该图生成。
- 提供 normalized 和 old OHM URL 两套 golden。

验收：`loader.json/pkgContextInfo/importee/filesInfo` 在 dependency matrix 中一致。

### Phase 4：稳定 CompileArkTS handoff

任务：

- 显式记录 hvigor Rollup Map 等价的模块插入顺序。
- 以 cache TS/source map golden diff 补齐 ViewPU、ComponentV2、paramsLambda、printer/comment、diagnostics。
- 把 release obfuscation 作用到最终 handoff inputs，而不是临时 chunks。

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
- HSP `bundle_type/moduleType` 修为 shared。
- ByteCode HAR/source HAR 两条路径拆开。
- integrated HSP/native/preload SO 从 dependency graph 和 native task 产物进入 packager。
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

1. 修掉 `temp-share-config.json` 被覆盖，并加回归测试。
2. `crossAppSharedConfig` 输入改成 `$profile`/array union，避免真实项目 parse 失败。
3. release obfuscation 改为作用于 CompileArkTS handoff inputs。
4. HSP packaging 传 shared bundle/module type，并从 AppScope/module profile 生成真实 pack.info。
5. `resConfig.json.dependencies` 和 `hspResourcesMap` 改由依赖图生成。
6. 建最小 oracle CI：先覆盖 minimal HAP debug/release、HSP、source HAR、share/startup、release obfuscation。
7. 再推进 ViewPU/cache TS/ABC 字节级对齐。

## 6. 当前不能直接采信的历史结论

仓库中存在一份 `meta/design/hvigor-full-gap-analysis.md`，末尾写着 “95/95 100% addressed”。按当前源码核验，这个结论过强：至少 share config 覆盖、release obfuscation 输入、HSP pack.info、配置输入模型、DependencyManager 等价层、ABC upstream cache/protoBin 仍未闭环。后续应以本文和 oracle report 为准更新状态。
