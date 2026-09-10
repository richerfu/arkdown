# OXC/Rolldown 替换 Hvigor：当前架构与剩余工作

状态：ArkTS 1.1 源码定义主链与 TSGO 能力已接入，strict 37/37 零差异；HAR 原生签名迁移等待 hapsigner-rs，其余为性能收口
更新日期：2026-09-10

最新审计见 [全链路对照](AUDIT-HVIGOR-REPLACEMENT.md)。下文历史的
build-complete 表述仅指 SDK 链路接入，不能视为 Rust/OXC 业务迁移或全行为
等价已经完成。语言版本允许由 product 配置指定，不属于替换差异。

本文档是替换工作的唯一状态文档。完成项不保留迁移流水账；历史以 Git
提交为准。工程规则以根目录 [`AGENTS.md`](../../AGENTS.md) 为准。

## 1. 目标和证据规则

最终目标是由 OXC、Rolldown 和 Rust 领域 crate 直接实现现代
Stage/ESMODULE 工程的构建行为，并从公开 JS CLI 完成 HAP、HSP、HAR、
APP 和 test 输出。Hvigor 是行为目标，不是需要保留的内部架构。

行为实现必须先阅读下列源码中的实际 owner：

- 当前 `developtools_ace_ets2bundle` 源码 checkout
- 当前 `arkcompiler_ets_frontend` 源码 checkout

文档、旧 Hvigor 反编译代码、旧 specs 和历史实现只能用于定位，不能作为
行为依据。缓存和构建产物只能用于核验源码理解。若源码没有定义行为，停止
实现并向用户确认；禁止根据现象猜测、补 fallback 或创造新的兼容语义。

每项差异必须遵循同一流程：

1. 找到上游源码 owner、调用入口、输入和输出。
2. 将行为归属到 arkdown 的 Rust 领域 owner；只在公开插件 ABI 需要时使用
   JS/N-API。
3. 添加保护该源码契约的单元、fixture 或真实 Hvigor 对比测试。
4. 删除已经失去生产调用者的旧 wrapper、adapter、alias、re-export、测试、
   依赖和文档。
5. 无法按源码实现时返回 typed diagnostic，不能静默降级。

## 2. 最终边界

```text
packages/arkdown/js/arkdown-cli.js
  argv / help / stdio / exit code / TS-JS plugin loading
                         │
                         ▼
                  typed N-API boundary
                         │
                         ▼
                 arkdown-build application
        config + immutable BuildSession snapshot
                         │
             Rolldown graph + Rust plugins
                         │
              OXC transforms + native TypeScript API checker
                         │
       resource/profile/native/ABC/obfuscation
                         │
                  package and sign
```

- `packages/arkdown/js/arkdown-cli.js` 是唯一 CLI。Rust 不得增加 binary、
  argv parser、命令生命周期或 CLI-shaped N-API export。
- JS 可以并且应当承担入口、值转换、stdio/exit policy，以及原有 TS/JS
  插件函数与上下文对象的兼容。
- 项目发现、配置合并、任务顺序、缓存、SDK 命令和产物生成必须由 Rust
  直接拥有。
- 内部不保留“Hvigor 兼容层”。与 Hvigor 同名但没有独立领域职责的 facade、
  task wrapper 和双轨实现必须删除。
- 构建不隐式执行 `ohpm install`。arkdown 消费已经物化的 package graph；
  git/workspace 等安装协议属于 `ohpm-rs`，不能复制进本仓库。

## 3. 当前实现

### ArkTS 1.1

语言版本选择优先级为：显式 `--arkts-version` / 等价 N-API 字段 >
所选 product 的 `arkTSVersion` > 默认 1.1。product 配置为 1.2 时，无需再传
CLI 参数；check 使用 product 语言版本同样是合法行为，不得列为替换缺口。
此规则按用户 2026-09-07 的确认取代“只能通过显式 CLI 参数启用 1.2”的旧约束。
不得仅根据源码、依赖或 SDK 内容擅自升级配置所选版本。

当前 1.1 主链已经由 Rust owner 完成项目快照、解析、OXC 变换、Rolldown
模块图、ABC、资源、profile、native、混淆、缓存、watch、打包和签名。
公开入口只有 JS CLI。真实 Hvigor fixture 矩阵在 strict 模式下要求所有
归一化产物相等，不再允许 allowlist；当前 37/37 通过，其中 24 个测试项执行真实
Hvigor 构建或诊断，并包含 project-mode APP 对其嵌套 HAP/HSP 的递归比较以及真实
已签名 HAP/HSP/APP/HQF 的验签。

这表示当前 fixture 覆盖范围内为零差异，不表示未被源码语料覆盖的行为可以
凭空视为完成。新增源码契约时必须继续扩充矩阵。

### TS/JS 插件

JS/N-API 边界保留活的函数句柄和上下文对象，Rust/Rolldown 拥有 hook 顺序、
模块图、watch 事件和缓存生命周期。ArkTS 1.1 已接通 graph、Hvigor 生命周期、
cache 和 watch hooks。ESMODULE 上游设置 `generateModuleGraphOnly` 和
`disableGenerateBundle`，因此 output hooks 可以被识别但不执行；不能伪造
JavaScript chunk 来触发它们。

TS、MTS、CTS 插件由 OXC/Rolldown 编译。插件 `meta`、side effects、watch
files、共享状态和逐插件 cache 必须继续通过 typed bridge 持久化，JS 不能
取得构建编排所有权。

### Checker

当前原生 TypeScript 专项仅覆盖 ArkTS 1.1，按用户确认不考虑 1.2；
具体源码 owner、实施顺序及回归门禁见
[ArkTS 1.1 原生 TypeScript 对齐合同](ARKTS11-NATIVE-TYPESCRIPT.md)。
以 OH 源码为准，不移植 OXC 独有扩展。TS 只负责类型检查与推断（包括声明
诊断），Program 固定 noEmit；代码变换、JS/声明生成及运行时 annotation
metadata 由 Rust/OXC/Rolldown 实现，按需消费类型查询结果。

`arkdown-ets-checker` 已接入 TypeScript
`94e2465a5f92c50eb86a000eb1c4fb95adac3fd0` 原生 API。Rust 管理
工程输入、源码 snapshot、进程和 OXC consumer，Go 负责 parser/binder/type
checker、OH resolver、SDK callback 选择和独立 ArkTS 1.1 linter；TSGO 不启动
Node、不嵌入 JS，也不保留旧 isolated-declarations/Unknown checker 兜底。SDK
配置 CommonJS function/class callback 时，TSGO 经双向 API 请求、Rust typed route
调用 JS CLI 进程内的 build-scoped 插件宿主；工程发现、类型推断、比较、诊断和
fallback 仍由 Go/Rust owner。
双向 SDK callback 允许合并 transport frame，但禁止省略、去重或缓存源码定义的插件
调用；每个 TSGO 子进程必须映射到独立的 JS host scope，避免多模块并发复用进程本地
session id。
class callback 接收与 compile-plugin 完全相同的 Rust 构建 `projectConfig` 快照；
ArkTS linter strict checker clone 与 normal checker 共享同一 SDK callback executor；
ESMODULE checker 的 TypeScript module kind 按源码设置为 ES2020；独立的 source
model 在 Stage 工程使用源码定义的 `moduleJson` sentinel，在 FA 工程保留非空
compile mode，不能混为同一个字段。

annotation retention/常量、Mutable/WrappedBuilder、enum member、SDK
crossplatform dependency 等类型事实已批量接入 OXC。SDK dependency 身份直接
来自上游 `checkCrossplatformValue` 所在的 declaration-use 回调，Rust 不再
重建 overload、alias 或 factory-return 推断。build/check compiler options、
source graph、声明诊断和源码定义的诊断筛选已经贯通。三平台 clean TSGO 产物
随 N-API 平台包发布；正常安装无需环境变量，`ARKDOWN_TYPESCRIPT_PATH` 只用于
开发覆盖。watch 已按 ets2bundle 生命周期长驻复用 Program，并保留原有诊断、
linter、type-facts 和 source-graph 刷新顺序。源码合同和验证说明见
[`crates/arkdown-ets-checker/DESIGN.md`](../../crates/arkdown-ets-checker/DESIGN.md)。

当前 clean TSGO revision `94e2465a5f92c50eb86a000eb1c4fb95adac3fd0`
已经生成 macOS arm64、Linux x64 GNU 和 Windows x64 MSVC 发行件并通过 archive
hash；macOS 平台包 dry-run 验证 `tsc` 和元数据会实际进入 npm tarball，且 JS CLI
在清除开发覆盖环境变量后能从本地平台包完成 `check`。Linux/Windows 只完成交叉
构建与元数据校验，不把它们记录为目标平台运行验证。

compiler roots 与 ets2bundle 保持同一所有权：Rust 从 loader 的
`entryObj + ohExports` 收集 roots，TSGO 只展开和检查其可达 source graph；不得恢复
全模块目录扫描。TSGO Program path 与 Rolldown snapshot path 是两种合法身份：前者
用于 checker RPC，后者用于 OXC inventory 和 facts consumer。`SourceStore` 在启动时
按照 `ets_checker.ts::getCanonicalFileName`/TSGO `tspath.GetCanonicalFileName` 一次性
索引 symlink alias 和大小写规范路径；不得把它改成逐文件 `realpath`、字符串猜测或
额外 compiler roots。TSGO 的 OH 4.9 generic inference、common-supertype、signature
relation 与 subtype-union allocation-order 语义已经补齐，Rust/OXC 只消费查询事实。

Arkdown 不调用 TSGO emit/transpile/printer/codegen；checker source graph 不替代
Rust resolver/Rolldown graph，返回的 annotation/component 信息也只能作为
Rust/OXC lowering 的 immutable semantic facts。JS 不参与上述代码处理。

当 `shouldEmitJs === false` 时，namespace 等 TypeScript module source 必须像上游
`rollup-plugin-ets-typescript.ts::transform` 一样原样进入 CompileArkTS。Rolldown 只负责
扫描该源码的模块图，其未被产物消费的 JS 投影产生的 `TOLERATED_TRANSFORM` 不得暴露为
构建 warning；其他可对应到上游 Rollup `onRollupWarn` 的诊断仍须保留。

ESMODULE 的 `processImportModule` 不执行 legacy `validateModuleName`，OXC checker
不得为 resolved ESMODULE import 添加内置组件同名错误。UI transform 诊断按
`emitLogInfo` 保留原始 `LogType`；仅独立 ArkTS linter 和 TypeScript checker 使用
各自源码定义的 `oh_modules` 筛选规则，不能把该规则扩大到 OXC transform 诊断。
ObjectLink 严格消费 TSGO resolved type identity，并按
`findNonNullType/checkObjectLinkType/checkTypes` 处理 nullable union、basic、
ObservedV2 和 Function。跨文件 Builder 的 import 解析按
`processImportModule/getDefinedNode/getIntegrationNodeInfo` 处理 named/default/namespace
alias 与 re-export；namespace 调用必须保留限定 callee，SDK namespace component 必须
携带对应 declaration metadata，不得要求业务源码改变合法 export，也不得用 JS 兜底。
`FlowItem` 的 WaterFlow 父节点限制
是上游 `checkNonspecificParents` 明确定义的源码诊断，不得作为 Arkdown 缺口放宽。

### ArkGuard

源码混淆由 `arkdown-obfuscate` 中的 OXC 实现直接处理 source/name/file cache
和 source map；字节码混淆由 Rust 构造计划并调用 SDK 的 PandaGuard 原生工具。
不得恢复 JS ArkGuard bridge、单文件 facade 或 Hvigor task wrapper。

规则解析现已覆盖当前 `ConfigResolver` 的完整 option 集；其中
`-remove-nosideeffects-calls` 按 `DisableConsoleTransformer` 只删除独立调用语句，
保留 `class['property']` 文本并支持名称通配符；`-keep-uncompact` 按
`resolveKeepUncompactConfig/handleUncompactPathsForArkguard` 的原始源路径、目录、
通配符和排除项决定单文件 printer。显式 `-print-namecache` 与上游一样不创建父
目录。JS CLI 双轮 release 实跑已验证默认/显式 cache 一致，新增声明后已有
identifier/property/file 映射零变化，未配置调用保留，选中文件保持多行。
`strip-language-default` 按 `ArkObfuscator.init` 选择 optimized preset；HAR 枚举
保留全部成员，普通工程在任一 enum initializer 含 identifier 时保留该枚举全部成员，
对应 `ApiExtractor.addPropWhiteList/shouldCollectEnum/visitEnumMembers`。SDK、语言、规则、
`-keep-dts`、AtKeep 与 InsightIntent 的 property/global/local 白名单保持独立，不允许
再合并成内部通用兼容集合。IdentifierCache
使用 `ScopeAnalyzer.Scope.loc` 路径并为函数项写入经前序 source map 回溯的起止行，
禁止 OXC `ScopeId` 和模糊后缀 cache 匹配。`-print-kept-names` 直接生成上游
`keptReasons`/`keptNames` schema 与七类 `whitelist.json`，按文件覆盖本轮结果并保留
未重编译文件的历史项；该产物不是规则数组回显。

### Cache 和 watch

当前已有版本化的跨进程 OXC transform/fact cache，以及同一 JS CLI watch
进程内的 Rolldown retained graph 和 TSGO retained Program。TSGO 更新直接消费
watcher 的 modified/created/deleted 事件及新 immutable source snapshot，不经过
JS 或独立 checker 兼容层。Rolldown 目前没有供 arkdown 使用的安全
磁盘 graph deserialize API，因此新进程仍需 resolve/parse/scan。这是明确的
上游能力边界，不得用空 `rollup_cache.json` 或伪图文件冒充命中。

## 4. ArkTS 1.2 的源码执行模型

`ets/static` 是 SDK 组件目录，不是可执行文件。上游普通工程链路为：

1. Node 执行 `ets/static/build-tools/driver/build-system/dist/entry.js`。
2. driver 加载 `ets/static/build-tools/libarkts/lib/libarkts.js`。
3. `Es2pandaNativeModule.ts::findNativeModule` 通过 Node native addon 加载
   `es2panda.node`，由 native binding 驱动 parser/checker/plugin 状态机。
4. driver 只在对应阶段启动 `dependency_analyzer` 和 `ark_link`；启用静态
   混淆时还会启动 `ark_guard`。

SDK 中可能存在独立 `es2panda` 可执行文件，但上游普通构建不直接执行它；它
只出现在 worker 崩溃后的本地诊断路径。因此
`StaticArktsCompiler::validate_sdk` 按源码校验 driver、`libarkts.js`、
`es2panda.node`、UI plugins 和对应阶段实际使用的子进程工具，不把独立
`es2panda` 作为构建硬依赖。

Arkdown 已由 Rust owner 完成 1.2 的版本选择、SDK/session 快照、源码分类、
模块/alias/declaration/syscap 输入、interop declgen、driver 配置与执行、缓存、
混淆、检查、preview、HAP/HSP/HAR/APP 布局和打包。静态 ArkTS 编译按上游
设计交给 SDK driver/libarkts，而不是在 Arkdown 内复制编译器；Arkdown 拥有
其 typed command plan、输入、诊断和产物生命周期。

执行模型覆盖纯静态、纯动态和混合模块。`.ets` 仅在首个有效 directive 为
`'use static'` 时进入静态分区，`.ts`/`.js` 留在动态分区；混合产物显式打包
`modules_static.abc` 和 `modules.abc`，不扫描可能过期的 sibling ABC。Source
HAR、ByteCode HAR、HSP 依赖和多模块输入均进入对应的 declaration、link 与
package 路径。reload/quick-fix 因 static driver 没有 1.1 协议而 typed reject；
widget 由 static UI plugin 链处理，不另造动态 widget 编译通道。

## 5. 剩余阻塞项

### P0：ArkTS 1.2 替换认证的外部门禁

- QEMU 的 `out/arm64_virt/ohos_ets` staging 已产生 static driver、libarkts、
  addon、UI plugins、`ark_link`、stdlib 和 declarations；另行构建
  `dependency_analyzer` 后可组成已验证的临时 SDK。但当前 `out/sdk` 没有以
  `sdk_build_arkts=true` 发布完整组件；正式 SDK 不能依赖仓库 staging 路径。
- 普通、interop、cache、Source 混淆和动态 PandaGuard 路径已用真实工具执行。
  static bytecode 混淆的 command/config/替换路径已有契约测试，但当前 QEMU
  构建关闭 `abckit_enable`，未产生 static `ark_guard`；补齐该 SDK 工具后必须
  再执行真实混合 bytecode-obfuscation fixture。
- 当前 QEMU 自带 static sample 不能通过同 checkout 的编译器源码约束（默认
  import、catch 类型、export annotation/private type）；不得修改样例或给
  Arkdown 添加 fallback。上游提供有效样例后加入 strict 产物对比矩阵。
- 用可发布 SDK 对 HAP/HSP/HAR/APP 完成结构、签名和设备/模拟器安装启动验证，
  才能把 ArkTS 1.2 状态从 build-complete 提升为 replacement-complete。

### P1：1.1 真实矩阵与架构收口

TSGO 的当前源码定义 ArkTS 1.1 能力差异为 0。OH linter 1004/1004 source case、
native checker、build semantic、SDK CommonJS callback 与 OXC 类型消费者均已通过；
下列项目不再是“先补 TSGO”的能力缺口。

- 按 ets2bundle 源码测试继续扩充 UI transform、system API、Visual、
  ByteCode HAR、router、worker、InsightIntent 和 HSP 的边界语料。
- TSGO `updateProgram` 已按 ets2bundle watch 生命周期接入：首轮和增量轮次共享
  模块/目标 session，旧 snapshot 在新 Program 创建后释放；每轮仍重新获取完整
  diagnostics、linter、source graph 和类型事实，不恢复旧 Rust 子集 checker 或
  SDK Node checker 兜底。
- `duplicate_check` 已用 Rolldown 分片 facts 取代全局 `Arc<Mutex>`，并在告警边界
  确定性归并；`har_dedup` 已按 `hsp-collect-har-files.js::buildEnd` 直接读取最终
  module graph，删除 `moduleParsed` 共享 collector 和无调用的 `HarDedupFact` facade。
- AOT partial、HotReload 首轮/增量、ColdReload patch、未签名 HQF 打包以及已物化
  Source HAR 的 APP 已从 JS CLI 调用真实 SDK 工具通过。ColdReload 现在直接消费
  loader `patchConfig` 三个路径字段及 SDK `cachePath` 默认 changed-list，不再保留
  固定内部路径这一错误兼容分支。AOT 的空 `.an/.ai` 与当前 `gen_aot.ts` 源码一致；
  该源码没有执行其构造的 `ark_aot_compiler` 命令，Arkdown 也不得额外执行。
- 真实 SDK PandaGuard 已从 JS CLI 验证 origin/obf/final HAP ABC、source map 与
  nameCache 的完整交接，并修复生成期 `ResourceTable.ts` 不属于启动快照的生命周期
  差异。SDK index-signature 参数白名单和上一阶段 source-map `sourceRoot` 继承也已
  按 ArkGuard 源码补齐。ArkGuard 规则集、name-cache 跨轮复用和
  keptNames/whitelist 已有源码对齐回归，不得继续列为实现缺口。
- project-mode `assembleApp` 已严格验证 2 个 HAP + 2 个 HSP 的组合（包含无 ETS
  源码的 resource-only feature HAP）以及 form-widget 链路，并据源码
  补齐 legacy `pkgSdkInfo.json`、form profile/restool 恢复、widget compile gate、
  ResourceTable module/package/cache owner、interface HAR `packageName`、显式
  `customizedHar: false`、disabled BinXO 空 cache、`.test` ohosTest 源码根和 kit
  same-name import printer。`ArkCompile.taskShouldDo`、`checkPackageMain` 与
  `CoreSourceSetImpl.initCodeMap` 还确认仅当选中 source set 的 `ets` 目录存在，或
  OHPM `main`（缺省为 `Index.ets`）文件存在时执行 CompileArkTS；仅有生成的
  `ResourceTable.ts` 不会触发 checker、bundle 或 ABC，
  `BasePackHapTask.generateCommand` 此时不传 `--ets-path`。Arkdown 已按该源码条件跳过
  编译并生成不含 `ets/` 的 HAP。
- 已签名 HAP/HSP/HQF 与包含 2 HAP + 2 HSP 的已签名 APP 已通过真实 Hvigor 严格
  对比和 `ApplicationVerifier` 验签。ColdReload 的 `compiledFileList`、新文件过滤、
  `isImportNewFile` 和 source-map JSON 写出格式已分别按
  `collect-importers-plugin.js` 与 `module_coldreload_mode.ts` 迁入 Rust。
- HAR 签名暂列明确遗留项：当前 `hapsigner-rs`
  `e8be862ec0998233ad6e90b70c4c4a7608249783` 不支持 Hvigor `PackageSignHar` 使用的
  HAR 目录签名协议，故保留 typed Rust runner 调用 SDK
  `sdk-sign-tool-full.jar sign-har`。不得在 Arkdown 内复刻或猜测该协议；待
  `hapsigner-rs` 原生支持后再迁移，并补真实 signed HAR 严格矩阵和验签。
- 在 Rolldown 提供结构化磁盘图接口后再实现跨进程 graph cache；在此之前不
  自创序列化格式。
- 按源码补充更细粒度的 ABC invalidation 和依赖 wave；所有并发结果按 typed
  ID 排序后提交。
- 统一剩余 SDK stderr、插件异常和阶段错误为携带 stage/module/path/span/code
  的 typed diagnostics。

当前权威 ets2bundle 源码没有定义 Hvigor task/system plugin 的公开 ABI；因此
Arkdown 只承诺并实现源码可证的 arkts-compose compile-plugin/Rollup ABI。没有
源码 owner 的 task registry、hook facade 或 `hvigorfile.ts` 近似执行层不得实现。

## 6. 完成门槛

只有同时满足以下条件，才可声明相应范围能够替换 Hvigor：

- 所有入口均从 npm JS CLI 或 Node API 进入 typed N-API；不存在 Rust CLI。
- 真实 Hvigor fixture 矩阵在 `ARKDOWN_PARITY_STRICT=1` 下零差异且无 allowlist。
- TS/JS 插件契约、独立 Node 进程 cache 恢复和 watch 重建测试通过。
- 源码契约测试、workspace test、Clippy 和 format 全部通过。
- HAP/HSP/HAR/APP/HQF 按变更范围完成结构验证，HAP/HSP/APP/HQF 完成签名验证；
  HAR 原生签名在 `hapsigner-rs` 提供目录协议后单独验收。
- 没有空 adapter、fake cache、双轨实现或只为保留 Hvigor 内部形状而存在的代码。
- ArkTS 1.2 构建实现已标记为 build-complete；只有上述外部门禁全部完成并
  通过真实发布 SDK、产物和设备验证后，才可标记为 replacement-complete。
  在此之前默认 1.1 行为不得受影响。
