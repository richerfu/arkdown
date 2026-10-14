# ArkTS checker 内存：SDK 插件边界与 mimalloc 回收

状态：Round 1 进行中（2026-10-12）。实现方：Codex；方案与验收：Claude。

## 实测基线（meituan-harmony，`build --no-cache`，macOS 16GB）

binding sha256 `9584db27…`（ca7b548 + 未提交 WIP，快照 `refs/arkdown/pre-codex-wip`）。

| 指标 | 基线 |
|---|---|
| peak memory footprint（`/usr/bin/time -l`） | 5.02 GB |
| wall time | 157 s（机器 swap 30/31GB，波动大） |
| checker 期间 V8 heapUsed 峰值 | 1.2 GB |
| SDK 插件 N-API 调用 / JSON 字节 | 464,674 次 / 2.27 GB（syscap 的 `declaration.text` 占 1.95 GB） |
| JS 侧 `createSourceFile` 重解析 | 5,553 文件 / 54.7 MB 源码（其中 303 个 SDK 声明 17.1 MB），AST 保留到 `releaseSession` |
| tsrs arena（vmmap VM_ALLOCATE） | ≈0.9 GB，与 tsrs bench 同量级，不是本轮目标 |
| `--skip-checker` 稳态 / 加 `MIMALLOC_PURGE_DELAY=0` | 2.1 GB / 1.5 GB |
| 诊断输出 | 10,231 条 warning，两次运行逐字一致 |
| `modules.abc` / `sourceMaps.map` | `1939efa4…` / `9c9b90a4…` |

## 根因

1. `tsrs_checker/src/oh_sdk_plugin.rs::node_snapshot` 每次 syscap 调用都 `file.text().to_string()` 复制节点文件和
   声明文件的全文（如 `common.d.ts` 908KB）；`tsrs_api/src/ohsdkplugin.rs::State::intern_source` 再做全文
   `==` 比较，并在 `sources` 中保存每个文件的全文副本。23 万次调用产生大量短命大分配。
2. 请求携带 `text`（`get_text_of_node`，声明节点平均 8.4KB），JS 只用它来 `node.getText(sourceFile) === text`
   做精确匹配和缓存键校验。
3. `packages/arkdown/js/plugin-adapter.js::SdkPluginHost.locateNode` 对每个出现过的文件做
   `createSourceFile(..., setParentNodes=true)`，AST、`nodes`、`children` 缓存全部保留到 session 结束。
4. mimalloc 默认 purge delay 下，约 0.6GB 已释放页面没有还给系统。

## 上游证据（必须保持的 ABI）

- `ets-loader/lib/fast_build/system_api/api_check_utils.js`：`externalApiCheckerMap.get(SYSCAP)` 中的每个
  checker 都以 `c.check(node, declaration, projectConfig)` 调用，参数是真实的 `ts.Node`。
- SDK `ts-checker-hooks/SyscapChecker.js::check` 读取 `apiNode.getSourceFile().fileName` 和 `apiNode.getText()`，
  自带 5000 项缓存；`since-checker.js` 是纯字符串函数。
- 因此 JS 仍必须把真实 TS 节点交给插件；可以改的只是 Rust→JS 的传输形态和 JS 侧 AST 的保留时长。

## Round 1 范围

### A. tsrs fork（`../tsrs`，分支 `perf/sdk-plugin-memory`，基于 `ohos` = fb3668e）

- `OhSdkPluginNodeSnapshot` 不再携带拷贝出的全文：持有能在 intern 时零拷贝取得源码的句柄
  （如源文件的 `Arc<str>` / 文件身份），只有首次注册某个 (file, 版本) 时才把源码序列化给 host。
- intern 按文件身份 + 内容版本判定，不做全文比较；`State` 不再保存全文副本（只保存判等所需的身份/哈希）。
  同一路径内容变化（`update_program` 之后）必须分配新 sourceId 并重新发送 source。
- 节点描述改为 `{fileName, sourceId, pos, end, start}`（UTF-16；`start` 必须是当前
  `tsrs_scanner::get_text_of_node` 取文本时使用的同一个起点，从而 `source.slice(start, end)` 与旧 `text`
  逐字相同）；删除 `text`。保留现有的“先注册再引用 ID”的顺序保证。
- 更新 `tsrs_api/tests/sdk_plugin_test.rs`，并新增断言：同一文件第二次调用不再携带 source；内容变化后重新携带；
  请求中不再出现 `text`。遵守 tsrs `AGENTS.md`（lint ratchet、`tools/lint/source.py`、性能改动写 `notes/`）。

### B. arkdown JS host（`packages/arkdown/js/plugin-adapter.js`）

- `locateNode` 的精确匹配条件由
  `(node.pos===pos || node.getStart(sf)===pos) && node.end===end && node.getText(sf)===text`
  改为 `(node.pos===pos || node.getStart(sf)===pos) && node.end===end && node.getStart(sf)===start`
  （同一源码下与旧条件等价，且不再构造子串）；保留包含关系剪枝和二分查找；匹配不到时仍抛
  `Unable to recreate SDK checker source node`。缓存键改为 `pos:end:start`。
- AST 保留有界：SDK/声明文件（`.d.ts` / `.d.ets`）整个 session 保留；其余文件放入固定容量 LRU
  （容量常量写明理由，默认 64），淘汰时丢弃 `sourceFile/nodes/children`，源码文本仍保留以便重解析。
- 更新 `__test__/plugin-adapter.test.cjs`：新请求形态、LRU 淘汰后能重解析并命中同一节点、声明文件不被淘汰。

### C. arkdown Rust 依赖

- 开发期在根 `Cargo.toml` 用 `[patch."https://github.com/ohos-rs/tsrs.git"]` 指向 `../tsrs/crates/*`；
  不得在 crate manifest 写 path 依赖。是否推送 fork、改回 `rev` 由用户决定，Codex 不推送、不提交。

### D. mimalloc 回收

- 在 binding 的全局分配器初始化处设置 mimalloc purge delay（不得依赖用户环境变量）。取值必须由测量决定：
  在 meituan 上比较 `MIMALLOC_PURGE_DELAY` = 10（默认）/ 1 / 0 的 peak、稳态与 wall time，选择 wall time
  回退 ≤3% 内 footprint 最低的值，并把数据写进本文件。若 `mimalloc-safe` 未暴露 option 接口，说明并改用其
  依赖的 sys crate API；不得引入新的分配器依赖。

## Round 1 实测（2026-10-12，Codex 实现后）

正确性：10,231 条诊断逐字一致，`modules.abc` / `sourceMaps.map` 哈希一致。

| 指标 | 基线 | Round 1 |
|---|---|---|
| peak footprint | 5.02 GB | 4.10 GB |
| V8 heapUsed 峰值 / GC 后保留 | 1.2 GB / — | 582 MB / 265 MB |
| SDK 请求 JSON | 2.27 GB | 285 MB（源码仅 54.7 MB，余为每次调用的封套） |
| wall | 157 s | 133 s |

峰值时刻（facts 刚完成）vmmap：tsrs arena 1.68 GB、mimalloc 1.53 GB、V8 页 0.87 GB（JSON 抖动留下的已提交页）。

调用去重统计：`value` 232,530 次只有 24 种参数；`syscap` 231,963 次有 199,677 种精确 (node, declaration, config)，
按声明只有 5,726 种。JS 执行合计 7 s，调用窗口 76.6 s：成本在跨线程往返（约 165 µs/次）和全局 Mutex 串行。

## 决策：插件结果缓存（用户 2026-10-12 选定）

上游 ets-loader 每次 SDK API 使用都直接调用插件（normal 与 strict 两个 program 各一次）。用户接受以下偏离：
- function 插件（value/format/distribution/regex）按 (path, functionName, operation, args) 缓存结果；
- syscap 按 (node 文件版本+pos+end+start, declaration 文件版本+pos+end+start, projectConfigId) 精确去重。
前提：SDK 插件对相同输入返回相同结果（`since-checker.js` 为纯字符串函数，`SyscapChecker` 自带结果缓存）。
不按“仅声明”去重（依赖 `SyscapChecker` 不读 `appNode` 的内部实现）。错误响应不缓存。

## checker 数量实测（临时测量 binding，已还原）

| checkers/pool | peak | wall | checker 阶段 |
|---|---|---|---|
| 4（默认） | 4.41 GB | 179 s | 92 s |
| 2 | 4.36 GB | 197 s | 133 s |
| 1 | 3.73 GB | 456 s | 392 s |

诊断三次都一致。结论：tsrs 内存以前端（AST/binder）为主，不调 checker 数量。同配置重复测量波动约 0.3 GB。

## Round 1b 范围

1. 撤销 Round 1 对 `runtime-architecture.test.cjs` 的两处放宽：`plugin-adapter.js` 缩减到 44 KiB 以内；lockfile
   pin 检查保持原样（`[patch]` 存在期间它失败是预期的，推送 fork 并改 rev 后恢复）。
2. `start` 只对普通节点成立：missing 节点、virtual 节点（pos/end 交换）、JSDoc 类型表达式（文本规范化）
   仍发送 `text`，JS 在 `text` 存在时用 `getText` 比较。补测试。
3. 按上面的决策在 `SdkPluginExecutor` 实现结果缓存（生命周期 = executor），错误响应不缓存。
4. 插件描述符（path/functionName/className）每 session 只发送一次，之后请求用 id 引用。
5. 源码预处理按需：`SourceSnapshot` 只对 tsrs 实际读取的 `.ets/.ts` 生成 overlay（`OnceLock` 缓存），
   `uiProps`/new-extend 名字、语义清单只来自 Program 读取过的文件（上游 `ets_checker.ts:374`
   `getScriptSnapshot`）。若诊断因此变化，停下报告差异，不得加补丁逻辑。
6. （1–5 验收后）一次性构建开启 tsrs lazy DTS：改为 per-program 选项而非进程全局开关，watch/preview 不开；
   诊断与产物必须逐字一致，并用 `TSRS_ARENA_POISON=1` 跑一遍。

## 不做（附理由）

- 降低 checker 数量：见上表，省内存极少、耗时显著增加。
- 按“仅声明”去重 syscap：依赖 SDK 插件内部实现，换 SDK 版本可能失效。
- 把 syscap 调用批量化：checker 需要同步结果来产生诊断，批量化会改变调用顺序。
- 共享 checker 类型图、复用 normal/strict pool：tsrs notes 已多次测量并否决；双 pool 来自上游 strict 语义。

## 总目标与路线图（2026-10-12 用户要求“做到极致”：内存和耗时）

全阶段时间线（4 checkers，cold `--no-cache`，179 s / peak 4.41 GB）：

| 阶段 | 现在 | 目标 | 主要手段 |
|---|---|---|---|
| 启动到 step 2（N-API 加载 + session 发现 38k 文件） | ~10 s | ≤ 3 s | profile 后定 |
| profiles | 3.6 s | ≤ 1.5 s | profile 后定 |
| checker 诊断 | 92 s | ≤ 10 s | 插件缓存 + syscap 按文件批量（R3） |
| facts + transform snapshot + plugin factory | ~7 s | ≤ 3 s | profile 后定 |
| rolldown bundler | 14 s | ≤ 6 s | profile 后定（scan 3.4 s、写 module sources 5.1 s） |
| ABC（es2abc） | 12 s | 测量后定 | SDK 工具，只调并发/输入 |
| packaging（1.6 GB HAP：pack 22 s + sign 12 s） | 35 s | ≤ 8 s | 流式签名、避免整包读入、压缩并行度 |
| **总计 cold** | **179 s** | **≤ 45 s** | |
| **peak footprint** | **4.41 GB** | **R1b 后 ≤ 3.0 GB，最终 ≤ 2.0 GB** | lazy DTS、按需预处理、去 JSON 抖动、Rust 堆归因 |

另测 warm（缓存命中）构建；正确性门槛对每一轮都不变：诊断逐字一致、`modules.abc`/`sourceMaps.map` 一致、
unsigned HAP 两次构建一致（signed HAP 含签名随机数，不比较）。

验收：meituan `--no-cache` cold 构建，每项取两次中位数（机器 swap 接近满，单次波动 ~0.3 GB）。

### R1b 验收（2026-10-12）

正确性全部通过（诊断、`modules.abc`、`sourceMaps.map`）。peak 3.90 / 4.05 GB，wall 172 / 174 s，checker 85 / 84 s：
收益很小。`sample` 8 秒（checker 阶段中段）：

- 4 个 checker 线程约 34% 时间在 `SdkPluginExecutor::check_syscap` 的全局 `Mutex` 上排队；等 JS 应答只占 ~3%。
- 忙碌 CPU 中 **95.5% 在 OH SDK API 使用检查**，真正的类型检查 < 1%：
  - 61% serde_json：`oh_sdk_plugin_project_config()` 每次调用重建/clone 整个 projectConfig `Map`，
    executor 再在锁内 `config_index` 做整 `Map` 相等比较（另有 madvise/memmove 来自这些大对象的分配释放）。
  - 30% tspath：`check_oh_sdk_declaration_use` 每次使用都对同一 use/declaration 文件做 `normalize_path`、
    `get_base_file_name`、`decl_file.text().contains("@Available")`（扫描整个声明文件）、`is_oh_card_file` 等。
  - function 结果缓存键是 `serde_json::to_string(args)` + SipHash。

### R1c 范围（先于 R2）

1. projectConfig 每个 program 只构建一次：`Arc<serde_json::Map>` 存在 compiler options/checker 上；executor 按
   `Arc` 指针识别 config（首次见到新 `Arc` 才做内容比较并分配 id）。
2. `check_syscap` 缓存命中路径不加全局锁（分片或 `RwLock`）；全局锁只覆盖“注册 source/config + 入队”，
   不跨 JS 往返。保持“先注册再引用 id”的顺序保证（N-API TSFN 队列 FIFO，需在代码中写明依据）。
3. 每个 `SourceFile` 的 OH SDK 分类事实在 checker 生命周期内只算一次（按 SourceFile node id 缓存）：
   system module 判定、`is_oh_project_file`、`is_oh_sdk_declaration_file`、`@Available` 是否出现、card/ohosTest、
   ArkUI 声明目录判定等。语义与现有逐次计算完全一致。
4. function 结果缓存键改为结构化键 + FxHash，不序列化 JSON。
5. 实现后 Codex 必须自己在 meituan 上用 `sample <pid> 8` 抓 checker 阶段，迭代到类型检查本身成为最大头，
   并在报告中给出前后 profile 摘要。可运行 meituan 验收构建（同一时间只跑一个构建）。

### R1c 验收（2026-10-12）

两次 cold：checker **9.4 / 9.3 s**（R1b 85 s），bundling 29 / 31 s（R1b 104–109 s），wall 138 / 145 s；
诊断、`modules.abc`、`sourceMaps.map` 一致。peak 4.32 / 4.33 GB、packaging 90 s（pack 68 s + sign 20 s）
不可信：数据盘只剩 1.9 GB（每次打包写两个 1.6 GB HAP，swap 也在同一卷）。待清理磁盘后重测。

unsigned HAP 每次整包哈希不同：条目时间戳为构建时间（`unzip -v`）。确定性改为比较“条目名 + CRC32 + 大小 + 压缩方式”列表，
不比较整包哈希。

checker 阶段 `sample`：全局锁等待消失；checker 线程 ~2/3 时间在等 JS 应答（`dispatch_batch`）。
JS 主线程 86% 忙。V8 CPU profile：TypeScript `skipTrivia`/`scan`/`scanJsDocToken`（`locate` 中每个访问节点都调
`getStart`）与 `createSourceFile` 重解析约 30%，`locate`/`locateNode`/`childNodes`/`dispatch` 约 13%，GC 8.6%，
SDK `SyscapChecker.check` 本身 1.7%。即：时间花在“为插件重建 TS 节点”和逐次往返，不在插件逻辑。

### R3 范围（syscap 按文件批量 + JS 节点定位）

1. tsrs：每个 checker 在一个文件检查期间收集 syscap 待定请求（缓存命中直接用），文件结束时一次 `invoke_batch`，
   再按原顺序补诊断。诊断最终排序；必须逐字一致。
2. JS host：批内按 sourceId 分组处理以获得 AST 局部性；`locate` 只用 `pos/end` 做包含剪枝，仅在 `end` 吻合的候选上
   调 `getStart`；统计每 session 的 `createSourceFile` 次数，据此确定 LRU 容量（报告数据）。
3. 目标：checker 阶段 ≤ 4 s；JS 主线程在 checker 阶段不再是瓶颈；checker 阶段 V8 已提交内存 ≤ 400 MB。

### 磁盘清理后的重新基线（2026-10-13）

用户批准删除 `arkdown/target/debug`、`tsrs/target/debug`、`meituan/build`（可再生成），可用空间 1.9 GB → 41 GB。
此前 R1c 的 packaging 90 s 是磁盘满造成的。干净磁盘上：

| | wall | checker | bundling | package | peak |
|---|---|---|---|---|---|
| pre-Codex（旧 binary + 旧 JS） | 134.8 s | 78.9 s | 98.7 s | 17.6 s | 5.01 GB |
| R1c run1 / run2 | 68.2 / 67.6 s | 10.1 / 10.7 s | 29.3 / 30.8 s | 19.9 / 17.7 s | 4.32 / 4.38 GB |

旧基线 `sourceMaps.map` 哈希来自残留增量状态；干净重建后 pre-Codex 与 R1c 的 `modules.abc`、`sourceMaps.map` 一致，
基线哈希已更新。R1c 峰值仍在 checker 末尾（vmmap）：tsrs arena ~1.7 GB、mimalloc ~1.5 GB、
V8 已提交 ~1.1 GB（checker 结束后立即回落到 284 MB）。

### R2 归因（Codex 只测不改）
- CPU：`dtrace` / `sample` 采样启动、profiles、bundler、packaging 四段，给出 top 函数。
- 内存：`dhat-heap` feature 归因 Rust 堆；tsrs 前端（AST/binder）与 checker 区分；packaging 是否整包读入内存。
- 产出写进本文件，作为 R3+ 的依据。

### R3 syscap 按文件批量
- syscap 结果只用于产生 warning（`oh_sdk_jsdoc.rs::check_oh_syscap_use`）。每个 checker 在一个文件的检查中
  收集待定请求，文件结束时一次 `invoke_batch`，再按原顺序补诊断；诊断最终排序，结果必须逐字一致。
- 目标：跨线程调用从 ~200k 降到 ~文件数量级。

### R3 验收（2026-10-13）

两次 cold：checker 8.3 / 9.1 s（R1c 10.1 / 10.7 s），peak 4.28 / 4.31 GB（不变），诊断/abc/sourcemap/HAP 条目一致。
wall 87 s 不可比：机器 load 13–16（Spotlight `mds_stores` 86% CPU 索引重建的 target 目录），磁盘回到 97%。
5,250 批、5,556 次解析、3 次重解析：LRU 64 足够。

checker 阶段 JS（V8 CPU profile，0.5 ms 采样）：TypeScript 解析 3.44 s、GC 1.05 s、adapter 1.00 s、TS 其他 0.99 s，
SDK 插件本身 **0.16 s**；JS 仍有空闲 → checker 线程按文件阻塞等待批结果，JS 与 tsrs 计算未重叠。
V8 实测：每 500 ms 强制 GC 时活跃堆峰值 **268 MB**、heapTotal 386 MB；整体 peak 4.30 → **3.84 GB**，checker 时间不变。
即 V8 的 ~0.88 GB 中约 0.5 GB 是可回收垃圾（V8 堆增长策略），不是数据。

### R4a 范围（内存）
1. normal / strict pool 串行释放（见下）。
2. SDK host 的 V8 堆纪律：在批结束时按堆增长阈值（如 heapUsed 超过上次 GC 后的 2 倍且 > 128 MB）调用
   `global.gc()`（`--expose-gc` 已由 CLI 提供，上游 serviceChecker 同样在释放后调用）。确定性的阈值触发，不用定时器。
   目标：checker 阶段 V8 已提交 ≤ 400 MB。

### R4a 验收（2026-10-13）

两次 cold：peak **3.46 / 3.48 GB**（R3 4.28 / 4.31 GB），checker 9.2 / 8.7 s，诊断/abc/sourcemap/HAP 条目一致。
（机器仍有 Spotlight 索引、磁盘 96%，wall 79 / 83 s 不作结论。）

0.3 s 粒度 vmmap 证实 normal pool 已释放：facts 完成后 tsrs 988 → 742 MB、mimalloc 986 → 834 MB。
但 **strict pass 自身更大**：tsrs 742 → 1410 MB（+668）、mimalloc 834 → 1273 MB（+439）。新峰值 =
前端 ~0.74 GB + strict pool ~0.67 GB（arena）+ mimalloc ~1.27 GB + V8 ~0.31–0.5 GB。
SDK host GC 后 V8 在 strict pass 时 311 MB（目标 ≤ 400 MB 达成）；normal pass 末尾仍有 ~500 MB 已提交。

距 2.0 GB 还差 ~1.5 GB，需要归因：strict pool 为何比 normal pool 大；mimalloc 1.27 GB 中 tsrs 堆表 vs SourceStore
vs arkdown 状态各占多少；前端 0.74 GB 中声明文件占多少（决定 lazy DTS 收益）。→ R2。

### R2 归因结果（2026-10-13，Codex 只测不改）

dhat t-gmax 1,151.6 MiB（Rust 堆，系统分配器）出现在 strict linter 结束前；arena census 同点 checker region
685 MiB used、file region 491 MiB used。诊断 10,231 条与基线逐字一致；两仓库与 release binding 已逐字节还原（已复核 binding 哈希）。

**strict pool 大的原因不是类型图，而是格式化符号名的逐次调用状态**：
- ArkTS linter `handle_call_expression`（`Function.apply/call/bind` 规则）每个调用表达式都调
  `get_fully_qualified_name`，递归 `symbol_to_string` → `NodeBuilder::enter_context` 每次新建
  `NodeBuilderContext`（416 B）+ `SymbolTrackerImpl`；未处在 emit scratch region 时落进 checker region 不释放。
  strict pool 保留 877,923 个 context（348 MiB）+ 878,057 个 tracker（20 MiB），外加其 COW 字段的 Rc 堆 161 MiB、
  tracked-symbol Vec 40 MiB：**≈ 569 MiB**。
- 上游 SDK TypeScript（ets-loader `typescript.js:48957`）`getFullyQualifiedName` 在
  `isTypeCheckerForLinter && containingLocation === undefined` 时按 symbol 缓存结果；tsrs 端口缺这层缓存。
- strict 与 normal 相比：类型 +13%、符号 +9%，但 checker region used +155%。

其他持有者：`SourceStore` 文本 213 MiB；tsrs `SnapshotFS::cache_source_file` 再拷一份 80 MiB；syscap 结果缓存 54 MiB；
project_checker 输入 + API 返回值约 78 MiB。

**修正**：本项目声明文件只占 file region used 的 11.6%（57 MiB）；linter 会遍历 715/875 个声明文件，lazy DTS 在
strict 峰值处可省 < 6.3 MiB，降为低优先级。前端主体是 `oh_modules` 实现文件（12,596 个，433 MiB）。

### R5 范围
1. 按上游移植 linter 专用的 `getFullyQualifiedName` 缓存：仅 linter checker、`containing_location` 为 None 时按
   symbol 身份缓存；未命中路径与上游一致（上游缓存外层结果，内层递归不缓存）。报告命中/未命中次数与 strict 峰值变化。
2. arkdown `SourceSnapshot` 与 tsrs `SnapshotFS` 共享同一份不可变文本（去掉 80 MiB 拷贝），保持版本/变更检测。
3. 之后再决定是否需要“逐次 NodeBuilder 上下文进 scratch region”（R2 第 1 名），以 R5 实测剩余量为准。

### R5 验收（2026-10-13）

两次 cold：**peak 2.71 / 2.78 GB**（R4a 3.46 / 3.48 GB），checker 7.6 / 7.4 s，wall 66.6 / 72.7 s；诊断/abc/sourcemap/HAP 条目一致。
限定名缓存命中 303,887 / 未命中 79,036（79.4%）；strict checker arena 685 → 355 MiB（Codex 报告）。
vmmap 峰值（strict 末尾）：tsrs 1065 MB、mimalloc 930 MB、**V8 506 MB**（normal pass 末已到 500 MB 并保持；GC 阈值未把已提交页压下来）。

### 非 checker 阶段 profile（R5 binding，`sample`）

| 阶段 | 耗时 | 主要发现 |
|---|---|---|
| 启动（SourceStore + 依赖清单） | ~8.5 s | `read` 42%、`getattrlist` 18%：每个文件 `canonicalize_or_owned`（`read_source_inventory_files`、`discover_with_preinventoried_roots`） |
| bundler | ~17 s | 每次 transform 调 `TransformEntrySourceId::normalized` → `AbsPath::canonicalize`（realpath），`OhosResolver::resolve_with_kind` 也逐次 canonicalize；违反 hot path 不做文件系统调用的规则；rolldown `load_source` 再从磁盘读源码（`read` 11%） |
| pack | ~11 s | `zlib_rs deflate_quick` 62%（已是 level 1 且并行），`write` 15% |
| sign | ~16 s | **73% 在 `HapZip::read_to_pieces` 把已打包的 454 个 .so 重新 inflate**（`optional_blocks_for_zip`），sha256 26%。打包时数据本来就是解压态，可在打包时产出签名所需摘要/分片，签名不再解压；输出必须逐字节一致 |

### R6 范围（内存）
1. V8 已提交内存：调整 SDK host 的 GC 触发（阈值/major GC），目标 checker 阶段 V8 已提交 ≤ 300 MB，checker 时间不回退。
2. 先测量 R5 后剩余的 NodeBuilder 逐次上下文（数量、arena/堆字节）；若 > 100 MB，按 R2 第 1 名把逐次
   `NodeBuilderContext`/`SymbolTrackerImpl` 放入调用期 scratch region（遵守 `notes/mem-emit-regions.md` 的边界：
   持久 builder、序列化类型缓存、合成结果节点不动）。

### R6 验收（2026-10-13）：两项都不采纳

- NodeBuilder 逐次上下文进 scratch region：R5 后 strict pool 仍有 218k 个上下文 / 141 MiB，实现正确（poison 与诊断通过）。
  但两次 cold peak **2.74 / 2.73 GB**，与 R5（2.71 / 2.78 GB）无差别；vmmap 峰值仍 2560 MB，strict arena 只低 ~50 MB
  （< 2% peak，低于 tsrs 的 5% 门槛），checker 时间在高负载下无法证明不回退。**撤销**，结论写入 tsrs notes。
- V8 GC 策略：8 种策略中，达到 ≤ 300 MB 的都让 checker 慢 23–50%（7.7 s → 9.4–12.8 s）；GC 后活跃堆仍有
  ~280 MB（声明 AST、应用 AST LRU、源码文本），已提交内存的下限由活跃数据决定。**不采纳**，生产代码未改。

当前状态：peak ≈ 2.7 GB，checker ≈ 7.5 s。继续降内存需要减少活跃数据本身（见 R8 候选），不再调 GC。

### R7a 范围（时间：签名）
1. 撤销 R6 的 NodeBuilder 改动（反向应用 R6 补丁），保留 notes 中的测量结论。
2. hapsigner（`../hapsigner-perf`，分支 `perf/parallel-native-sign`，基于已发布 0.2.0 = `ec531c0`）：
   `code_sign.rs::native_sign_infos` 对 454 个 .so 逐个解压 + fs-verity + CMS 签名，单线程（profile：inflate 41%、
   sha256 26%）。改为按条目并行、按原顺序收集；hnp 同理。输出结构不变。开发期用根 `Cargo.toml`
   `[patch.crates-io] hapsigner = { path = "../hapsigner-perf" }`。
3. 验证：unsigned HAP 条目一致；signed HAP 中除 CMS 签名字节外的 code-sign 块（fs-verity 摘要、各 SignInfo 的
   偏移/长度/顺序）与改动前逐字节一致；hapsigner 自带校验（若有）通过。

### R7a 验收（2026-10-13）

两次 cold：**签名 4.7 / 5.1 s**（之前 11–16 s），pack 12.1 / 11.5 s，wall 73 / 75 s（load 9–11），诊断/abc/sourcemap/HAP 条目一致。
hapsigner `verify-app`（其 `CodeSignVerifier` 校验 code-sign 块与各 .so 的 fs-verity）对 meituan signed HAP 输出
`verify signature success`。inflate + fs-verity 并行，CMS 签名保持原顺序串行。打包/签名阶段 footprint 1.43 GB，不影响峰值。
peak 2.85 / 2.85 GB（R5 2.71 / 2.78，R6 2.74 / 2.73）：tsrs 与 R5 代码一致，差异在高负载下的波动范围内（~0.1 GB），不归因于 R7a。

### R7b 范围（时间：bundler / 启动）
1. 签名不再重新解压：pack 阶段把签名所需数据交给签名（`arkdown-pack` → `arkdown-sign`/hapsigner），HAP 逐字节一致。
2. bundler/resolver 热路径去掉 canonicalize：用启动时 SourceStore/依赖快照已有的规范路径身份；rolldown 加载源码复用 SourceStore 文本。
3. 启动：canonicalize 结果按目录复用，不改变符号链接语义。

### R7b 验收（2026-10-13）

两次 cold（load 13–15）：startup 8.1 / 7.6 s，transform snapshot 2.4 / 2.9 s（之前 3.2–4.7 s），rolldown bundler
12.7 / 13.9 s（之前 16.8–19.5 s），peak 2.86 / 2.85 GB，诊断/abc/sourcemap/HAP 条目一致。
`sample`：bundler 中 `getattrlist` 由 ~14% 降到 < 0.1%（剩 21 个样本，均为每次构建一次的 `canonical_root`）；rolldown 不再从磁盘读源码。
启动阶段剩余 `getattrlist` 9%，来自 `arkdown_profile::bytecode_har`（loader json）与 `arkdown_native::collector` 的逐项 canonicalize。
旧路径审计：19,954 条规范路径新旧完全一致。

测试：`module_source_writer_rewrites_unresolved_native_so_static_import` 失败。该测试与其实现均属用户的未提交 WIP
（`module_source_writer{,_tests}.rs` 相对 HEAD 有改动、相对 pre-Codex 快照无改动），Codex 在 R7b 前的状态复现同样失败，与本轮无关。

mimalloc purge delay A/B（交错 2 组，load 12–15）：delay 0 peak 2.94 / 2.79 GB，delay 10 peak 3.11 / 2.92 GB；
wall 被负载淹没无结论。保留 0。

checker 阶段分段（日志时间戳）：normal pass（含 program 创建）9.3–9.9 s，strict pass 1.9–2.3 s。

### R8 范围（时间：syscap 流水线，即 R4b 第 1 项）

用户 2026-10-13 决定：**保持真实 `ts.Node`**（不做 appNode 延迟构造），做流水线。
- checker 线程在文件结束时提交本文件 syscap 批（不阻塞），继续检查后续文件；在该 checker 返回本 pool 的
  逐文件诊断前，等待其所有在途批并按源顺序补 SDK 诊断。`oh_suppresses_sdk_warning` 等判定的输入与时机语义不变。
- 源码/配置/描述符“先注册再引用”在多个在途批下仍须成立（未确认注册的源，后续批要么重带源要么等待确认）。
- 目标：normal pass 9.3–9.9 s → ≈ max(tsrs 计算, JS 工作)；报告 JS 主线程在 normal pass 的忙碌占比。

### R8 验收（2026-10-13）

在清理磁盘后两次 cold：**wall 61.8 / 76.8 s**，normal pass 8.0 / 7.3 s（R7b 9.3–9.9 s），strict pass 1.9 / 1.8 s，
checker 7.9 / 7.5 s，pack 10.0 / 20.0 s，sign 5.1 / 7.6 s，peak 2.86 / 2.79 GB；诊断/abc/sourcemap/HAP 条目一致。
每 checker 最多 2 个在途批；未确认的注册会重发，不依赖 FIFO。

第二次运行时可用空间又降到 4.4 GB（meituan 构建产物约 7 GB/次），pack/sign 变慢；此前一次在 3.0 GB 时 pack 22–34 s。
`bench.sh` 现在在每次运行算完哈希后删除 meituan 构建产物。删除 18 GB 只回收 ~9 GB 可用空间（APFS 本地快照
含 `com.apple.os.update-MSUPrepareUpdate`）：磁盘余量仍是计时噪声的主因。

当前 vs 基线：peak 5.02 → ~2.8 GB；checker 79 → ~7.7 s；签名 11–16 → ~5 s；wall 135 → 62 s（最好的一次）。

### R8 后非 checker 阶段 profile（`sample`）

- bundler 14.7 s（scan 4.6、module graph 1.8、module sources written 4.4、source map 1.3）。忙碌样本中：
  - `Path::components` 迭代 9%：`SourceStore::lookup_path`（经 `SourceInventory::canonical_path`，727）对 root alias 逐个
    `strip_prefix`；`TransformSourceSnapshot::resolve_static_with_declarations`（607 + 425 + 416，来自 const enum
    分析、`imported_binding_is_declared_type`、const enum 内联）；`HvigorModulePathMap::source_file_for_entry`（207）。
  - `madvise` 10%、`memmove` 6%：`HvigorTypescriptPrinter::print_internal` 与 `TypeScriptSourceBoundaryMapper::mappings`
    每个文件分配/释放大缓冲与 `SourceMap`（mimalloc purge delay 0 立即归还页）。
- 启动：`getattrlist` 32%，来自 `arkdown_profile::bytecode_har`（`dependency_version_from_graph`、
  `collect_other_compile_files_from_graph`）与 `arkdown_native::collector::dependency_default_lib_dirs` 逐项 canonicalize。
- ABC：时间在 SDK `es2abc` 子进程（97.6% CPU），arkdown 进程本身几乎空闲。

### R9 范围（时间：bundler 与启动的剩余热点）
1. 路径查找：`SourceStore::lookup_path` 的 root alias 前缀匹配改为不随 alias 数线性增长的结构（先查精确 key）；
   `TransformSourceSnapshot::resolve_static_with_declarations` 对同一 (importer, specifier) 结果按快照缓存或预索引。
   结果与现有完全一致。
2. 打印/源映射：`HvigorTypescriptPrinter` 与 `TypeScriptSourceBoundaryMapper` 预估容量、按线程复用缓冲，减少
   大块分配/释放；输出字节与 source map 逐字节一致。
3. 启动：`arkdown_profile::bytecode_har`、`arkdown_native::collector` 使用依赖快照已有的规范路径，不逐项 realpath。

### R9 验收（2026-10-13）

两次 cold（load 3–11）：**wall 64.4 / 58.2 s**（新最好），startup 5.5 / 6.1 s，profiles 1.8 / 2.1 s，rolldown scan 2.1 / 2.0 s
（R8 4.6 s），bundler 12.1 / 11.8 s，checker 8.0 / 7.7 s，peak 2.83 / 2.82 GB；诊断/abc/sourcemap/HAP 条目一致，
`loader.json` 逐字节一致（82,095 B）。缓存：`TransformSourceSnapshot` 的声明解析缓存属于快照（`Arc` + `RwLock`，
新快照为空）；`PRINT_SCRATCH` 是每线程仅复用容量的 scratch，使用前清空，无跨调用语义状态。

### R9 后 pack/sign profile（`sample`，按线程）

- pack 10.4 s：并行 deflate 写临时文件后，**一个线程**把所有临时文件顺序拷进 HAP（该线程占全部忙碌样本 87%：
  `write` 72%、`read` 13%、close/unlink 12%），其余 12 个线程各 ~9%。
- sign 5.2 s：**一个线程**读 1.6 GB unsigned HAP、写 1.6 GB signed HAP 并边写边算 SHA-256（89%：`write` 44%、
  sha256 27%、`read` 16%）。
- 即 1.6 GB 压缩数据落盘三次（临时文件 → unsigned → signed），对磁盘余量极敏感。

### R10 范围（时间：pack / sign I/O）
1. pack：去掉“全部写临时文件再串行拷贝”。按条目顺序流式写出：worker 并行压缩，写线程按 index 有界重排后
   立即写；小/中条目在内存，超过预算的大条目才落临时文件。内存预算写明（打包阶段峰值不得超过 checker 峰值）。
   unsigned HAP 逐字节不变。
2. sign（hapsigner-perf）：签名会先做 `HapZip::align()`（对齐 `developtools_hapsigner` `Zip.alignment()`，可能改条目
   extra 字段）。**仅当 align 对本包为 no-op**（条目已按 4096/4 对齐）时，条目区与 unsigned 相同，才可避免整包重写：
   同卷 clone/拷贝后在中央目录处截断并写签名块 + CD + EOCD；否则走现有路径。先测量 meituan 的 unsigned 是否已对齐。
   内容摘要按 1 MiB 分块并行计算后按序合并（与现有算法结果一致）。不改 hapsigner 公共 API。
3. 验证：unsigned HAP 两次一致；新旧签名路径产出的 signed HAP 条目区 `[0, CD offset)` 逐字节相同、
   code-sign 块非签名字节相同；`hap-sign verify-app` 通过。

### R10 实测（2026-10-14）：正确性通过，性能分项判断

磁盘 6.6 GB 时：两次 cold peak 2.84 / 2.86 GB，wall 71 / 75 s；pack 9.0 / 9.4 s，sign 9.4 / 16.9 s。
用户批准清理六个其他任务的临时目录后，可用空间达到 29 GB；重新测两次：
- wall **62.3 / 59.0 s**，peak **2.85 / 2.84 GB**；
- pack **6.05 / 6.62 s**（R9 10.4 s，明确改善）；sign **5.63 / 6.43 s**（R9 5.2 s，未证明改善）；
- 两次 `diag=same artifacts=same hap=same`。Codex 的同输入 unsigned/签名结构对比及 `verify-app` 通过。

meituan unsigned 并非已对齐：1260 个 extra 字段变化，总增加 12,016 B；还有排序及 `.pages.info` 添加，因此走
原重写路径，不命中 clone 快速路径。signing 的并行内容摘要需独立 A/B 决定是否保留；不能把整项 R10 算作提速成功。
打包流式化使用 64 MiB 压缩数据预算，超过预算落盘，已闭合排队文件句柄；待进一步审查内存预算与错误恢复。
机器 load 仍在 8–17 之间，整包 wall 只作为观测，不作严格回退结论。

### 已决：appNode 延迟构造（用户 2026-10-13：保持真实节点）

JS 为插件重建真实 `ts.Node`，需用 ets-loader TypeScript 4.9.5（无 `JSDocParsingMode`）重解析 5,250 个业务文件
（~3.4 s + 数百 MB）。当前 SDK `SyscapChecker.check` 只读 `apiNode.getSourceFile().fileName` 与 `apiNode.getText()`，
不读 `appNode`。选项：保持真实节点（现状）/ appNode 用 Proxy 首次访问时才解析（身份与 instanceof 不同）。

### R4b 范围（时间）
1. syscap 流水线：checker 线程提交本文件的批后继续检查下一文件，在本 pool 诊断收集结束前统一等待并按源顺序补诊断；
   JS 持续处理队列，与 tsrs 计算重叠。诊断逐字一致。
2. lazy DTS（per-program 选项，一次性构建开启）。

### R4 tsrs 前端与 checker 生命周期
- lazy DTS 改为 per-program 选项（非进程全局），一次性构建开启，watch/preview 不开；`TSRS_ARENA_POISON=1` 验证。
  本项目以 SDK/oh_modules 声明为主，tsrs 实测此类项目 -5~-16%（`notes/mem-lazy-dts-members.md`）。
- **normal / strict pool 串行释放（最大单项，预计 ~0.8 GB）**。实测（R1c，vmmap 每 3 s + `TSRS_MEM_SPLIT=1` + 两次
  `sample`）：
  - 前端（parse+bind 13,487 文件）arena ≈ 0.87 GB（normal pool 初始化时 `parse end` reservation 872 MiB）。
  - normal pass 期间 tsrs arena 628→1002 MB、mimalloc 697→974 MB；strict pool 初始化时 reservation 1261 MiB。
  - 最后 ~7 s strict linter pass 与 normal pool 共存（`sample` 同时出现两类帧）：tsrs +715 MB、mimalloc +568 MB，
    峰值 4.2 GB = tsrs 1.72 + mimalloc 1.54 + V8 0.88 GB。
  - 做法：先用 normal pool 算完 transform facts（`annotation_transform_infos`、`arkts_transform_type_facts`），
    把 normal 诊断转为 owned（strict pass 只需 `(code,pos,len)` 去重集合与 owned 诊断），再释放 normal pool 的
    checker region 与堆状态，最后跑 strict pass。tsrs 需要“提前退役整个 pool”的 API（可复用 `--maxMemory` 的
    `retire_checker` 机制）。`P<Diagnostic>` 属于 checker region，释放前必须完成转换。诊断必须逐字一致。
- 叶子文件释放（tsrs `fileregions`，CLI 已有）：前提是该文件的 transform facts 在释放前已收集；需单独设计。
- `tsrs_vfs::FS::read_file` 零拷贝。

### R5+ 按 R2 归因排序处理其余阶段。

## Round 1 验收标准（Claude 执行）

1. 正确性：meituan `--no-cache` 构建成功；排序后的 warning 列表与基线逐字一致（10,231 条）；
   `modules.abc`、`sourceMaps.map` 哈希与基线一致。
2. 内存：peak footprint ≤ 3.5 GB；checker 期间 V8 heapUsed ≤ 400 MB；SDK 请求 JSON 总量 ≤ 200 MB。
3. 时间：wall time 不高于基线（同机连续测量，取两次）。
4. 测试：tsrs `cargo test -p tsrs_api`、`-p tsrs_checker` 中 SDK 相关测试、lint ratchet；arkdown
   `cargo test -p arkdown-ets-checker -p arkdown-build`、`node --test packages/arkdown/__test__`、
   `cargo clippy --workspace -- -D warnings`、`cargo fmt --check` 全部通过。
5. 范围：diff 只涉及上述文件与必要测试；无 Hvigor 兼容层、无启发式分支、无占位实现。
