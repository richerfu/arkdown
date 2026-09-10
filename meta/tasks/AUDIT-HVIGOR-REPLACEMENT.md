# Hvigor 替换全链路审计

审计日期：2026-09-10。Arkdown 起始基线：
`9915ed8971dfb6d60fb0072792084dfce492963c`。

## 证据和判定规则

实现依据是当前 `developtools_ace_ets2bundle`、其
`docs/kb/06-hvigor调用链分析.md`、当前 `arkcompiler_ets_frontend` 和各委托
编译器/SDK owner。旧 Hvigor 反编译树和旧 specs 不作为证据。生成产物、缓存、
诊断与 sourcemap 只能定位或验证差异，不能定义算法。源码没有定义行为时必须
停止并询问，禁止新增 heuristic、fallback 或内部 Hvigor facade。

语言版本允许由产品配置覆盖，属于已确认产品行为，不是替换缺口，后续不得再次
列为问题。本轮优先保证 ArkTS 1.1；1.2 的静态 SDK driver 认证不混入 1.1
checker 或插件结论。

## 当前能力结论

| 领域 | 上游行为 owner | Arkdown owner | 结论 |
|---|---|---|---|
| 唯一入口 | Hvigor wrapper 的 argv/stdio/exit 生命周期 | `packages/arkdown/js/arkdown-cli.js` + typed N-API | 已对齐目标架构；Rust 无 CLI |
| 项目/模块/target 计划 | OHOS task 组装 loader、resource、compile、package、sign DAG | `arkdown-build` 与领域 crate | Rust 直接拥有，不保留 task facade |
| 依赖解析 | loader config、OH resolver、package graph | `arkdown-session`、`arkdown-resolver`、TSGO OH resolver | 已接入已物化依赖；构建不隐式安装 |
| ArkTS 1.1 checker | OH TypeScript parser/binder/checker/linter；ets2bundle `ets_checker.ts` | packaged TSGO + `ProjectSemanticChecker` | 源码定义能力差异为 0；固定 `noEmit`，ESMODULE 使用 ES2020 module，Stage/FA 使用源码的 `moduleJson` 分界，normal/strict checker 共享 callback executor；TSGO 经双向 API 回调 JS CLI 进程内的 SDK plugin host，自身不启动 Node；SDK class callback 接收同一份完整 compile `projectConfig`，旧 Rust subset checker 已删除 |
| checker-driven transform | `ohApi.ts`、component transforms、`checkCrossplatformValue`、`processImportModule` | TSGO batched facts + OXC consumers | annotation、builder、enum、SDK dependency 已接通；Builder 与 SDK component 的 named/default/namespace import 按 aliased declaration 解析；Program 路径经 SourceStore 映射回 Rolldown snapshot identity；Rust 不重做类型推断 |
| compile-plugin | arkts-compose `getPlugins/doWrapper/getCompilePluginContext` | JS function ABI + N-API + Rolldown adapter | hook、顺序、share、cache、watch、TS/JS module loading 已接通 |
| plugin `this.parse` | Rollup 4.23.0 `ParseAst` | OXC ESTree bridge | 已按源码只消费 `allowReturnOutsideFunction`；未知 Acorn 字段忽略；错误为 `RollupError/PARSE_ERROR` |
| SDK/native tools | ets2bundle command builders、SDK binaries | typed Rust plans | restool/ABC/linker/signer/PandaGuard 等直接调用实际工具；不是 JS 兼容层 |
| source ArkGuard | ets2bundle obfuscation config + arkcompiler ArkGuard source | `arkdown-obfuscate` OXC implementation | Rust 原生 owner；name/file/property cache 与 consumer rules 进入 build stage |
| cache/watch | ets2bundle cache/watch hooks | task cache、OXC facts、Rolldown retained graph、watch coordinator | retained TSGO/Rolldown 与 HotReload 增量实跑已通过；下述仅有性能收口 |
| HAP/HSP/HAR/APP/HQF | OHOS packaging/signing tasks | `arkdown-pack`、`arkdown-sign`、app coordinator | 1.1 strict fixtures 进入真实产物链；2 HAP + 2 HSP APP 及已签名 HAP/HSP/APP/HQF 零差异 |

TSGO 完成矩阵、发布方式和回归见
[ArkTS 1.1 原生 TypeScript 对齐合同](ARKTS11-NATIVE-TYPESCRIPT.md)。

本轮发行门禁已通过：N-API release 构建、JS 28/28、Rust workspace test、
Clippy `-D warnings`、rustfmt 和 diff check 均成功；不设置
`ARKDOWN_TYPESCRIPT_PATH` 的 JS CLI `check` 能从本地平台包发现 TSGO，npm
platform package dry-run 同时包含原生绑定、`tsc`、clean revision 构建清单、
许可证与 NOTICE。Linux/Windows 仅声明交叉构建和 hash 验证，不声明对应平台运行。

## 已删除的多余实现

- Rust isolated-declarations surface/IR/link/infer/relate/print checker；
- Rust/OXC SDK declaration symbol、overload、alias、factory-return 伪推断；
- SDK Node checker fallback、TSGO 内嵌 Node worker、双 checker 路径和 annotation 文本改写；
- Acorn 旧版 `sourceType/ranges/locations/onComment/onToken/program/ecmaVersion`
  等 `PluginContext.parse` 兼容层。当前 Rollup 4.23 `ParseAst` 不消费这些字段；
- 失去生产 caller 的 plugin crate、facade、wrapper 和旧文档状态。

## 当前差异清单

### TSGO：源码定义能力差异为 0

当前适配后的 TSGO 提交 `0780074afa51745e9be859be1ad9400a7f61dbf6` 已覆盖 OH parser、
binder、checker、ArkTS 1.1 linter、OH resolver、SDK function/class callback、
诊断筛选、retention 与 Rust/OXC 所需类型消费者。OH linter 当前源码语料共
1004 个 source case，1004/1004 执行且无跳过；两份只承载 JSON 数据、没有
`.ets/.ts` 输入的目录按 OH runner 规则不属于 source case。

本轮补齐了最后一个真实链路缺口：ArkTS linter 为 strict 模式 clone compiler
options 时曾丢失私有 SDK callback executor，导致 HarmonyOS `5.0.5(17)` 被错误
按整数比较并误报 API 11/12。TSGO 现在让 normal/strict checker 共享同一宿主
executor；native boundary 与 JS/N-API 真实构建都只保留 SDK 插件判定的 API 20
告警。

TSGO 只负责类型检查、推断、诊断和只读 semantic facts，不负责 JS/声明生成或
代码改写。watch 已按 `rollup-plugin-ets-checker.ts::etsChecker` 生命周期保留每个
模块/目标的 TSGO session：首轮创建 Program，后续轮次把 watcher 的
modified/created/deleted 事件与新 immutable source snapshot 传给
`createProgram(oldProgram, fileChanges)`，再按原顺序重新收集 source graph、linter、
syntax/semantic/declaration diagnostics 和 transform type facts。JS 不参与该链路。

checker roots 已按 ets2bundle `loadEntryObj/getEntryObj/rootFileNamesCollect` 改为
loader 的 `entryObj + ohExports`，由 TSGO 扩展为可达 source graph；递归扫描模块
目录并把未被入口引用的文件送入 checker 的旧路径已经删除。TSGO 同时补齐 OH 4.9
generic candidate/constraint、common-supertype、signature relation 和 union subtype
reduction 顺序，真实业务 `Map<string, ICanvasDraw<any>>` 构造不再被错误收窄。
含计算成员的 numeric enum 同样按 OH 4.9 建模，不会触发新版 enum-union 的错误
穷举。ObjectLink 的 nullable/basic/ObservedV2/Function 身份由 TSGO 返回，Rust
不再从类型语法猜测。

真实业务工程暴露的 `@ObjectLink` 失败不是 checker 规则差异，而是文件身份断链：
macOS 大小写不敏感文件系统上，TSGO Program 对目录入口返回 `index.ets`，Rolldown
启动快照保留实际 `Index.ets`，导致 OXC 查询没有送入 checker。当前实现按照
`ets_checker.ts::getCanonicalFileName` 与 TSGO `tspath.GetCanonicalFileName`，在
`SourceStore` 启动阶段建立碰撞安全的 canonical-file 索引；RPC 使用 compiler path，
OXC source range 与最终 facts 使用 snapshot path。该修复同时覆盖 ObjectLink、
annotation、builder receiver、enum member 与 SDK dependency 等所有类型消费者。

同轮源码审计确认：`processImportModule/getDefinedNode` 对 default import 使用
`getAliasedSymbol` 取得被装饰的 Builder 声明，对 namespace import 则由
`getIntegrationNodeInfo/processImportNode` 遍历导出并把 `usedPropName` 注册到
`CUSTOM_BUILDER_METHOD`，`transferBuilderCall` 保留原始限定 callee。OXC 的
module-export closure 和 SDK declaration metadata 现完整覆盖 named/default/namespace
三种 Builder import；namespace SDK component 同样携带声明属性元数据，未使用 JS
兜底。跨文件 default/namespace Builder 已由回归测试验证，default 路径同时经过真实
业务构建验证。`FlowItem` 则由
`process_component_build.ts::checkNonspecificParents` 和 `flow_item.json` 明确定义为
只能位于 `WaterFlow` 内；上游会报 10905201，因此这属于业务源码错误，不是 Arkdown
兼容缺口，也不得在 transform 中放宽。

SDK function/class 回调现在由 TSGO checker workers 聚合成有序 transport batch，
Rust typed boundary 转发给 JS runtime；batch 不去重、不缓存结果，每个源码调用仍按序
执行。Source/projectConfig 只做 wire interning，并由 Rust 为每个 TSGO 进程分配唯一
host scope，避免多模块并行构建中进程本地 session id 重复。build 现在通过单个
`getArkTSBuildDiagnostics` 请求复用 OH `TSDiagnostics.ts` 已计算的 normal semantic
结果，并使用 TSGO 依赖图分区编译池，避免 API/LSP 单 checker 串行路径。真实工程
13205 个 Program source、1739 个 transform-query 文件上，checker 从旧链路约
206.4 秒降至约 133.4 秒，减少约 73 秒（35%）；完整缓存构建 208.78 秒并成功生成签名
HAP。SDK 插件仍逐次执行，未以缓存返回值换取性能。

用户显式选择 `build --skip-checker` 时，Arkdown 跳过 linter、syntactic/semantic/
declaration diagnostics，但保留 OXC 生成所需 TSGO type facts；该模式实测 checker
约 7.2 秒、完整无缓存构建 167.56 秒并生成签名 HAP。它是 opt-in 扩展，默认构建仍
执行源码定义的完整 checker，独立 `check` 不受影响。

相邻审计还确认，未 emit 的 TypeScript namespace 与上游
`rollup-plugin-ets-typescript.ts::transform` 一样以 AST/打印源码交给 CompileArkTS；
Rolldown 在仅用于模块图扫描的 JS 投影上会为 namespace 内的 exported `let` 产生
`TOLERATED_TRANSFORM`。该投影不参与 module-source/ABC 产物，因此 Arkdown 只在 scan
warning 边界过滤这一 Rolldown 专属诊断，保留实际 TypeScript source 和其他具有上游
Rollup 对应项的 warning。真实工程最终日志已验证该噪音诊断为 0。

Rolldown 热路径也已按当前插件源码收口：
`check-duplicate-filenames-plugin.js::load` 的 Map/Set 收集改为分片并发 facts，并在
告警边界稳定排序；`hsp-collect-har-files.js::buildEnd/collectSourceFiles` 改为直接从
`PluginContext::get_module_ids` 的最终图收集，不再给每个 `moduleParsed` 增加共享写锁。
没有生产消费者的 `HarDedupFact/facts.rs` facade 已删除。

### 当前源码未定义：Hvigor task/system plugin ABI

当前权威 `developtools_ace_ets2bundle` 源码定义的是 arkts-compose
compile-plugin 与 Rollup hook，不定义 `HvigorNode.task/registerTask/getTaskByName`
等 Hvigor task/system plugin 的公开对象和生命周期。Arkdown 已完整接入当前源码
定义的 TS/JS compile-plugin ABI；此前按方法名猜测的 task registry、task hook、
configure-cmake hook 与 watcher facade 已删除，不能把它们当作待补齐实现。

因此这部分不计入当前源码范围内的能力差异。若要把 task/system plugin 纳入替换
合同，必须先提供定义参数、返回值、依赖边、cache 与生命周期的当前权威源码；在
此之前既不能声称支持，也不能创造一个近似兼容层。

### 已完成：test/ohosTest 与完整 strict matrix

`test`、`ohosTest` 已恢复完整 build/cache/loader/package 收集。源码确认最终 HAP
差异来自 `generate_sourcemap.ts` 并发 transform 写入导致的 JSON 成员顺序，而非
代码语义；comparator 现在只在归一化 ZIP/TAR member 时按 JSON 数据语义排序，
没有跳过目录或产物。`coverage.log` 只归一化当前 OH coverage parser 不消费的
进程前缀和 Hypium wall-clock duration，保留 class/test/result/status-code/stack/
coverage records。ArkTS 1.1 strict Hvigor suite 当前 37/37 通过，其中 24 个测试项
执行真实 Hvigor 构建或诊断（签名组合测试在单项内执行 HAP/HSP/APP 三组构建），覆盖
HAP、HSP、HAR、HQF、ByteCode HAR、test、ohosTest、APP、multi-module、incremental、
release、startup/share/widget，其余为 comparator、diagnostic 与 harness 合同测试。

本轮扩展真实 project-mode `assembleApp` 对比到 2 个 HAP + 2 个 HSP，递归比较
APP 内的 entry/feature HAP、普通/form-widget HSP、interface HAR、ABC、profile、
资源、`pack.info`、`pac.json` 和 legacy `pkgSdkInfo.json`。严格矩阵据此定位并按
源码修复了以下差异：

- `GeneratePkgSdkInfo` 仅在非 normalized OHM URL 下生成 legacy package inventory；
  它按 module tree、project tree 的顺序遍历依赖，在 HSP 节点停止，并将原始字节紧跟
  `pack.info` 写入 HAP/HSP；
- `ProcessProfile.processFormExtensionAbilities` 只从 restool 输入移除 HAP 的 form
  ability，`CompileResource.recoverFormExtensionModule` 在 restool 后从
  `ark_module.json` 恢复，最终 HAP profile 不丢失 form ability；声明
  `formWidgetModule` 的 HAP 本身不执行配对 HSP 的 widget compile pass；
- `GenerateOhosTestTemplate` 的迁移源码根是
  `.test/<product>/intermediates/src/ohosTest`，不是普通 `build` 中间目录；
- `GenerateLoaderJson` 对普通构建也显式写 `customizedHar: false`；
  `CacheNativeLibs.refreshBinxoFileCache` 在未启用 ASAN/BinXO 时仍写空 cache 和
  `enableAsanBinxo: false`；
- `PrepareSharedHarRes.generateModuleJson` 的 interface HAR 白名单包含
  `module.packageName`；未被当前模块依赖图引用的 workspace HSP 不得自动写入
  `module.dependencies`；
- generated `ResourceTable.ts` 的 OHM URL 使用 owning module，package/cache owner
  独立保留 package metadata；TypeScript emit 与 retained TS printer 的 source-map
  语言事实分别保存；
- `process_kit_import.ts` 对未改名 kit import 保持 `{ X }`，不能打印成
  `{ X as X }`；ArkGuard 的 SDK index-signature 参数进入原始 API 白名单，合并
  source map 时精确保留上一阶段 `sourceRoot` 的存在性。
- `ArkCompile.taskShouldDo` 在选中 source set 存在 ETS `CodeModel` 或
  `checkPackageMain` 找到 OHPM `main`（缺省 `Index.ets`）时启用；
  `CoreSourceSetImpl.initCodeMap` 只根据物理 `<source-set>/ets` 目录创建模型。因此
  resource-only feature 即使 loader 含生成的 `ResourceTable.ts` 也不执行 checker、
  Rolldown、ArkGuard 或 ABC；根目录带 `main` 的 ByteCode HAR 仍正常编译。
  `BasePackHapTask.generateCommand` 在 loader-out `ets` 目录不存在时不传
  `--ets-path`，最终 resource-only HAP 合法且不含 `ets/`。

修复后 APP、HAP/HSP/HAR/HQF 及完整 37 项 strict suite 均为零差异。

### R3：缺少独立真实矩阵的 1.1 路径

本轮已经从 JS CLI 补做以下真实执行验证：

- AOT `hostPGO + apPath` 生成 `anBuildMode=partial`、复制模块 `.ap`，并将零字节
  `.an/.ai` 打入 HAP。该结果不是 placeholder：当前
  `compiler/src/gen_aot.ts::generateAot` 只构造 `ark_aot_compiler` 命令文本，随后
  以 `openSync(..., 'w')` 创建这两个空文件，并未执行该二进制；Arkdown 不得擅自
  增加上游没有的执行步骤。
- HotReload 首轮由真实 es2abc 生成 `symbolMap.map`；同一 JS CLI 进程收到 `.ets`
  watcher 事件后写出 `modifiedFiles`，并以 `--input-symbol-table --hot-reload`
  生成 patch `modules.abc` 与 `sourceMaps.map`。macOS FSEvents 验证必须在允许文件
  事件送达的运行环境执行，沙箱内无事件不属于编译行为差异。
- ColdReload 审计定位并修复了真实差异：旧实现忽略 loader
  `patchConfig.oldMapFilePath/changedFileList/patchAbcPath`，错误使用内部固定目录。
  现在按 `main.js::readPatchConfig` 与 `module_coldreload_mode.ts` 消费字段，并在
  `changedFileList` 缺失时使用 `<cachePath>/changedFileList.json`。真实 es2abc 已在
  指定目录生成 cold patch ABC/sourcemap；`PackageQuickfix` 随后通过 SDK
  `app_packing_tool --mode hqf` 产出 HQF，归档包含 `patch.json`、
  `ets/modules.abc`、`ets/sourceMaps.map`。
- `collect-importers-plugin.js` 的 initial/reload 两段源码图状态已经迁入 Rust：首轮
  按 Rolldown entry/importedIds 广度遍历写 `compiledFileList`，ColdReload 过滤基线
  不存在的新文件并计算 `isImportNewFile`；`PrepareQuickfix.collectEts` 只追加
  changed-list 的 `mtimeMs`。`module_coldreload_mode.ts` 定义的两空格 source-map JSON
  写出格式也已对齐。已签名 HQF 现在由真实 HotReload symbol table、ColdReload ABC、
  SDK HQF packer 和 `hapsigner` 串联生成，归档、build 中间件和密码学验签均零差异。
- 已物化 `file:` Source HAR 依赖的多模块 fixture 能由 JS CLI 生成 APP；扩展后的
  form-widget project fixture 严格验证 2 HAP + 2 HSP、项目 `pack.info`/`pac.json`
  和嵌套归档，其中 resource-only feature 验证无 ETS source set 的任务禁用与无 ABC
  打包语义。构建服务没有为了 sample 隐式执行安装，缺少 `oh_modules` 时的 resolver
  错误属于正确前置条件。

ArkGuard 已补齐当前 `ConfigResolver` 中此前遗漏的
`-remove-nosideeffects-calls` 与 `-keep-uncompact`，并从 JS CLI 完成双轮 release
实跑：默认/显式 name cache 相同，新增声明后原 identifier/property/file 映射零
变化，调用删除与逐文件 compact 行为符合 `DisableConsoleTransformer` 和
`ArkObfuscator.writeObfuscationResult`。显式 `-print-namecache` 不再擅自创建父目录。
`-extra-options strip-language-default` 现在按 `ArkObfuscator.init` 切换 optimized
语言预置。IdentifierCache 使用 `ScopeAnalyzer.Scope.loc` 的命名/匿名作用域路径，
函数项携带经前序 source map 回溯的起止行，不再持久化 OXC `ScopeId` 或按名称后缀
猜历史缓存。`-print-kept-names` 已按 `writeUnobfuscationContent` 输出固定
`keptReasons`、逐文件/属性命中原因和 `whitelist.json`，合并上一轮未重编译文件并
覆盖本轮文件；显式路径按规则文件解析并创建父目录。保留名不再扁平化：SDK、语言、
规则、`-keep-dts`、AtKeep 与 InsightIntent 分别进入 property/global/local 集合，
`TransformUtil.isReservedTopLevel/isReservedProperty/needToReservedLocal` 的三条决策路径
由同一 Rust reason inventory 驱动，转换与 keptNames 不会使用两套规则。
HAR 按 `addPropWhiteList` 保留全部枚举成员；非 HAR 按
`ApiExtractor.shouldCollectEnum/visitEnumMembers` 在任一 initializer 含 identifier 时保留
该枚举的全部成员，而不是保留 initializer 中引用的标识符。

真实 SDK PandaGuard 组合已经从唯一 JS CLI 完成 release HAP 验收。该用例使用 SDK
`panda_guard` 生成不同于 origin ABC 的 obf ABC；loader-out ABC 与 HAP 内
`ets/modules.abc` 均逐字节等于 obf ABC，PandaGuard 改写后的 source map 同步到
intermediate、mapping 和 symbol 输出，nameCache 也同步到 symbol 输出。首次实跑还
定位并修复了生成期 `ResourceTable.ts` 被错误要求存在于启动快照的问题：上游在资源
阶段生成后通过 `resourceTableTs` 加入 ArkGuard 输入；Rust 现在从已物化的 typed
CompileArkTS module file 携带该源码，不做运行期兜底读盘。

签名主链已经使用当前发布的 `hapsigner 0.2.0` 原生 API 覆盖 HAP、HSP、HQF 以及
APP 内嵌 HAP/HSP；真实 PKCS#12、证书、签名 profile 和 DevEco 加密密码材料由严格
fixture 生成，全部产物同时通过 Hvigor 零差异比较与 `ApplicationVerifier` 验签。

HAR 签名是唯一明确保留的签名遗留项。当前 Hvigor `PackageSignHar` 调用
`sdk-sign-tool-full.jar sign-har`，其协议是对含 `package/` 的任务目录原地处理，不是
HAP/HSP/HQF 的 archive-signing 协议。当前 `hapsigner-rs`
`e8be862ec0998233ad6e90b70c4c4a7608249783` 尚未提供该 HAR 目录协议，因此 Arkdown
继续由 typed Rust runner 直接调用 SDK Java 工具；这不是内部 Hvigor 兼容层，也不在
Arkdown 内复刻协议。待 `hapsigner-rs` 原生支持 HAR 后再迁移，并以真实签名 HAR
严格矩阵和验签作为删除 SDK backend 的门禁。

每个后续场景加入时仍必须比较其上游任务实际消费和生成的结构，不可只检查
command/config 或 `contains` 片段。

## 性能项，不作为产物能力缺失

- Rolldown 暂无当前接入可安全使用的跨进程完整 graph deserialize API；新进程
  仍需 resolve/parse/scan。不得创建空 graph cache 或私有伪格式。
- 模块 dependency wave 仍可增加并行度；提交前必须按 typed ID 排序，保证产物
  与诊断稳定。

## 下一步顺序

1. 按具体 owner 扩充剩余 R3 真实矩阵；HAR 原生签名迁移等待 `hapsigner-rs` 提供
   目录签名协议，迁移前保留 SDK `sign-har` backend。
2. 功能门禁通过后处理剩余跨进程 graph cache 和依赖 wave 性能项。
3. 只有取得当前 task/system plugin 权威源码后，才能决定是否扩展公开插件合同。

完成门槛是 JS CLI/Node API 进入 typed Rust service、ArkTS 1.1 strict fixture
零跳过/零 allowlist、目标插件 ABI 回归、workspace test/clippy/format、真实
HAP/HSP/HAR/APP/HQF 结构验证、HAP/HSP/APP/HQF 签名验证，以及没有内部 Hvigor
兼容层。HAR 原生签名在 `hapsigner-rs` 提供目录协议后单独完成，不阻塞此前范围。
不得用“尚未证明全部兼容”代替上述具体结果，也不得用单个 sample 成功宣称全行为完成。
