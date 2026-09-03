# OXC/Rolldown 完整替换 hvigor 重构计划

状态：实施中（优先收口 ArkTS 1.1）
更新时间：2026-09-03  
适用范围：Stage 模型、ESMODULE、ArkTS 1.1 默认编译、显式 ArkTS 1.2 编译，以及 HAP/HSP/HAR/APP 的现代构建链路

## 1. 结论

### 1.1 2026-09-03 ArkTS 1.1 第一批实施进度

- 已建立可执行的 `arkts11.json` 源码契约清单，当前 80 个最小行为直接通过公开 OXC transform/check 入口验证，不再以旧反编译代码或旧 spec 为预期；system API 已覆盖 `apiAvailable` 参数和 SDK 身份安全的 `@Available` 内容、OS、父作用域契约，UI checker 新增 Extend/LocalBuilder/Env、SDK `Type` 身份、BuilderParam、ObjectLink、Monitor 根状态、Button label、组件 parent/child grammar 和属性装饰器冲突规则。
- 已删除旧 OXC checker 中约千行没有新源码错误码/函数证据的猜测式规则，改为结构化、多诊断、稳定顺序的 `EtsCheckReport`；`arkdown check --checker oxc` 可在不启动 Node checker 的情况下运行该路径。
- 已对齐首批高频规则：struct/class/function decorator target、build/Styles、成员初始化、Watch、Observed V1/V2、生命周期、全局复用池、SDK 组件/容器命名与根节点约束、`apiAvailable`。
- 已补动态 ArkTS 的 `worker.Worker('*.ts') -> '*.js'` 和 `ESObject -> any` 全程序 OXC 变换，并把 Builder factory/global reuse pool 错误改为源码错误码。
- 已把 `@Observed/@ObservedV2` 类事实并入并行构建的 `ModuleExportIndex`，named/default/namespace import 与 barrel re-export 的继承和 V1 property 类型检查不再退化为单文件名称判断；standalone check 也复用一次性 source snapshot。
- 已从 SDK `components/*.json` 一次性加载 atomic/single/specific-child/style/systemApi grammar，补齐 10905219–10905223、10905228、10905348、10905363、10905365、10905367、10905372，并通过无 cache 的真实 ArkTS 1.1 OXC/Rolldown HAP 构建。
- 默认 checker 仍保持 `sdk`，尚未提前切换。切换和删除 `ets-checker-bridge.cjs` 的剩余阻断项是完整 system API/permission/syscap、其余跨文件类型推导、warning suppressor，以及剩余 UI rule corpus；这些完成前不得宣称 ArkTS 1.1 已完全替换。

arkdown 已经具备可继续演进的 Rust 基座，但还不能称为“由 OXC/Rolldown 完整替换 hvigor”。当前 18 个能力域中：

- 5 个已经形成可用的 Rust 主体：资源/Profile/Loader、解析与打包图、ArkTS 1.1 主干变换、ABC 原生工具适配、打包与签名。
- 8 个只完成了部分行为或所有权仍然错误：配置与任务入口、依赖安装、BuildSession、语义检查、缓存/Watch、插件桥、诊断、对齐测试。
- 5 个仍是阻断项或由 SDK JavaScript 代跑：系统 API 检查、Visual 代码生成、ArkTS 1.2 UI/Interop/Memo 全阶段、ArkGuard、真正的并行 StageExecutor。

这不是完成度百分比；不同能力域的规模相差很大。尤其 ArkTS 1.2 的 4 组插件、62 个 UI 校验规则和 42 个 property-translator 相关文件，工作量显著高于一个普通构建阶段。

当前最明显的架构偏差是：

1. `arkdown-napi` 有约 6.6 万行 Rust，实际承担了构建编排、ABC、打包、运行时任务、hvigor 公共 API 仿真等主体逻辑，不是薄绑定层。
2. `packages/arkdown/js` 有约 4,454 行手写 JavaScript，其中约 4,269 行属于 hvigor runtime/parameter/task/worker/checker 兼容层；目标态只应保留原生入口、CLI trampoline 和 TS/JS 插件适配。
3. 默认 checker 仍是 SDK Node checker，OXC checker 只是显式的快速结构检查模式，与 OXC-first 目标相反。
4. ArkTS 1.2 虽然已经按参数正确分流，但 Rust 只解析并投影模块请求，真正的 UI、语法检查、Memo 和 ABC 驱动仍由 Node 加载 SDK 插件执行。
5. Visual 和源码/字节码混淆仍通过 Node 桥调用 SDK JavaScript。
6. 之前的 `rollup_cache.json` 写入为空内容，且 `RollupCacheStore` 没有生产调用，不能代表 Rolldown 图缓存。

因此，后续工作的重点不是继续扩大 hvigor 兼容外壳，而是把已有 Rust 能力重新收口到一个原生 BuildApplication，并按源码逐阶段补齐编译行为。

## 2. 证据范围

本计划只以用户指定的新源码工程为上游行为依据，不使用旧 hvigor 反编译代码，也不使用旧 specs 推导行为。

主要导航资料：

- `developtools_ace_ets2bundle/docs/kb/06-hvigor调用链分析.md`
- `developtools_ace_ets2bundle/docs/kb/01-整体架构概述.md`
- `developtools_ace_ets2bundle/docs/kb/02-装饰器转换详解.md`
- `developtools_ace_ets2bundle/docs/kb/03-语法检查规则详解.md`
- `developtools_ace_ets2bundle/docs/kb/04-组件编译详解.md`
- `developtools_ace_ets2bundle/docs/kb/05-编译任务导航.md`
- `developtools_ace_ets2bundle/compiler/docs/knowledge/*.md`

关键源码所有者：

- 动态流水线与固定顺序：`compiler/compile_plugin.js`
- ArkTS 1.1 transform：`compiler/src/fast_build/ets_ui/rollup-plugin-ets-typescript.ts` 和 `compiler/src/process_*.ts`
- checker：`compiler/src/fast_build/ets_ui/rollup-plugin-ets-checker.ts`、`compiler/src/ets_checker.ts`
- system API：`compiler/src/fast_build/system_api/`
- ABC：`compiler/src/fast_build/ark_compiler/`
- Visual：`compiler/src/fast_build/visual/`、`compiler/src/process_visual.ts`
- ArkTS 1.2 插件：`arkui-plugins/interop-plugins/`、`ui-plugins/`、`ui-syntax-plugins/`、`memo-plugins/`
- ArkTS 1.2 共享状态：`arkui-plugins/common/plugin-context.ts`、`program-skipper.ts`、`metadata-collector.ts`、`node-cache.ts`
- ArkTS 1.2 源码用例：`arkui-plugins/test/` 和 `compiler/test/`

文档只用于定位；实现和测试预期以相应源码、源码仓库内的输入/期望输出为准。

## 3. “完整替换”的边界

### 3.1 必须替换

- hvigor 的项目构建编排、任务顺序、配置归一化和阶段状态管理。
- ets-loader 中的 ArkTS 1.1/1.2 解析、检查、UI 变换、system API 处理、模块收集、Visual 处理和增量状态。
- Rollup 内建插件在现代 ESMODULE 链路中的行为。
- Node 驱动的 SDK checker、静态 ArkTS 插件、Visual generator 和 ArkGuard bridge。
- hvigor 的缓存失效语义、Watch/Reload 协议、产物布局、错误码和关键中间文件。

### 3.2 可以保留为原生后端

以下是 SDK/native 工具，不是需要保留的 hvigor JavaScript 架构：

- `es2abc`、`es2panda`、`ark_link`、dependency analyzer 和 AOT 工具。
- `restool`。
- `hapsigner` 或等价的 Rust/native 签名后端。
- `panda_guard` 等原生字节码混淆工具。

Rust 必须负责发现工具、构造参数、输入输出清单、缓存键、错误归一化和生命周期；不得再由 SDK JavaScript driver 负责。

### 3.3 不属于缺失能力

- 旧 webpack 构建链。
- JSBUNDLE/FA、其 Terser/Babel 分支和旧 wrapperChunk 行为。
- 为模拟 hvigor 进程而存在的 worker pool、daemon sticky state、GC plugin、waitAsync bridge。

这些能力与现代 Stage/ESMODULE 目标冲突，应在入口处给出稳定的 typed unsupported diagnostic，而不是继续实现兼容层。

### 3.4 ArkTS 版本决策

- 未提供参数时必须始终使用 ArkTS 1.1。
- 只有显式传入 `--arkts-version 1.2` 或等价 N-API 字段时才启用 ArkTS 1.2。
- manifest、依赖、源码扩展名、SDK 能力和插件不得静默把 1.1 构建升级为 1.2。
- 版本选择必须在 `BuildRequest` 创建时冻结，并进入 transform、cache、ABC、watch 和 artifact fingerprint。

当前 `ArktsLanguageVersion::V1_1` 的默认值以及 `BuildArgs --arkts-version` 的显式分流符合该约束，后续不得回退。

## 4. 目标架构

```text
JavaScript entry / CLI trampoline / TS-JS plugin adapter
                            │
                            ▼
                    arkdown-napi (thin)
                            │ BuildRequest + PluginHandles
                            ▼
                  arkdown-build application
        ┌───────────────────┼────────────────────┐
        ▼                   ▼                    ▼
 config + session     StageExecutor          diagnostics
        │                   │
        ├── OHPM/package inventory
        ├── resource/profile/loader plans
        ├── immutable source + SDK snapshots
        │
        └──────────────► Rolldown graph
                              │
                    Rust built-in adapters
                              │
              OXC ArkTS 1.1 or explicit 1.2 passes
                              │
                  module facts + transformed source
                              │
                    arkdown-abc native plans
                              │
                obfuscate → package → sign
```

强制所有权规则：

- JS 不能解析项目配置、组织任务图、决定缓存、构造 SDK 命令或生成构建产物。
- N-API 不能拥有构建业务；它只做值转换、异步调用和插件函数句柄管理。
- BuildSession 是一次构建唯一的文件、包、SDK、依赖和 SourceStore 快照。
- Rolldown hook 不做目录扫描、重复读文件或包根向上查找。
- 每个并行阶段返回 `(TypedId, StageOutput)`，统一排序后提交，不能让并发顺序进入产物。
- 内建插件全部为 Rust；JS/TS 仅用于第三方插件 ABI。

## 5. 当前能力矩阵

| 能力域 | 当前状态 | 证据与实际差异 | 重构结论 |
|---|---|---|---|
| 项目模型/CLI/任务编排 | 部分，所有权错误 | `packages/arkdown/src/cmd/build/coordinator/` 已有编排，但 `runtime.js`、`hvigor-*-runtime.js` 和 Rust `runtime_task` 又模拟 hvigor DAG/公共 API | 抽出 `arkdown-build`；删除 hvigor runtime/task facade；CLI/N-API 共用同一原生入口 |
| 配置和 build script | 部分，兼容层过多 | Rust 已读 build-profile/module 配置，但仍支持编译/执行 `hvigorfile.ts`、`hvigorconfig.ts` 并覆盖模型 | 结构化配置全部 Rust 化；任意 hvigor 脚本不再执行，给出迁移诊断；TS 只用于插件模块 |
| OHPM/依赖获取 | 部分且未接线 | `arkdown-ohpm` 有 registry/tarball/integrity，但生产 BuildSession 不调用；当前主要扫描已安装 `oh_modules` | 在 session 建立前执行 lockfile 驱动的 offline/prefer-offline/online acquire；缓存、完整性、代理都由 Rust 管理 |
| BuildSession/输入快照 | 部分 | 已有 package/sdk/fingerprint/source snapshot，但 profile 又维护一套 `LocalDependencyGraph`，bundle hook 仍重复查包和读 JSON5 | 只保留 session graph/source store/package index；删除重复图和 hook I/O |
| Profile/Loader/Resource | 基本可用但需补金标 | 已有 typed schema、loader、pkgContext、restool、router/startup/worker 等实现；大应用边界仍需源码用例覆盖 | 保留 domain crate；用源码 fixture 补 router、worker、InsightIntent、ByteCode HAR、overlay、跨模块资源回归 |
| Resolver/Rolldown 图 | 基本可用但热路径不合格 | 已有 OHM URL、kit、native/json/commonjs、duplicate/module meta；`ModuleMetaPlugin` 每模块向上找包，JSON5 hook 读盘，若干 collector 用 `Arc<Mutex>` | session 预载全部 metadata/source；plugin 只读 snapshot 并返回事实；确定性归并 |
| ArkTS 1.1 UI transform | 主干可用，未完全覆盖 | 已覆盖 V1/V2、state、Builder/Styles/Extend、WrapBuilder/MutableBuilder、ForEach/Repeat、常见组件和 source map；仍缺 InsightIntent、worker 专用变换、declaration merger、部分 interop/边界 | 直接移植源码测试语料；按 OXC pass 补齐，不以名称或行数判断完成 |
| ArkTS checker | 部分且默认错误 | OXC checker 仅覆盖一部分 decorator/build 规则；默认仍是 SDK Node checker | 默认改为完整 OXC checker；移除 `sdk` 模式和 `ets-checker-bridge.cjs`；`off` 仅保留为显式调试选项 |
| System API/syscap/permission | 大部分缺失 | 当前 `api_transform` 主要做 Kit import rewrite 和少量 `apiAvailable`；没有源码中 version/permission/syscap/device/cross-platform 校验体系 | 建立 `SystemApiInventory` 和 OXC semantic pass，保持 warning suppression、错误位置及 API 版本行为 |
| Visual/SuperVisual | JS 托管 | Rust 只做发现、验证和 OXC 插入，生成片段由 Node 调 `codegen_ets.js` | 把 `.visual` parser/codegen 和 deterministic insertion 全部迁入 Rust/OXC；删除 Node generator |
| ArkTS 1.2 | Rust 行为基本缺失 | OXC 使用 `SourceType::ets_static()` 并生成图投影，但生产编译仍由 Node static driver 加载 UI/syntax/memo 插件；interop 未完整接入，多个 config 字段为空 | 按源码固定阶段在 OXC 实现 interop parsed/checked、UI parsed/checked、UI syntax checked、memo checked+recheck；Rust 直接调用原生后端 |
| ABC/AOT/中间文件 | 基本可用但散落 | dynamic es2abc、filesInfo、sourceMaps、AOT/quickfix 有较多实现，static 仍被 JS driver 包住，代码位于 N-API crate | 抽出 `arkdown-abc`；统一动态/静态/preview/reload command plan 和缓存键 |
| 源码与字节码混淆 | JS 托管 | `arkdown-obfuscate` 把 `arkguard_bridge.cjs` 写入缓存并启动 Node；bytecode 同样走 CJS bridge | 源码混淆使用 OXC minifier/mangler/name cache；字节码走 native panda_guard adapter |
| Cache/Watch/Reload | 部分 | task 粗缓存可跳模块，但 Rolldown 改动后仍重扫；此前空 `rollup_cache.json` 是假缓存；ArkTS 1.2 reload/widget 明确 unsupported | 实现图级 cache manifest、transform fact cache、ABC file cache 和事件失效；补 static reload/widget，或在最终 scope 中稳定拒绝 |
| Package/Sign | 基本可用 | HAP/HSP/HAR/APP、profile、签名已有 Rust 实现，但仍在 N-API coordinator 内编排 | domain crate 保留；编排移至 `arkdown-build`；用产物结构/签名校验做金标 |
| TS/JS 插件接入 | 可运行但外壳过大 | Native build 已能通过 TSFN 调 JS hooks，但依赖 `runtime.js` 私有字段和 hvigor public runtime；生产只扫描 module sources，没有真实 chunk 输出 | 单独做小型 plugin adapter；声明并验证实际支持的 hook；不可达 hook 在注册时拒绝 |
| 诊断/错误码/可观测性 | 部分 | 有 typed error 和 tracing，但 SDK stderr、插件异常、构建阶段错误仍有多套封装 | 建立统一 `BuildDiagnostic`，携带 stage/module/path/span/code/suggestion；N-API 只序列化 |
| 并行与内存 | 架构缺失 | 模块总协调仍有顺序循环；插件 collector 共享锁；重复读取和大字符串克隆存在 | 实现 build waves/StageExecutor、dense IDs、per-worker arena、事实归并和峰值内存基准 |
| 源码对齐测试 | 不足 | ArkTS 1.1 有较多本地测试，ArkTS 1.2 仅少量静态投影/规则测试，不能覆盖上游庞大 test corpus | 建立 source-corpus manifest、golden runner、artifact comparator 和 unsupported matrix |

## 6. ArkTS 1.2 的准确判断

“OXC 已支持 ArkTS 1.2”解决的是语法树入口，不等于已经具备源码行为。

上游静态链路不是单次 parse/codegen，而是固定的多阶段程序变换：

1. `interop-plugin`: parsed、checked，并在 checked 后 recheck subtree。
2. `ui-plugin`: parsed component transform；checked collector、property/struct/builder transform、resource/router/InsightIntent metadata。
3. `ui-syntax-plugin`: checked collector 与 UI 规则检查。
4. `memo-plugin`: checked function/signature/parameter/return/internal transform，然后 recheck。

当前 arkdown 的 `StaticArktsGraphProjection` 只把静态源码的 import/export 请求投影为 Rolldown 可扫描的标准 ESM；它明确不产生可执行的静态 ArkTS 变换结果。`StaticArktsCompiler` 则启动 Node SDK driver，配置里的 `permissions`、`sdkConfigPaths`、`aliasConfig`、`sdkAliasMap`、`interopSdkPaths` 等仍为空或默认值。

因此，ArkTS 1.2 不能标记为“已支持”，准确状态应是：

- 显式版本选择：完成。
- OXC 静态语法解析和依赖投影：完成。
- 静态语义数据模型：部分。
- UI/Interop/Memo/Lint 源码行为：尚未在 Rust 完成。
- 原生静态 ABC 命令编排：尚未从 JS driver 解耦。
- static widget/reload：显式未支持。

实现时不得保留 “OXC parse + SDK JS transform” 双轨作为最终方案。若某个 checked pass 需要 OXC 当前未暴露的类型或 recheck API，应先扩展 OXC fork 的 typed API，再实现 pass；不得回退到 Node/libarkts 插件。

## 7. JS/TS 边界设计

最终允许的手写 JavaScript 只包括：

1. `runtime-entry.js`：加载 `.node`，转发 `build/check/clean`。
2. `plugin-adapter.js`：把已加载的 JS/TS plugin 对象标准化为 native descriptor，并把 hook 调度到 TSFN。
3. `arkdown-cli.js`：必要时作为 npm bin trampoline，立即进入 Rust CLI；如果发布原生二进制，则可删除。

生成的 N-API `index.js` loader 不属于业务层，可继续由 napi-rs 生成。

插件加载约束：

- `.js/.mjs/.cjs` 由最小 loader 加载。
- `.ts/.mts/.cts` 先调用 Rust/OXC 的 `compilePluginModule` 去除类型并生成 source map，再由 Node 加载；删除 npm `typescript` 运行时依赖。
- plugin config 只包含可序列化数据；函数只以 opaque handle 跨 N-API。
- 内建插件不可经过 JS。
- 插件不得获得 hvigor project/module/task facade；只获得稳定的 arkdown context。

第一版承诺的 hook 应以实际构建链为限：

- `buildStart`
- `resolveId`、`resolveDynamicImport`
- `load`
- `transform`
- `moduleParsed`
- `shouldInvalidCache`
- `buildEnd`、`beforeBuildEnd`

如果生产流水线只调用 Rolldown scan 而不生成 chunks，就不能宣称支持 `renderChunk/generateBundle/writeBundle`。要么在注册时用 `ARKDOWN_PLUGIN_HOOK_UNSUPPORTED` 拒绝，要么先把 pipeline 切到真实 Rolldown output 后再开放。所有插件返回值必须在 Rust 侧做 schema 验证和稳定错误转换。

## 8. 可删除和应迁移的实现

### 8.1 已确认并立即删除

- `arkdown-core/src/paths.rs`：与 `arkdown-path::lexical_normalize` 重复，且没有调用者。
- `arkdown-cache/src/rollup_cache.rs`：仅有自身单测，没有生产调用，记录 module IDs 而不保存可恢复的 Rolldown 图或 bundle 结果。
- bundle stage 写空 `rollup_cache.json` 的逻辑。

这些删除不会损失真实构建能力，只移除重复或虚假的兼容表象。

### 8.2 迁移完成后整组删除

| 删除对象 | 前置迁移 |
|---|---|
| `packages/arkdown/js/runtime.js` | native build API 和 plugin adapter 稳定 |
| `hvigor-parameter-runtime.js`、`hvigor-public-runtime.js`、`hvigor-task-runtime.js`、`hvigor-worker*.js` | 删除 hvigor public/task/worker facade |
| `packages/arkdown/src/runtime_task/` | `arkdown-build::StageExecutor` 覆盖所有真实 build roots |
| `hvigor_runtime.rs`、`hvigor_sync.rs`、`hvigor_maintenance.rs`、`hvigor_dependency_workspace.rs` 等 public facade | Rust config/session/OHPM/CLI 入口完成 |
| `build_script_runtime_model/` 及 config/session 中的 hvigor script runtime | 结构化模型归一化完成，插件 loader 独立 |
| `ets-checker-bridge.cjs` 和 `checker=sdk` | OXC semantic/checker corpus 通过 |
| static ArkTS Node driver/config bridge | OXC static passes + native ABC plan 完成 |
| Visual Node runner | Rust Visual parser/codegen 通过源码金标 |
| `arkguard_bridge.cjs`、`bytecode_arkguard_bridge.cjs` | OXC source obfuscation + native bytecode adapter 完成 |
| Yuku module analyzer、Zig native analyzer、`--module-analyzer yuku` | OXC analyzer 金标覆盖完成；OXC 成为唯一实现 |
| `arkdown-profile::LocalDependencyGraph` | profile 直接消费 `arkdown-session::DependencyGraphSnapshot` |

删除必须包括 workspace dependency、N-API export、`.d.ts`、测试、文档和环境变量，不保留无调用 wrapper。

### 8.3 不应机械删除

- `shouldInvalidCache`：它是第三方 compile plugin 兼容行为，应保留为薄 ABI，但缓存状态归 Rust。
- native SDK 工具调用：要从 JS driver 迁到 Rust command plan，而不是重写 es2abc/restool 本身。
- Node 执行用户单测或显式 JS plugin：这是用户代码运行环境，不等同于用 JS 实现构建业务；必须与 compiler/build Node 依赖分开审计。

## 9. 最终 crate 职责

### `arkdown-core`

只保留 dense IDs、通用诊断、artifact-neutral value types。继续把 build config、manifest domain、resolution records 迁出。

### `arkdown-path`

唯一的 typed path/specifier/serialization owner。逐步淘汰 `canonicalize_or_owned` 式隐式 I/O fallback；输出路径做词法规范化，输入实存路径在 session 创建时 canonicalize 一次。

### `arkdown-ohos-schema`

只定义原始 OHOS JSON/JSON5 schema，不做项目发现和文件读取。

### `arkdown-config`

负责发现并归一化 build-profile、module、target、product、signing、SDK 配置，输出 immutable `EffectiveBuildConfig`。不执行 hvigor JavaScript。

### `arkdown-ohpm`

负责 lockfile、registry、offline store、tarball、integrity 和安装计划；被 session startup 正式调用。

### `arkdown-session`

负责一次启动唯一的 `BuildSession`：package graph、source store、SDK inventory、resource/profile inputs、symlink identity、fingerprints。其他 crate 只读 snapshot，不重复扫盘。

### `arkdown-resolver`

只消费 session package index 和 typed specifier；无文件读取、无目录扫描、无全局可变状态。

### `arkdown-ets-transform`

按 `dynamic/`、`static_/`、`checker/`、`system_api/`、`visual/`、`interop/`、`declaration/` 组织 OXC passes。一个文件一次 parse/semantic，多个 pass 共享 arena/scoping/facts，最后一次 codegen/source map。

### `arkdown`

只保留 Rolldown bundler 和 built-in adapter。plugin factory 接收 immutable `BundlePlan`；hooks 返回模块事实，构建结束确定性归并。禁止 hook 内重复读文件和 `Arc<Mutex<Vec<_>>>` collector。

### 新增 `arkdown-abc`

负责 filesInfo、npmEntries、sourceMaps、动态 es2abc、静态 es2panda/ark_link、AOT、quickfix、bytecode obfuscation 的 typed command plan 和结果解析。

### domain crates

`arkdown-resource`、`arkdown-profile`、`arkdown-cache`、`arkdown-obfuscate`、`arkdown-pack`、`arkdown-sign`、`arkdown-watcher` 各自拥有领域行为，不反向依赖 CLI/N-API。

### 新增 `arkdown-build`

唯一 application service，拥有 `BuildApplication`、`BuildCoordinator`、`StageExecutor`、build waves 和产物提交。它组合 domain crates，但不暴露 N-API 类型。

### 新增 `arkdown-cli`

只负责 clap、日志和退出码，调用 `arkdown-build`。`arkdown-napi` 同样调用 `arkdown-build`，两者之间不能互相依赖。

### `arkdown-napi`

只做 JS/Rust 值转换、async runtime、plugin handles 和 diagnostics serialization。目标是删除 `cmd/build/coordinator`、ABC、package/sign、runtime task 等业务模块后再设 LOC gate，而不是在现有 6.6 万行上继续加功能。

## 10. 分阶段实施计划

各阶段都必须完整交付声明范围，不能以 no-op、空 cache、TODO 或“先能编译”结束。

### Phase 0：源码行为清单与 golden harness

工作：

- 建立 `tests/source-parity/manifest.json`，每项记录上游 source owner、fixture、ArkTS 版本、阶段、预期 diagnostic/transform/artifact。
- 从新源码仓库的 `compiler/test`、`arkui-plugins/test` 选择最小输入和期望输出，覆盖正例、负例、增量和多模块场景。
- comparator 分层比较：diagnostics、transformed source/AST facts、module graph、loader/filesInfo/sourceMaps、ABC entry names、package entries。
- 对时间戳、绝对路径、签名随机量先结构化归一化，再比较；禁止直接忽略整个文件。
- 建立 unsupported matrix，明确 legacy JSBUNDLE/FA 等拒绝项。

退出条件：每个矩阵能力域至少有一个失败用例能暴露现有差异；后续每个 parity commit 必须让对应条目转绿。

### Phase 1：抽出原生 application，收薄 JS/N-API

工作：

- 新建 `arkdown-build`，把 `packages/arkdown/src/cmd/build/coordinator`、prebuild、layout、intermediates、package/sign orchestration 按 owner 迁出。
- 新建 `arkdown-cli`；npm CLI 只做 trampoline。
- 定义 `BuildRequest`、`BuildResult`、`PluginDescriptor`、`PluginHookHandle` 和 `BuildDiagnostic`，禁止 application crate 依赖 napi。
- 用 OXC 提供 TS plugin module transpile；建立最小 `plugin-adapter.js`。
- 删除 hvigor public/task/worker/config runtime 以及相关 N-API export、`.d.ts` 和 npm `typescript` 依赖。
- 对 hvigorfile 自定义 task/config 给出迁移诊断，不能静默忽略。

退出条件：CLI 和 N-API 对同一 request 产生相同 build plan；业务 JS 仅剩三类允许文件；内建构建在没有 `typescript` npm 包时可运行。

### Phase 2：统一配置、依赖和 BuildSession

工作：

- `arkdown-config` 生成单一 `EffectiveBuildConfig`，在这里冻结 ArkTS 版本。
- 把 `arkdown-ohpm` 接入 startup：按 lockfile 创建 acquire plan，区分 offline/prefer-offline/online，验证 integrity 后一次性生成 package inventory。
- BuildSession 一次收集 source、manifest、package、symlink、SDK、resources、router、workers、native libs、mock、ByteCode HAR 信息。
- profile 直接使用 session dependency graph，删除 `LocalDependencyGraph`。
- resolver/bundler/transform 不允许自行向上找 package root 或递归扫描 `oh_modules`。
- 把 `arkdown-core` 中 config/manifest/resolution migration targets 移给所属 crate。

退出条件：startup 后的构建阶段不再读取 project manifests/lockfiles；同一路径不会因不同 crate 的 normalize/canonicalize 得到不同 identity；OHPM 有真实生产 caller。

### Phase 3：Rolldown 内建插件和插件 ABI 收口

工作：

- `BundlePlan` 预载 JSON/JSON5 source、package metadata、module path map 和 native declaration map。
- `ModuleMetaPlugin`、`DuplicateCheckPlugin`、HAR dedup 等改为 per-module facts + sorted reducer，移除共享 `Arc<Mutex>` collector。
- 内建插件顺序按源码行为显式建模，不用 JS wrapper 模拟。
- 把 `shouldInvalidCache` 状态放到 `arkdown-cache`；JS plugin 只返回决策。
- 明确 scan-only 与 generate-output 模式；未执行的 chunk hooks 注册即报错。
- 删除迁移完成后的独立 plugin crate/wrapper，不保留兼容 re-export。

退出条件：plugin hook 热路径零目录扫描、零 manifest 重读；单线程与多线程输出字节一致；第三方 JS/TS fixture 覆盖 hook 顺序、异常和 invalidation。

### Phase 4：ArkTS 1.1 checker、system API 和剩余 transform

工作：

- 扩展 OXC semantic facts，按源码补齐 62 个 UI rule files 所代表的规则族；共享 rule context，不重复 traverse。
- 补 decorator target/type/modifier、V1/V2 混用、constructor、build mutation、children/nesting、reuse、Observed/Track、Env/Monitor/SyncMonitor、WrapBuilder 等检查。
- 建立 system API index，补 since/available、permission、syscap、device、cross-platform、warning suppressor 和 `apiAvailable` 控制流。
- 扩展 WrapBuilder/MutableBuilder 源码 corpus，并补 InsightIntent parser/metadata、worker transform、declaration merger、dynamic/static interop、mock/sendable/lazy import 等剩余缺口。
- checker 默认切到 OXC，删除 SDK Node checker。
- transform 使用一次 parse/semantic，多 pass 共享 scoping，最后统一 codegen/source map。

退出条件：选定的 ArkTS 1.1 transform/check/system-api 源码 corpus 全绿；默认构建不启动 checker Node；错误 code/span/severity 与 source expectation 对齐。

### Phase 5：ArkTS 1.2 全 Rust/OXC 链路

工作：

- 建立 `StaticArktsPipeline`，固定执行 interop parsed → UI parsed → type/semantic checkpoint → interop checked → UI checked → UI syntax checked → memo checked → recheck。
- 迁移 ProgramSkipper、NodeCache、metadata/resource/router/InsightIntent collector，改为 session-owned immutable input 与 per-file facts，避免全局 singleton。
- 迁移 struct/component、builder lambda、entry、property/type translators；按 translator/rule 建 corpus，不做笼统“static transform”。
- 迁移 memo function/signature/parameter/return/internal transform 和稳定 positional IDs。
- 完整填充 permissions、SDK aliases、interop SDK paths、dependent modules、resource paths 等 static config，不保留空占位。
- `arkdown-abc` 直接调用 es2panda、dependency analyzer 和 ark_link；删除 Node SDK driver/plugin files 的运行时依赖。
- 实现或明确纳入 static widget、preview、HAR/HSP、reload 的独立 plans。

退出条件：未指定参数仍走 1.1；指定 `1.2` 时构建进程不加载 SDK JS plugin/driver；static source corpus、ABC entry/module records 和 package artifact 对齐。

### Phase 6：Visual 和 Obfuscation 原生化

工作：

- 将 `.visual` 解析、模板展开、变量/函数/build/aboutToAppear 生成改为 Rust typed IR + OXC AST/codegen。
- 保留现有 insertion validation，但不再接受 SDK 返回的字符串 fragment 作为最终架构。
- 源码混淆用 OXC minifier/mangler，补 keep rules、reserved names、property/name cache、HAR consumer merge 和 source map。
- 字节码混淆走 `arkdown-abc` native panda_guard plan，统一错误和缓存。
- 删除 CJS bridges 和 runtime 写桥文件行为。

退出条件：Visual/obfuscation 构建不启动 Node；debug/release、HAR consumer、name-cache 增量 fixtures 全绿。

### Phase 7：真实增量缓存、Watch 和 Reload

工作：

- `arkdown-cache` 建立版本化 cache manifest：compile env、ArkTS version、plugin identity、source/dependency/resource/config group fingerprints。
- 保存可恢复的 Rolldown graph/module facts 或使用 Rolldown 官方 cache API；不能再用 module ID 列表冒充图缓存。
- OXC transform cache 保存 source hash、dependency facts、diagnostics、generated code/map，并与 allocator 生命周期解耦。
- ABC cache 精确到 filesInfo entry 和 native tool identity；Watch 事件映射到反向依赖闭包。
- 统一 hot/cold reload、quickfix、preview 状态机；为 1.2 定义真实 protocol，而不是复用 1.1 字符串路径。
- commit 使用 temp + fsync/rename 或平台等价原子方案；失败不污染上次成功缓存。

退出条件：修改单文件只重做受影响闭包；cold build、warm no-op、leaf edit、shared dep edit、resource edit、plugin invalidation 有可重复测试；崩溃后缓存可恢复。

### Phase 8：StageExecutor、并行和内存收口

工作：

- 从依赖图生成 module/build waves；同一 wave 并行执行 resource、transform、ABC 等允许并行的工作。
- 所有 worker 只返回 `(ModuleId, Output)`；主线程按 dense ID 排序提交 loader、filesInfo、source maps 和 artifacts。
- 移除 `Arc<Mutex<Vec<_>>>`、重复 source strings 和重复 manifests；共享大数据用 `Arc<[T]>`/interned IDs。
- 建立 startup inventory、stage wall time、peak RSS、cache hit/miss/reason 指标。
- package/sign 保持最终顺序提交，避免并发污染 archive order。

退出条件：`RAYON_NUM_THREADS=1` 与 `8` 产物字节一致；大 fixture 峰值内存和时间有固定基线；无 stage 在 coordinator 内做隐式重复扫描。

### Phase 9：最终兼容层删除与发布门禁

工作：

- 执行第 8 节删除表，移除所有零调用 wrapper、旧 N-API export、CLI flag、环境变量、测试和文档。
- 删除 Yuku、SDK checker、static JS driver、Visual JS、ArkGuard JS、hvigor task/runtime/config script 解释器。
- 清理所有 crate 内 direct path/version/git dependency，统一 root workspace dependencies。
- 更新 README/API 文档，只公布真实支持能力和 typed rejections。

退出条件：除用户 JS/TS plugin 和用户 JS tests 外，构建路径没有 Node child process；JS 层不含项目模型/任务/缓存/命令构造；workspace 全量门禁通过。

## 11. 验收门禁

### 行为

- ArkTS 1.1 默认 fixture 构建成功。
- ArkTS 1.2 只有显式参数才启用，且完整 static corpus 通过。
- HAP/HSP/HAR/APP、debug/release、preview/watch/reload、ByteCode HAR、native libs、router/worker/InsightIntent 有覆盖。
- transformed source、diagnostic、loader/filesInfo/sourceMaps、ABC records、archive entries 分层对齐。
- 所有不支持场景都有稳定 code、说明和测试，不允许 silent fallback。

### 架构

- `arkdown-napi` 不拥有 build stage/domain implementation。
- 手写 JS 仅包含入口、trampoline、plugin adapter。
- 内建插件全 Rust；manifest/lock/source 不在 hook 内读取。
- BuildSession 是唯一 startup inventory。
- 没有只在自身单测中出现的 production compatibility API。

### 性能与确定性

- 单线程/多线程产物一致。
- 同输入重复构建 cache key、模块顺序、ZIP entry 顺序一致。
- no-op rebuild 不重复 parse/resolve/ABC。
- 插件事实采用 deterministic reducer，不依赖 lock acquisition order。

### 常规命令

```sh
cargo fmt --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
git diff --check
```

建议增加 CI 静态门禁：

```sh
# 构建业务不得再调用 Node SDK bridge；用户 test/plugin runner 单独 allowlist。
rg 'ets-checker-bridge|arkguard_bridge|codegen_ets|StaticArkts.*node' crates packages

# 最终删除项不得回归。
rg 'runtime_task|hvigor_runtime|build_script_runtime_model|Yuku|ARKDOWN_YUKU' crates packages

# 空或伪 bundle cache 不得回归。
rg 'rollup_cache.json|commit_rollup_cache|restore_rollup_cache' crates packages
```

## 12. 执行优先级

正确的先后关系是：

1. 先建立源码 corpus 和 comparator，避免继续堆“看起来像”的 parity。
2. 紧接着抽 `arkdown-build` 并收薄 JS/N-API，因为后续所有能力都不应继续落在错误层。
3. 统一 session/graph 后再改 plugins/cache，避免把重复 I/O 固化到新接口。
4. 先完成默认 ArkTS 1.1 的 checker/system API/残余 transform，再完成显式 1.2 全链路。
5. Visual/obfuscation/真实 cache 和并行化在业务行为稳定后落地，但其 JS bridge 必须在最终发布前删除。

不能把“删除所有 JS”放在第一步：第三方插件仍需执行环境，且 static/checker/Visual/ArkGuard 的行为必须先由 Rust 接管。也不能把删除放到最后才开始：已确认零调用、重复或伪实现的代码应立即移除，避免后续继续依赖。
