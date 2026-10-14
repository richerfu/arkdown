# ArkTS checker 内存：SDK 插件边界与 mimalloc 回收

状态：R11 已收尾（2026-10-14），example cold 峰值 5.02 → ~2.36 GB；结论见“R11 收尾”，后续方案见“R11 P2-5”。实现方：Codex；方案与验收：Claude。

## 实测基线（example-harmony，`build --no-cache`，macOS 16GB）

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
  在 example 上比较 `MIMALLOC_PURGE_DELAY` = 10（默认）/ 1 / 0 的 peak、稳态与 wall time，选择 wall time
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

验收：example `--no-cache` cold 构建，每项取两次中位数（机器 swap 接近满，单次波动 ~0.3 GB）。

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
5. 实现后 Codex 必须自己在 example 上用 `sample <pid> 8` 抓 checker 阶段，迭代到类型检查本身成为最大头，
   并在报告中给出前后 profile 摘要。可运行 example 验收构建（同一时间只跑一个构建）。

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

用户批准删除 `arkdown/target/debug`、`tsrs/target/debug`、`example/build`（可再生成），可用空间 1.9 GB → 41 GB。
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
hapsigner `verify-app`（其 `CodeSignVerifier` 校验 code-sign 块与各 .so 的 fs-verity）对 example signed HAP 输出
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

第二次运行时可用空间又降到 4.4 GB（example 构建产物约 7 GB/次），pack/sign 变慢；此前一次在 3.0 GB 时 pack 22–34 s。
`bench.sh` 现在在每次运行算完哈希后删除 example 构建产物。删除 18 GB 只回收 ~9 GB 可用空间（APFS 本地快照
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
   同卷 clone/拷贝后在中央目录处截断并写签名块 + CD + EOCD；否则走现有路径。先测量 example 的 unsigned 是否已对齐。
   内容摘要按 1 MiB 分块并行计算后按序合并（与现有算法结果一致）。不改 hapsigner 公共 API。
3. 验证：unsigned HAP 两次一致；新旧签名路径产出的 signed HAP 条目区 `[0, CD offset)` 逐字节相同、
   code-sign 块非签名字节相同；`hap-sign verify-app` 通过。

### R10 实测（2026-10-14）：正确性通过，性能分项判断

磁盘 6.6 GB 时：两次 cold peak 2.84 / 2.86 GB，wall 71 / 75 s；pack 9.0 / 9.4 s，sign 9.4 / 16.9 s。
用户批准清理六个其他任务的临时目录后，可用空间达到 29 GB；重新测两次：
- wall **62.3 / 59.0 s**，peak **2.85 / 2.84 GB**；
- pack **6.05 / 6.62 s**（R9 10.4 s，明确改善）；sign **5.63 / 6.43 s**（R9 5.2 s，未证明改善）；
- 两次 `diag=same artifacts=same hap=same`。Codex 的同输入 unsigned/签名结构对比及 `verify-app` 通过。

example unsigned 并非已对齐：1260 个 extra 字段变化，总增加 12,016 B；还有排序及 `.pages.info` 添加，因此走
原重写路径，不命中 clone 快速路径。signing 的并行内容摘要需独立 A/B 决定是否保留；不能把整项 R10 算作提速成功。
打包流式化使用 64 MiB 压缩数据预算，超过预算落盘，已闭合排队文件句柄；待进一步审查内存预算与错误恢复。
机器 load 仍在 8–17 之间，整包 wall 只作为观测，不作严格回退结论。

### R11 归因（2026-10-14，基线 d41da9f，只测不改）

时间已达标（R10 wall ~54–62 s），R11 只处理内存。基线 cold：peak 2.85 GB，但 checker 结束后进程仍长期保持
~2.1–2.5 GB 直到签名结束。三种临时 probe（均未提交，补丁存于 `/tmp/arkmem/heap/probe/`）：

1. **footprint vs Rust live 时间线**（计数 GlobalAlloc 包 mimalloc，每 100 ms 采样）：

   | 阶段 | footprint MiB | Rust live MiB |
   |---|---:|---:|
   | checker（bundling-step → diag-done） | 2493–2717 | 716–854 |
   | sdk-released | 1192–1247 | 286–364 |
   | plugin factory 完成 | 1964 | 峰值 1216 |
   | scan / graph | 2095–2130 | 667–719 |
   | bundler-done | 2118 | 735 → 420 |
   | ABC（强制回收前） | 1758–1979 | 406 |
   | pack / sign（ABC 开始时强制回收后） | 838–917 | 406 |

   ABC 开始时对所有线程 `mi_collect(true)`，1 s 内 footprint 1979 → 859 MiB，live 不变。**~1.1 GB 是 mimalloc
   保留而非泄漏**：mimalloc v2 的跨线程 free 进入 owner page 的 `xthread_free`，只有 owner 线程走分配慢路径或
   `mi_collect` 时才回收；空闲的 tokio worker / rayon 线程从不回收（`mi_option_generic_collect` 对空闲线程无效）。
2. **采样堆 profiler**（每 256 KiB 一次 backtrace，按调用点统计 live/peak）。Rust 堆归因（MiB）：

   | 时刻 | 合计 | 主要持有者 |
   |---|---:|---|
   | checker 峰值（live 897 MB） | 835 | SourceStore 文本 232；tsrs 符号表 124；NodeBuilder/linter 98；tsrs vfs `SyncMap<bool>`/`to_path` 72；project_checker 70（`Utf16OffsetMap` 20）；tsrs 其他 125；SDK syscap 结果缓存 55；position map 23 |
   | bundler 峰值（live 1321 MB） | 1214 | **ETS transform source map 解析 240**（`OwnedSourceMap::from_json_string` 195 + rolldown `set_source_contents` 45）；SourceStore 232；oxc AST arena（scan 在途）135；rolldown module 源码 `ArcStr` 83；**rolldown 每模块 `FxDashSet` 82**；transform collector 56；ets-transform 打印/映射 ~100；`ModuleExportIndex` 20 |
   | ABC 开始（live 382 MB） | 405 | **SourceStore 232**；ets-transform 打印输出与 mappings 56；printer scratch oxc arena 19.5 |

   - `EtsTransformPlugin::transform` 把每个模块的 transform map 解析成 `OwnedSourceMap` 交给 rolldown，rolldown
     再填 `sourcesContent`（原始源码副本）存进 `module.sourcemap_chain`。arkdown 的 `BundlerOptions.sourcemap` 是
     `None`，map 由 `HvigorModuleTransformCollector` 以原始 JSON 写出；rolldown 只在 render（sourcemap 开启）与
     HMR 读该链，arkdown 两者都不用。增量路径（`scan_modules` + 缓存）下该链还随 module table 驻留整个 watch 会话。
   - `EtsTransformPlugin::load` 为快照模块调用 `LoadPluginContext::add_watch_file(id)`，它除写全局
     `watch_files` 外还调用 `PluginDriver::add_transform_dependency`，为**每个模块**建一个默认分片的
     `FxDashSet`（64 个 cache-padded shard ≈ 8 KiB）只放 1 个元素。rolldown 自身磁盘加载
     （`module_task.rs` `watch_files.insert`）只写全局集合；`transform_dependencies` 仅 `hmr_stage` 读取。
   - SourceStore 文本在启动时一次性读入（`discover_with_preinventoried_roots` closure #12，204 MiB），
     驻留到签名结束；bundle 之后仍有 obfuscation / static_arkts 等读取者，需逐一确认最后读取点。
3. dhat 全量 backtrace 太慢（14 分钟仍在 SDK inventory），弃用。

vmmap（checker 峰值）：tsrs arena（VM_ALLOCATE）1079 MiB、mimalloc（IOAccelerator）1028 MiB、V8（tag 255）
500 MiB。SDK 释放后 V8 heapUsed 28 MB，但 tag 255 仍 294 MiB 直到结束。

### R11 范围（内存）

**阶段 1（只改 arkdown，按收益排序）**

- **A. 阶段边界堆回收**：类型化回收器，与 `sdk_plugin_host` 同样经 `BuildRequest` 注入；binding 拥有
  mimalloc，负责实现。coordinator 在以下边界调用：checker/SDK 释放后；每个 bundle stage 结束后；ABC 前；
  pack 前。
  - 覆盖三类线程：调用线程 `mi_collect(true)`；rayon 全局池用 `broadcast`；tokio worker 用 park/unpark 回调 +
    回收 epoch（全局原子 + 线程本地已见值，每 epoch 只回收一次），再投递 worker 数量的空任务唤醒。
  - **禁止 barrier**（probe 中 barrier 因一个 worker 同步执行 build future 而死锁 27 s）。
  - 非 mimalloc、debug、`dhat-heap` 构建为 no-op。每次回收以 tracing 记录耗时与 footprint 前后值。
- **B. bundler 无用保留**：
  - 当 rolldown sourcemap 关闭时，ETS transform 不再解析/移交 `OwnedSourceMap`。原始 JSON 继续经
    collector 写出，输出逐字节不变。
  - 快照加载只登记全局 `watch_files`，与 rolldown 磁盘加载一致，不再建每模块 transform 依赖集合。
  - 找出 ABC 时仍驻留的 transform 打印输出/mappings（56 MiB）的持有者，在一次性构建中于最后读取后释放。
  - printer scratch arena 若为线程本地池，在阶段边界收缩。
- **C. SourceStore 文本生命周期**：先测量“读入字节 vs 实际被任何消费者读取的字节”。一次性构建在最后
  读取者之后释放文本（保留身份/指纹）；watch/增量保持现状。若改为延迟读取，必须用指纹校验保证快照一致性，
  校验失败给出类型化错误。
- **D. project_checker `Utf16OffsetMap`**：只为需要转换的文件构建，转换后即释放。

**阶段 2（tsrs，开发期 `[patch]`，是否推送由用户决定）**：
- syscap 结果缓存只存解释所需字段或共享 `Arc`（55 MiB）；
- vfs `SyncMap<bool>` 存在性缓存与 `to_path` 的键表示（72 MiB）；
- `EntryVec::set_capacity` 的容量策略（len/capacity 实测后再定）。

**不做**：rolldown scan 在途 AST 限流（会影响并行度）；V8 GC 调参（R2 已否决）；tsrs arena 前端结构。

### R11 验收标准

1. 正确性：同 Round 1 第 1 条。另外 HAP 条目列表一致；watch 测试与增量 bundler 测试通过。
2. 内存（两次 cold 取中位，`memtl.py` 时间线）：
   - 阶段 1：
     - ABC/pack/sign 阶段 footprint ≤ 0.9 GB（基线 ~2.05–2.17）；
     - bundler 阶段最大 footprint ≤ 1.9 GB（基线 2.3–2.55）；
     - bundler 峰值 Rust live ≤ 1.0 GB（基线 1.32）；
     - 整体 peak 不高于基线（2.85）。
   - 阶段 2：整体 peak ≤ 2.75 GB。
3. 时间：wall 中位数不超过基线 +3%；单次回收 ≤ 100 ms。
4. 测试与范围：同 Round 1 第 4、5 条。回收器为类型化注入，不得有全局可变单例泄露到 domain crate；
   不得有 `cfg(test)`、无用测试或占位实现。

### R11 实测（2026-10-14，阶段 1 Codex 实测记录；阶段 1 已验收并提交，阶段 2 进展见“R11 阶段 2 归因”及其后各节）

本轮代码未提交。阶段 2 未启动。A4 已补齐 Tokio 空闲线程覆盖，用户确认其延迟满足单次 reclaim ≤ 100 ms；50 ms 仅为占位设计目标。用户随后批准 B1 在 Rolldown 最后 transform 消费者之后释放 chain 的本地补丁，并要求先实施 E 的 SDK session V8 page release；本轮结果见下。

**源码核实与实际范围**

- A：在 `arkdown-build::request` 定义 `BuildHeapReclaimer` / `BuildHeapBoundary`，与 SDK host 一起通过 build/check/watch 请求传递。N-API binding 按现有 allocator cfg 实现；debug、dhat、非 mimalloc 目标无动作。一次性 checker program/session/prepared 释放后、整个 bundle 循环及输出释放后、ABC 前、pack 前分别回收。Rayon broadcast；Tokio park/unpark 检查 epoch；A1–A3 从 Rayon 向 Tokio 公共队列投递空任务，A4 改为有界占位唤醒，调用方始终不等待 worker coverage；细节与实测见下。
- **B1 初始方案未实施，修正上节“无人读链”的归因前提**：`ApiTransformPlugin::transform_sync` 和 `HvigorModuleTransformCollector::transform` 均调用 `get_combined_sourcemap()`；后者还通过 `select_transform_source_map` 比较原始与合成 map。当前 upstream `compiler/src/fast_build/ark_compiler/transform.ts::transformForModule` 对 JS-emitting ETS/TS 和 runtime JS 读取 combined map，`fast_build/system_api/rollup-plugin-system-api.ts` 生成 hires identity map。直接返回 Omitted 会改变这些消费者以及公开 JS plugin ABI 的结果，不能视作无用数据删除。原 map 解析及 typed error 保留，未重复解析作验证。后续用户已批准改为 Rolldown 最后消费者之后释放，见 B1 本地补丁记录；arkdown ETS hook 仍提供完整 map。
- B2：快照 load 改为 `ctx.inner.add_watch_file`，匹配 Rolldown `module_task.rs` 的全局登记。全库检索没有 arkdown 对 `transform_dependencies` 的读取，也没有启用 Rolldown HMR。
- B3：确认持有者为 `source_emit::PrintScratch` 的 TLS 容量：tokens、codegen_positions、supplemental entries/positions 和 OXC allocator。它们在每次 print 后 clear/reset，容量仍驻留；没有发现 bundle output/transform store 在一次性 bundle 返回后继续持有这些分配。新增 printer owner 的 `release_thread_scratch`，在被回收线程上先释放 scratch，再 collect。A4 之前未及时唤醒的 Tokio worker 仍保留 scratch，见下面历史采样结果；A4 在每个实际 collect 的线程上执行同一 owner 释放方法。
- C：SourceStore 保留 ID/path/fingerprint，文本通过共享 content slot 在最后读取后退役；所有文本/字节读取和 SourceInventory trait 都传播 `SourceReadError::Released`，不会重新读盘。动态 ArkTS 在 obfuscation/coverage 后、ABC 前释放；static ArkTS 仍在 ABC 读文本，故延至 pack 前。watch/preview/hot-reload/持有 incremental runtime 的构建不退役。APP 子构建和多 target 共享父快照，保守保留到父会话结束，未提前回收。
- D：`NativeAnnotationFactsConverter` 的每文件 offset map 改为首次 span 转换时构造；空 facts 仍检查 source identity，但不建两个 offset vector。map 原本就随每文件转换结束销毁，本轮没有额外延长生命周期。UTF-16 surrogate 边界错误仍受测试保护。

**逐项收益与局限**

- A 单独：ABC 阶段峰值约 2.17 → 1.73 GB（约 420 MiB），签名附近 2.05 → 1.68 GB。A/B/C/D 的阶段表见下；阶段差值包含调度/V8 波动，不能直接当作互不重叠的分配归因相加。
- B2+B3：ABC 1.73 → 1.63 GB（约 95 MiB）。最终采样中基线约 82 MiB 的 `add_transform_dependency` 分片集合消失；printer/mappings 约 56 → 46.1 MiB，OXC arena 约 19.5 → 16.5 MiB，剩余容量对应未回收线程，当时 B3 仍未全部释放；该采样早于 A4，不能用于断言 A4 后仍有相同容量。
- C：精确加载文本 **222,759,043 bytes / 212.4 MiB，19,954 文件**；传给 checker SnapshotFS 的集合覆盖全部文本，因此“被 API 取出”也是此数。checker 实际 `read_sources` **82,348,751 bytes / 78.5 MiB，13,838 文件**，不能把完整 snapshot 传递误报为全部解析。其他消费者的源代码核实覆盖 bundler source inventory、obfuscation、static_arkts、visual_codegen、plugin_share、source_snapshot；动态构建最后读取点是 ABC 前。退役时精确释放文本 222,759,043 bytes，外部 leased_bytes=0。B → C 的 ABC 阶段峰值另降约 238 MiB。身份/指纹等 SourceStore 元数据继续保留。
- D：最终 checker 峰值采样未再出现基线约 20 MiB 的 `NativeAnnotationFactsConverter → Utf16OffsetMap::new`；仅 query_ranges 约 0.8 MiB。D 的单独 cold footprint 没有测得稳定下降（2.669 → 2.742 GB，单次波动），不把 20 MiB 采样权重声称为已证明的整体峰值收益。**收尾时已撤销 D**（见“R11 收尾”）。
- 最终临时采样：bundler 阶段 live peak **1,321,209,149 → 1,220,346,179 bytes**（约 96.2 MiB）；pack 前 live **381,973,250 → 144,565,515 bytes**（约 226.4 MiB）。map 解析/原始源码副本仍约 218.7 MiB；bundler live 的 1.0 GB 目标未达。采样权重并非精确字节计数，不能与 live counter 相加。探针含 blocking-pool 回收实验，所有探针代码已撤销。

**A1–A3 线程覆盖实验（历史结果）**

初版在 checker 后可见 18/18 个 Tokio worker，bundle/ABC/pack epoch 仅约 3/3/5；从 Rayon 外部队列提交空任务后仍不能保证全覆盖。额外尝试一次 yield，以及独立调度 probe 的多轮 yield，都只唤醒少数 worker，未保留这些尝试。A2 日志可见两个 worker 到签名结束才回收，当时进程 footprint 又下降约 250 MiB。这解释了为何 A1–A3 当时仍未达到 0.9 GB。现有 Tokio 公共 API 的短空任务不保证均匀分派，不能将“提交了 N 个任务”写成“N 个线程都已回收”。

mimalloc v2 `heap.c::mi_heap_collect_ex` 的 `_mi_abandoned_reclaim_all` 只在 main/backing heap force collect 时执行；一般线程也有 `_mi_abandoned_collect`。增加异步 N-API 回调在 Node 主线程 collect，避免等待 JS。一次 checker 边界主线程回调约 3.6 ms，观测到进程 footprint 约 55 MiB 下降，不能将并发进程指标全归因于 tsrs scoped threads。临时对 4 个 blocking worker 投递 collect：bundle 时约 0–2 us，ABC 约 0–721 us，pack 约 135–4069 us；仍未解决约 1.4 GB 驻留，且并发 footprint 无法隔离它们的独立贡献。没有留下额外 blocking-pool 调度。

**测量口径**

独占串行运行 release build / example cold build。A1–A3 时 worktree 未安装 napi CLI，`npx napi` 失败，曾使用另一 checkout 已安装的同版本 `@napi-rs/cli 3.7.1` 可执行文件在当前 `packages/arkdown` 构建。A4 已按新指令改用 `npx -y -p @napi-rs/cli@3.7.1 napi build --platform --release`，仅将 npm cache 指向可写临时目录，没有执行主 checkout 的工具。构建产物为当前 worktree 的 `.node`。原 `memtl.py` 增加 debug 日志过滤，`tlsum.py` 仅补充空 vmmap 数据处理，均为本地临时脚本。沙箱拒绝 vmmap 和 `/usr/bin/time -l` 的 sysctl，故 bench 的 `peak=GB` 为空；footprint 用原 proc_pid_rusage 时间线，不伪造该列。每次时间线结束均删除 example/build。

**逐步 memtl 阶段表**（GB；每格为该阶段峰值，A/B/C/D 累加；A2 为 A4 之前的补充回收方式，单次表仅用于归因）

| 阶段 | base1 | A | +B2/B3 | +C | +D | A2 |
|---|---:|---:|---:|---:|---:|---:|
| startup | 0.37 | 0.27 | 0.26 | 0.27 | 0.26 | 0.27 |
| profiles | 0.33 | 0.25 | 0.23 | 0.23 | 0.23 | 0.23 |
| bundling-step | 2.62 | 2.52 | 2.54 | 2.54 | 2.54 | 2.52 |
| facts-done | 2.84 | 2.62 | 2.65 | 2.67 | 2.74 | 2.65 |
| diag-done | 2.85 | 2.66 | 2.68 | 2.67 | 2.72 | 2.67 |
| sdk-released | 1.11 | 1.30 | 1.31 | 1.29 | 1.30 | 1.23 |
| snapshot-built | 1.30 | 1.43 | 1.46 | 1.46 | 1.47 | 1.40 |
| factory-done | 2.31 | 1.69 | 1.72 | 1.64 | 1.65 | 1.53 |
| scan-done | 2.51 | 2.07 | 2.42 | 2.33 | 1.84 | 1.60 |
| graph-done | 2.55 | 2.21 | 2.42 | 2.33 | 1.92 | 1.60 |
| sources-written | 2.43 | 2.08 | 1.96 | 1.86 | 1.87 | 1.90 |
| bundler-done | 2.39 | 1.81 | 1.69 | 1.86 | 1.85 | 1.89 |
| abc-start | 2.17 | 1.73 | 1.63 | 1.38 | 1.42 | 1.34 |
| pack-start | 2.12 | 1.75 | 1.65 | 1.40 | 1.44 | 1.40 |
| packed | 2.11 | 1.74 | 1.64 | 1.39 | 1.43 | 1.39 |
| signed | 2.05 | 1.68 | 1.59 | 1.34 | 1.37 | 1.15 |

**逐步 bench 原始记录**（cold，诊断均为 10,231 行）

```text
r11-a	cold	run1	ok=1	peak=GB	wall=56.25s	startup=5938ms	profiles=1976	bundling=27389	checker=7486	abc=7848	package=12887	pack=7336	sign=5173	diags=10231	diag=same	artifacts=same	hap=same
r11-a	cold	run2	ok=1	peak=GB	wall=56.55s	startup=4722ms	profiles=1981	bundling=28506	checker=7613	abc=8182	package=12958	pack=7147	sign=5420	diags=10231	diag=same	artifacts=same	hap=same
r11-b	cold	run1	ok=1	peak=GB	wall=56.91s	startup=6122ms	profiles=1825	bundling=28466	checker=7953	abc=7597	package=12700	pack=7356	sign=4976	diags=10231	diag=same	artifacts=same	hap=same
r11-b	cold	run2	ok=1	peak=GB	wall=53.73s	startup=4559ms	profiles=1839	bundling=27710	checker=7473	abc=7586	package=11850	pack=5570	sign=5918	diags=10231	diag=same	artifacts=same	hap=same
r11-c	cold	run1	ok=1	peak=GB	wall=53.66s	startup=4477ms	profiles=1843	bundling=27151	checker=7436	abc=7781	package=12240	pack=7118	sign=4758	diags=10231	diag=same	artifacts=same	hap=same
r11-c	cold	run2	ok=1	peak=GB	wall=53.80s	startup=4509ms	profiles=1854	bundling=27323	checker=7529	abc=7762	package=12185	pack=7184	sign=4644	diags=10231	diag=same	artifacts=same	hap=same
r11-d	cold	run1	ok=1	peak=GB	wall=55.08s	startup=5035ms	profiles=1928	bundling=27887	checker=7601	abc=8086	package=11974	pack=6805	sign=4795	diags=10231	diag=same	artifacts=same	hap=same
r11-d	cold	run2	ok=1	peak=GB	wall=54.11s	startup=4494ms	profiles=1947	bundling=26971	checker=7502	abc=7697	package=12831	pack=6797	sign=5659	diags=10231	diag=same	artifacts=same	hap=same
```

**A4 之前两次 cold 时间线**（GB；与上表相同阶段口径）

| 阶段 | final1 | final2 |
|---|---:|---:|
| startup | 0.35 | 0.27 |
| profiles | 0.32 | 0.23 |
| bundling-step | 2.55 | 2.17 |
| facts-done | 2.72 | 2.36 |
| diag-done | 2.72 | 2.40 |
| sdk-released | 1.24 | 1.09 |
| snapshot-built | 1.40 | 1.25 |
| factory-done | 2.04 | 1.42 |
| scan-done | 2.51 | 1.51 |
| graph-done | 2.51 | 1.55 |
| sources-written | 1.98 | 0.99 |
| bundler-done | 1.95 | 0.99 |
| abc-start | 1.40 | 1.30 |
| pack-start | 1.43 | 1.28 |
| packed | 1.41 | 1.27 |
| signed | 1.34 | — |

final2 的 signed 终止窗口只采到退出后的 0，按缺少有效样本处理；签名期间的峰值取 packed → signed 区间，不将退出后的 0 当作内存收益。

| 指标（两次中位） | 实测 | 目标 | 结论 |
|---|---:|---:|---|
| 整体 footprint | 2.560 GB | ≤ 2.85 GB | 通过 |
| bundler footprint | 2.030 GB | ≤ 1.9 GB | 未达标 |
| ABC footprint | 1.350 GB | ≤ 0.9 GB | 未达标 |
| pack footprint | 1.352 GB | ≤ 0.9 GB | 未达标 |
| sign footprint | 1.339 GB | ≤ 0.9 GB | 未达标 |
| bundler live Rust（一次采样） | 1.220 GB | ≤ 1.0 GB | 未达标 |

最终 8 次边界调用耗时 **2.995–10.905 ms**，异步单线程回收最大 **25.028 ms**，均小于 100 ms；这些是调用/执行耗时，不表示所有 Tokio worker 已在此时间内完成回收。

**A4 之前 bench 原始记录与时间结论**

```text
r11-final	cold	run1	ok=1	peak=GB	wall=64.81s	startup=9415ms	profiles=1852	bundling=26761	checker=7370	abc=7891	package=18754	pack=12011	sign=6346	diags=10231	diag=same	artifacts=same	hap=same
r11-final	cold	run2	ok=1	peak=GB	wall=57.57s	startup=6782ms	profiles=1911	bundling=26731	checker=7372	abc=7583	package=14419	pack=7615	sign=6405	diags=10231	diag=same	artifacts=same	hap=same
```

两次均为 `diag=same artifacts=same hap=same`，diagnostics 10,231 行；`modules.abc` / `sourceMaps.map` 哈希及 HAP 条目与已保存基线相同。wall **64.81 / 57.57 s，中位 61.19 s**。相对上文 R10 的 62.3 / 59.0 s（中位 60.65 s）为 **+0.9%**，在 +3% 内；本轮没有重建并重新配对测量旧版 baseline，不能据此声称比旧 bench 的最快 53.61 s 更快。首轮 startup 9.4 s、pack 12.0 s 明显高于第二轮 6.8 s / 7.6 s，保留两次结果，不剔除慢值。

**验证结果**

- `cargo test -p arkdown -p arkdown-build -p arkdown-session -p arkdown-ets-transform -p arkdown-core -p arkdown-resolver`，并以 `--no-fail-fast` 跑完：除已有 stale-module 测试外通过。新 SourceStore 测试首次因 `/var` 与 `/private/var` 规范路径断言失败，修正后 `cargo test -p arkdown-session --test source_text_lifetime` 两项通过；完整轮次计 990 通过、2 失败、2 ignored；修正并定向重跑后只剩已重现的基线失败。阶段 recording reclaimer、UTF-16 以及现有 incremental snapshot/transform 测试通过。
- `module_source_writer_rewrites_unresolved_native_so_static_import` 在原样 `git archive HEAD` 的 d41da9f 副本中单独重现相同 “stale module source must be removed” 失败，本轮未修改 writer。
- `cargo test -p arkdown-napi`：52 通过、3 ignored；包含两次 build 的自一致性测试。Hvigor fixture harness 的原有环境门控未打开，不将其视为本轮全矩阵 parity 验证。
- 指定的目录式 `node --test packages/arkdown/__test__` 在 Node 22.23.2 返回 MODULE_NOT_FOUND；改为 `node --test packages/arkdown/__test__/*.cjs`：34 通过、6 个 opt-in E2E 跳过。
- `ARKDOWN_NATIVE_CACHE_E2E=1 node --test packages/arkdown/__test__/native-cache.e2e.cjs`：通过，覆盖 Rayon 1/4 线程的 ABC 一致性、缓存命中、输入变化及损坏产物恢复。
- 新 `ARKDOWN_NATIVE_WATCH_E2E=1 node --test packages/arkdown/__test__/native-watch.e2e.cjs`：**当前环境未通过**。初次 build/回收成功，watcher 注册成功，但无文件通知，45 s 超时。独立 `notify::RecommendedWatcher` 在工作区创建/修改文件同样超时；临时目录和工作区两处复现。测试使用 `--expose-gc` 避免 CLI 再派生进程，并在实际 watcher 注册后修改文件，退出时清理子进程。没有因此改动生产 watcher 或用轮询掩盖失败；需在能收到 macOS 文件通知的环境补验。
- `cargo clippy --workspace -- -D warnings`、`cargo clippy -p arkdown-napi --release -- -D warnings`、`cargo fmt --check`、`git diff --check`：通过。额外 release clippy 检查 allocator 启用分支。

**后续范围（已获用户批准）**：B1 在 Rolldown 完成全部 transform hook / collector 消费之后、将 module view 写入持久 module table 之前退役 map chain，且仅限 sourcemap/dev-mode 关闭时；不能在 ETS hook 入口截断。owner 为 `ModuleTask::run_inner` 的 `load_source` 返回点与 `create_ecma_view` 交接。A4 已修复空闲 worker 覆盖并通过 ≤ 100 ms 延迟标准，保持实现不变。阶段 2、tsrs `[patch]`、提交与 push 均未进行。

**A4（有界占位唤醒，2026-10-14）**

仅修改 binding 的回收实现、runtime 配置及本文；不改 Rolldown，不启用 mimalloc v3，不提交或 push。正式配置 `WORKER_HOLD_TIMEOUT = 20 ms`。

- runtime owner 用 `thread_name_fn` 区分启动时创建的 scheduler worker 与之后的 blocking-pool 线程，`on_thread_start` 写 TLS 标志；覆盖分母只算 18 个 scheduler worker。依据 Tokio 1.53.1 `multi_thread::Launch::launch` 与 `blocking::Pool::spawn_thread` 的创建顺序，blocking 线程不会混入计数。
- 回收 owner 持有 epoch、覆盖计数与 Condvar；调用线程和 Rayon 先 collect，再从 Rayon 向 Tokio injection queue 投递 18 个任务。任务 collect 后只在截止时间内占位等待；新 epoch 或全覆盖均唤醒等待者，build caller 不等 Tokio rendezvous。probe1 的无界 barrier 会等住运行 build future 的线程；本实现先同步计入调用 worker，等待始终有界。
- 直接在 `on_thread_unpark` collect 的初稿仍失败：ABC 截止时只有 8/18。源码表明 `worker::park` 的 after_unpark 回调仍处于 searching 状态，下一休眠 worker 要等回调返回、`run_task` 退出 searching 才被通知，导致 collect 串行。A4 在唤醒窗口内将 collect 交给任务；窗口外 unpark 与原 park hook 继续兜底。
- BundleCompleted 与 BeforeBytecode 可能重叠。保留在途边界计数；线程在较新 epoch collect 也覆盖被跳过的旧边界，但不会重复计数。超时不丢弃正在 collect 的完成证据，日志区分 covered 与 collecting。allocator 条件仅在 active/no-op 两个模块上各写一次。

占位上限对照（单次 cold；数字为全覆盖耗时 ms，四个边界最终均为 18/18）：

| 配置 / 标签 | checker | bundle | ABC 前 | pack 前 | 备注 |
|---|---:|---:|---:|---:|---|
| r11-a4-10 | 5.482 | 56.663 | 36.386 | 14.778 | bundle 10 ms 截止时 2 完成、16 正在 collect；pack 17+1 |
| r11-a4-50 | 5.423 | 79.752 | 55.963 | 15.130 | 较长占位未改善 collect 延迟 |
| r11-a4 | 11.049 | 77.988 | 57.344 | 16.686 | 正式 20 ms，第一次 |
| r11-a4-2 | 8.218 | 85.036 | 58.786 | 16.364 | 正式 20 ms，第二次 |

初次修正后的 20 ms 对照 `r11-a4-20b`：checker/ABC/pack 为 6.869/36.954/13.957 ms；bundle 截止时 8/18，逐线程日志证实余下 10 个也完成，最慢 collect 54.992 ms。当时旧 epoch 的 timeout 会丢掉后续完成记录，正式版已修复该日志生命周期问题。10/50 ms 对照与正式版只差常量；保留 20 ms，不将短占位上限误称为 collect 本身的耗时上限。

正式两次边界证据（全部 `outcome=complete workers_covered=18 workers=18`）：

| 边界 | epoch | 调用方 ms（1 / 2） | 完成覆盖 ms（1 / 2） | 覆盖 |
|---|---:|---:|---:|---|
| CheckerReleased | 1 | 6.506 / 5.087 | 11.049 / 8.218 | 18/18、18/18 |
| BundleCompleted | 2 | 5.801 / 8.012 | 77.988 / 85.036 | 18/18、18/18 |
| BeforeBytecode | 3 | 45.867 / 42.182 | 57.344 / 58.786 | 18/18、18/18 |
| BeforePack | 4 | 3.311 / 4.195 | 16.686 / 16.364 | 18/18、18/18 |

调用方 3.311–45.867 ms；异步单线程 collect 最大 75.222 ms。**空闲线程覆盖问题已解决，A4 延迟已验收**：bundle 全覆盖 77.988/85.036 ms、ABC 前 57.344/58.786 ms，均满足 R11 单次 reclaim ≤ 100 ms。用户明确 50 ms 是占位设计目标，不是验收门槛。占位等待与 allocator/scratch 的同步释放是不同耗时，不能用前者的 20 ms 掩盖后者。

正式 `memtl.py r11-a4` / `r11-a4-2` 阶段表（GB，每段峰值；独占串行运行，结束后均删除 example/build）：

| 阶段 | r11-a4 | r11-a4-2 |
|---|---:|---:|
| startup | 0.352 | 0.334 |
| profiles | 0.326 | 0.328 |
| bundling-step | 2.621 | 2.596 |
| facts-done | 2.872 | 2.760 |
| diag-done | 2.843 | 2.837 |
| sdk-released | 1.240 | 1.376 |
| snapshot-built | 1.391 | 1.417 |
| factory-done | 2.353 | 2.381 |
| scan-done | 2.562 | 2.573 |
| graph-done | 2.113 | 2.149 |
| sources-written | 1.987 | 2.022 |
| bundler-done | 1.953 | 2.017 |
| abc-start | 0.601 | 0.603 |
| pack-start | 0.591 | 0.592 |
| packed | 0.573 | 0.579 |
| signed | 0.515 | 0.497 |

签名峰值使用 packed → signed 区间；signed 尾窗只反映退出过程，不用于宣称额外收益。

| 指标（两次中位） | A4 | 目标 | 结论 |
|---|---:|---:|---|
| 整体 footprint | 2.854 GB | ≤ 2.85 GB | 未达 |
| bundler footprint | 2.568 GB | ≤ 1.9 GB | 未达 |
| ABC footprint | 0.602 GB | ≤ 0.9 GB | 通过 |
| pack footprint | 0.591 GB | ≤ 0.9 GB | 通过 |
| sign footprint | 0.576 GB | ≤ 0.9 GB | 通过 |

相对 A4 前 final1/final2 的阶段中位，ABC/pack/sign footprint 分别降低约 **714/726/727 MiB**。这些是时间线差值，包含运行波动，不当作独立 live 分配归因。整体中位为精确 2.854172416 GB，比 2.85 GB 线高约 4.0 MiB，按严格目标记未达，不四舍五入判通过。bundler 仍为 2.568 GB，后续 B1 本地补丁已获批准。此次没有重跑 live heap 采样，不能推称 bundler live ≤ 1.0 GB。

测量工具仍只作两项本地调整：`memtl.py` 允许从环境传入 reclaimer debug filter；`tlsum.py` 对空 vmmap 数据输出无分类明细。proc_pid_rusage 采样、阶段窗口与 artifact 比较逻辑不变。

A4 bench 原始记录（cold，诊断均为 10,231 行）：

```text
r11-a4	cold	run1	ok=1	peak=GB	wall=62.02s	startup=5659ms	profiles=2110	bundling=30331	checker=8034	abc=9100	package=14613	pack=7315	sign=6892	diags=10231	diag=same	artifacts=same	hap=same
r11-a4	cold	run2	ok=1	peak=GB	wall=64.89s	startup=6235ms	profiles=2637	bundling=29902	checker=8200	abc=11022	package=14879	pack=7534	sign=6943	diags=10231	diag=same	artifacts=same	hap=same
```

两轮均为 `diag=same artifacts=same hap=same`。wall 62.02 / 64.89 s，中位 **63.455 s**。当前机器有 DevEco emulator CPU 竞争及约 10 GB swap，且没有重新配对测旧版 baseline；不能用与历史 R10 的差值认定回归或收益。wall ±3% 验收由 Claude 在沙箱外配对 base/new 完成。`peak=GB` 仍因 `/usr/bin/time -l` 的 sysctl 被拒而为空，内存结论来自上述时间线。

A4 验证结果：

- `cargo test -p arkdown-napi`：52 passed，0 failed，3 ignored；debug 链接器仍提示 `__eh_frame` 过大，不影响退出成功。
- `cargo clippy --workspace -- -D warnings`：通过。
- `cargo clippy -p arkdown-napi --release -- -D warnings`：通过，检查实际 allocator 分支。
- `cargo fmt --check`、`git diff --check`：通过。
- 本次三个变更文件为 `packages/arkdown/src/heap_reclaimer.rs`、`packages/arkdown/src/lib.rs`、本文；保留此前 R11 未提交更改。原 watch 环境问题不因本次 cold 结果被视为已验收。B1 的后续批准与测量单列如下。

A4 结论：**worker 覆盖、reclaim 延迟与 ABC/pack/sign 内存达标；本组 bundler / 严格整体峰值未达，wall 待配对验收**。保留 v2；v3 的 2.99 GB checker / 约 2.1 GB bundler / 1.23 GB ABC 为用户已提供的否决结果，此次未重试。

**E（SDK session 释放时归还 V8 pages）**

`SdkPluginHost.collectHeap` 接收 release intent；只有实际删除 session 后传入该 intent，重复 release 不重复 GC。`dispatchBatch` 的阈值回收仍无参数。保持当前 upstream `compiler/src/ets_checker.ts::serviceChecker` 释放 checker 后调用 `global.gc()` 的时点，仅在 Node 22+ 请求 `{ type: 'major', execution: 'sync', flavor: 'last-resort' }`，无产物语义变化。

兼容性核实修正了原先“旧 Node 忽略参数后 full GC”的假设：[Node 18.17.0 gc-extension.cc](https://github.com/nodejs/node/blob/v18.17.0/deps/v8/src/extensions/gc-extension.cc) 和 [Node 20.0.0](https://github.com/nodejs/node/blob/v20.0.0/deps/v8/src/extensions/gc-extension.cc) 只识别 `minor` / `async`，此组选项会进入 truthy 参数的 **minor GC** fallback。[Node 22.0.0](https://github.com/nodejs/node/blob/v22.0.0/deps/v8/src/extensions/gc-extension.cc) 才有已核实的 explicit major/sync 与 last-resort 解析。因此旧版本保留无参数 full GC，未声称它们支持 reduce-memory flavor。releaseSession 测试 stub `global.gc`，覆盖 22 的参数、18.17/20.0 的无参数 fallback、重复释放，以及 batch GC 仍无参数；原跨 session / AST 重建断言保留。

`r11-e` 使用 A4 正式 binding（无需 cargo rebuild）。时间线 exit=0，wall 64.3 s；SDK `retained_committed` 从 A4 第一次的 **203,784,192 → 30,556,160 bytes**，下降 **165.2 MiB**。sdk-released footprint **1.240 → 0.741 GB**，约 **476 MiB**；bundler 最大 **2.562 → 2.152 GB**，约 **391 MiB**（这两项对比 A4 第一次，不能与 V8 字节差叠加）。总体 peak **2.744 GB**，ABC/pack/sign **0.595/0.597/0.574 GB**。阶段表与 B1 对照列在下一节；E 单次数据用于归因，不冒充两次中位验收。

E bench 原始记录（cold）：

```text
r11-e	cold	run1	ok=1	peak=GB	wall=73.76s	startup=6470ms	profiles=2577	bundling=34978	checker=9537	abc=9861	package=19645	pack=11326	sign=7158	diags=10231	diag=same	artifacts=same	hap=same
r11-e	cold	run2	ok=1	peak=GB	wall=68.35s	startup=6291ms	profiles=2295	bundling=36077	checker=13221	abc=8835	package=14605	pack=7629	sign=6539	diags=10231	diag=same	artifacts=same	hap=same
```

两次均 `diag=same artifacts=same hap=same`、10,231 行诊断；wall **73.76/68.35 s，中位 71.055 s**。机器运行 DevEco emulator（用户观测约 70% CPU），约 10 GB swap；checker 时长、V8 增长及 wall 有噪声。保留所有结果，**不以未配对 wall 判回归或收益**；Claude 将在沙箱外做配对 base/new 时间验收。

E 测试：`node --test packages/arkdown/__test__/plugin-adapter.test.cjs` 22 passed；完整 `node --test packages/arkdown/__test__/*.cjs` 34 passed / 6 opt-in E2E skipped。新增 380 bytes 触及旧文件的 44 KiB 架构预算（原文件仅余 7 bytes），将该测试预算调为 45 KiB 并说明 session-release GC 属于 JS ABI owner；入口文件集合、native ownership 和删除契约断言均保留。

**B1（Rolldown 本地补丁）**

用户已批准本地 fork / root `[patch]`，没有提交、push 或改 pinned rev。Rolldown 生产 diff 仅在 `ModuleTask::run_inner` 的 `load_source(...).await?` 之后、`create_ecma_view` 之前增加：

```rust
if !self.ctx.options.is_sourcemap_enabled() && !self.ctx.options.is_dev_mode_enabled() {
  sourcemap_chain = Vec::new();
}
```

所有 load/transform hooks（包含 arkdown ApiTransformPlugin、collector 与公开插件的 `get_combined_sourcemap()`）先消费 map；arkdown ETS transform 的 map 提供、JSON 解析及错误行为保持原样。`Vec::new()` 释放元素及 vector 容量，不将容量留在 module table / watch cache。

后续读者核实：`render_ecma_module` 仅在 sourcemap 开启且非 virtual module 时 collapse；`hmr_stage::render_module_code` 在需 map 时读取，dev-mode 门控完整保留链；`ScanStage::create_sourcemap_channel` 仅在 native-magic-string **且 sourcemap 开启**时创建 handler，因此不会重新填充被释放的 chain。独立 `runtime_module_task.rs` 的单个 virtual runtime module 路径保持原样。incremental scan 的 snapshot 复用 `EcmaView`，不改 normalized options；首次扫描和之后重扫均走相同 owner，缓存不会重新引入已退役的普通模块 maps。

arkdown 根 manifest 临时 patch 全部 **24** 个同源 crate，`cargo tree -p rolldown` 与完整 `cargo metadata` 均确认每个名字恰好一份、全部为 local path，图中没有剩余 Rolldown git package。没有在 crate manifests 增加 path dependency。

本地 fork 另加 `tests/sourcemap_retention.rs`，四种 sourcemap/dev-mode 组合逐一验证 full scan、未变更缓存复用、变更重扫：后续 hook 总能读到前序 map，只有 sourcemap/dev-mode 都关闭时 module chain 为空；native magic-string 选项同时开启。更新 fork 的 bundler-data-lifecycle 文档解释生命周期。`cargo check -p rolldown` 通过；`cargo test -p rolldown --features experimental --test sourcemap_retention` 1 passed；`cargo test -p rolldown --features experimental --test integration sourcemap` 16 passed / 1 ignored。测试核对保留的 `sourcesContent`；缺失 combined map 会在外层断言报告。

`r11-b1-live` 临时复用原 probe 的 allocation counter，**未启用 stack sampling**；100 ms 采样的 bundler live Rust peak 为 **920,341,291 bytes（0.920 GB / 877.7 MiB）**，满足 ≤1.0 GB。ABC 阶段 live peak 124,128,564 bytes，pack 前后为 144,963,998 bytes。相对 A4 前历史采样的 1,220,346,179 bytes 低约 **286.1 MiB**，但这不是同环境/同 sampling 配置的配对结果，不把全部差值归因 B1。counter 探针 run 的 wall/footprint 不用于正式验收。运行后已删除 probe module、逐字节恢复 `lib.rs`，再次用指定 napi CLI 构建正式 binding；没有留下 profiler 代码。

正式 `memtl.py r11-b1` / `r11-b1-2` 阶段表（GB，每段 peak；不含探针；两次 exit=0，wall 84.2 / 65.3 s；每次结束后删除 example/build）：

| 阶段 | A4 两次中位 | E 单次 | B1 第一次 | B1 第二次 |
|---|---:|---:|---:|---:|
| startup | 0.343 | 0.355 | 0.363 | 0.281 |
| profiles | 0.327 | 0.326 | 0.342 | 0.326 |
| bundling-step | 2.608 | 2.599 | 2.604 | 2.616 |
| facts-done | 2.816 | 2.723 | 2.753 | 2.749 |
| diag-done | 2.840 | 2.744 | 2.811 | 2.811 |
| sdk-released | 1.308 | 0.741 | 0.877 | 0.870 |
| snapshot-built | 1.404 | 0.902 | 0.929 | 0.885 |
| factory-done | 2.367 | 1.789 | 1.379 | 1.306 |
| scan-done | 2.568 | 2.079 | 1.657 | 1.605 |
| graph-done | 2.131 | 2.152 | 1.723 | 1.732 |
| sources-written | 2.004 | 2.019 | 1.587 | 1.595 |
| bundler-done | 1.985 | 2.006 | 1.557 | 1.565 |
| abc-start | 0.602 | 0.595 | 0.601 | 0.600 |
| pack-start | 0.591 | 0.597 | 0.600 | 0.598 |
| packed | 0.576 | 0.574 | 0.577 | 0.574 |
| signed | 0.506 | — | 0.524 | 0.554 |

`signed` 为退出尾窗；签名内存验收取 `packed → signed` 区间。bundler max 取 SDK 释放后到 ABC 前的窗口；`bundling-step` 开始后的 checker 峰值计入 overall。

| 内存指标 | B1 实测 | 目标 | 结论 |
|---|---:|---:|---|
| overall footprint，两次中位 | 2.811 GB | ≤ 2.85 GB | 通过 |
| bundler footprint，两次中位 | 1.728 GB | ≤ 1.9 GB | 通过 |
| ABC footprint，两次中位 | 0.600 GB | ≤ 0.9 GB | 通过 |
| pack footprint，两次中位 | 0.599 GB | ≤ 0.9 GB | 通过 |
| sign footprint，两次中位 | 0.576 GB | ≤ 0.9 GB | 通过 |
| bundler live Rust，一次计数 | 0.920 GB | ≤ 1.0 GB | 通过 |

bundler footprint 精确为 **1.723427712 / 1.731602432 GB，中位 1.727515072 GB**。相对 E 单次 2.152229376 GB 再降 **405.0 MiB**；相对 A4 两次中位 2.567553088 GB 共降 **801.1 MiB**（E + B1 累积；其中 A4 → E 为约 396.1 MiB）。这些是同脚本阶段峰值差，不是独立分配类别归因，不与 V8 committed / Rust live 字节简单相加。

正式两次均保留 reclaimer debug 日志，全部四个 epoch 最终 complete；A4 实现本轮没有变更：

| 边界 | epoch | caller ms（1 / 2） | complete ms（1 / 2） | 覆盖 |
|---|---:|---:|---:|---|
| CheckerReleased | 1 | 4.187 / 3.972 | 9.184 / 8.507 | 18/18、18/18 |
| BundleCompleted | 2 | 6.208 / 6.774 | 63.856 / 58.318 | 18/18、18/18 |
| BeforeBytecode | 3 | 30.672 / 22.598 | 34.892 / 26.654 | 18/18、18/18 |
| BeforePack | 4 | 3.666 / 3.028 | 15.267 / 12.746 | 18/18、18/18 |

第一次 bundle 在 27.394 ms 有 `timeout covered=7 collecting=11` 中间日志，之后 63.856 ms 达到 complete；这是有界占位截止，不代表 worker 遗漏。两次 caller 最大 **30.672 ms**，全覆盖最大 **63.856 ms**，均满足 ≤100 ms。

B1 bench 原始记录（正式 binding、cold）：

```text
r11-b1	cold	run1	ok=1	peak=GB	wall=68.23s	startup=6112ms	profiles=2160	bundling=30555	checker=8429	abc=9421	package=19796	pack=11154	sign=8208	diags=10231	diag=same	artifacts=same	hap=same
r11-b1	cold	run2	ok=1	peak=GB	wall=95.12s	startup=6102ms	profiles=2270	bundling=31830	checker=8100	abc=11854	package=42803	pack=26083	sign=16141	diags=10231	diag=same	artifacts=same	hap=same
```

两次均 **`diag=same artifacts=same hap=same`**，10,231 行诊断；`modules.abc` / `sourceMaps.map` hash 与 HAP entry list 均与保存基线一致。wall **68.23 / 95.12 s，中位 81.675 s**，保留慢值。第二次 package 阶段 42.803 s（pack 26.083 s、sign 16.141 s），第一次为 19.796 s；机器 emulator / swap 竞争仍在，不能从未配对数据认定回归或收益。**wall ≤ baseline +3% 仍由 Claude 配对验收**。`peak=GB` 空列原因与 A4 相同（沙箱拒绝 time 的 sysctl），footprint 验收来自两次 proc_pid_rusage 时间线，不填造 bench peak。

本轮内存结论：阶段 1 的 overall / bundler / ABC / pack / sign footprint 与 bundler live Rust 目标均达；A4 延迟和覆盖仍达标。阶段 2 未启动，tsrs 未改；v2 保持不变，未重试 v3。实际文件通知的 watch E2E 环境限制仍需在沙箱外补验；本轮 Rolldown 测试已覆盖缓存复用与重扫，不能替代 OS watcher 验证。

E + B1 最终验证（所有 cargo / example 构建串行，无重叠）：

- 指定 `npx -y -p @napi-rs/cli@3.7.1 napi build --platform --release` 在当前 worktree `packages/arkdown` 完成；npm cache 使用临时目录，未执行主 checkout 的工具。正式 binding 不含 probe。
- `cargo check -p rolldown`：通过（arkdown root patch 图）。
- 本地 fork `cargo test -p rolldown --features experimental --test sourcemap_retention`：1 passed，覆盖四种配置和三次扫描；`cargo test -p rolldown --features experimental --test integration sourcemap`：16 passed / 1 ignored。
- `cargo test -p arkdown-napi`：52 passed / 3 ignored，0 failed。
- `node --test packages/arkdown/__test__/*.cjs`：34 passed / 6 opt-in E2E skipped，0 failed。
- `cargo clippy --workspace -- -D warnings`、`cargo clippy -p arkdown-napi --release -- -D warnings`：均通过。local Rolldown 依赖仍打印 pinned 源码 `oxc_index` 宏的 deprecated 警告；N-API debug 链接有原有 `__eh_frame` 大小警告，命令均 exit=0。
- `cargo fmt --check`、Rolldown 两个 Rust 修改文件的 rustfmt check、两仓库 `git diff --check`：通过。
- 无残留 probe，`lib.rs` 与加探针前备份字节一致；最后一次 example/build 已删除。两仓库均未 commit / push。

本轮新增/修改范围为 arkdown 根 `Cargo.toml` / `Cargo.lock`、`packages/arkdown/js/plugin-adapter.js`、其 `plugin-adapter.test.cjs` / `runtime-architecture.test.cjs` 及本文；此前 R11 改动保留。Rolldown 为 `crates/rolldown/src/module_loader/module_task.rs`、新增 `crates/rolldown/tests/sourcemap_retention.rs` 和 `internal-docs/bundler-data-lifecycle/implementation.md`；测试新文件仍 untracked，需与 tracked diff 一起审阅。

**变更文件清单**（保持未提交；不含 gitignored binding/测量数据）

```text
Cargo.lock
Cargo.toml
crates/arkdown-build/src/check.rs
crates/arkdown-build/src/compile_project_config.rs
crates/arkdown-build/src/coordinator/bundle_stage.rs
crates/arkdown-build/src/coordinator/executor.rs
crates/arkdown-build/src/obfuscation.rs
crates/arkdown-build/src/project_checker.rs
crates/arkdown-build/src/request.rs
crates/arkdown-build/src/source_snapshot.rs
crates/arkdown-build/src/static_arkts.rs
crates/arkdown-build/src/watch_stage.rs
crates/arkdown-build/tests/check_semantic.rs
crates/arkdown-core/src/lib.rs
crates/arkdown-core/src/source_inventory.rs
crates/arkdown-ets-transform/src/source.rs
crates/arkdown-ets-transform/src/transform/source_emit.rs
crates/arkdown-resolver/src/resolver.rs
crates/arkdown-session/src/source_store.rs
crates/arkdown-session/tests/source_text_lifetime.rs
crates/arkdown/src/plugins/ets_transform.rs
crates/arkdown/src/plugins/ets_transform_tests.rs
meta/tasks/PERF-CHECKER-SDK-PLUGIN-MEMORY.md
packages/arkdown/Cargo.toml
packages/arkdown/__test__/native-watch.e2e.cjs
packages/arkdown/__test__/plugin-adapter.test.cjs
packages/arkdown/__test__/runtime-architecture.test.cjs
packages/arkdown/js/plugin-adapter.js
packages/arkdown/src/heap_reclaimer.rs
packages/arkdown/src/lib.rs
packages/arkdown/tests/self_consistency_test.rs
```

### R11 阶段 1 验收（Claude，2026-10-14）

**B1 落地**：fork 提交 `ohos-rs/rolldown@63112cea`（分支 `perf/drop-sourcemap-chain`，基于原 pin `3aac1451`），
已签名并推送。arkdown 根 `Cargo.toml` 的 7 个 Rolldown 依赖改 pin 到该 rev，删除本地 `[patch]`；`Cargo.lock`
中 24 个同源 crate 全部指向新 rev。上文“临时 patch / local path”的描述只适用于开发期测量。

**正确性**（沙箱外）：
- `cargo test --no-fail-fast` 覆盖 arkdown-napi / session / core / resolver / ets-transform / arkdown / arkdown-build：
  仅 `module_source_writer_rewrites_unresolved_native_so_static_import` 失败，d41da9f 上同样失败，与本轮无关。
- `node --test packages/arkdown/__test__/*.cjs`：34 passed / 6 skipped；
  `ARKDOWN_NATIVE_WATCH_E2E=1` 的 native watch E2E（真实 FS 通知）1/1 通过。
- fork 新增 `crates/rolldown/tests/sourcemap_retention.rs` 1 passed；fork 的 clippy（`-D warnings`）与 fmt 通过。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all --check`、`git diff --check` 通过。
- Node 18.17 / 20.0 gc-extension 源码核实：选项对象只有 `type:'minor'` 或 `execution:'async'` 时才算“找到”，
  否则按 truthy 参数做 minor GC，E 的 Node 22 门控是必要的。

**配对 wall / peak**（example cold，base = d41da9f JS + `base.node`，new = 本轮；交错 3 组；DevEco emulator 运行、
swap 约 8.5/9.2 GB；`/usr/bin/time -l` peak footprint）：

| 组 | base wall | new wall | base peak | new peak | bundling base/new | ABC base/new |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 67.86 s | 54.86 s | 2.82 GB | 2.69 GB | 34.1 / 24.0 s | 9.2 / 7.6 s |
| 2 | 72.14 s | 55.94 s | 2.77 GB | 2.75 GB | 27.3 / 24.7 s | 9.5 / 7.7 s |
| 3 | 55.82 s | 52.84 s | 2.70 GB | 2.72 GB | 24.8 / 24.7 s | 8.4 / 7.9 s |
| 中位 | **67.86 s** | **54.86 s** | **2.77 GB** | **2.72 GB** | | |

每组 new 都不慢于 base（−19% / −22% / −5%），满足 wall ≤ base +3%；诊断 10,231 行、`modules.abc` /
`sourceMaps.map` hash、HAP 条目全部一致。改 pin 后的 binding 再跑一次 cold：2.83 GB / 59.5 s，产物一致
（整体峰值在 checker 段，base 三次 2.70–2.82，属同一波动范围）。

**结论**：阶段 1 验收通过。bundler 2.57 → 1.73 GB、ABC/pack/sign ≈ 2.1 → 0.6 GB、bundler live 0.92 GB；
整体峰值仍由 checker 段决定（2.69–2.83 GB），阶段 2 的 ≤ 2.75 GB 需要降低 checker 段驻留（tsrs + V8）。

### R11 阶段 2 归因（Claude，2026-10-14，阶段 1 提交后，只测不改）

cold example，`vmmap` 每 1 s + `phys_footprint` + JS 主线程 200 ms `process.memoryUsage()`（`/tmp/arkmem/tl/p2-attr`，
采样开销与机器负载使 wall 167.7 s，不作耗时依据）。checker 在 Bundling 步内先于 bundler 运行
（07:27:53 → 诊断完成 07:28:25.8 → SDK session 释放 07:28:30.4 → bundle plugin factory 07:28:42）。

| 时段（相对 transform facts 完成） | footprint 峰 | tsrs arena | mimalloc | V8 heapTotal / heapUsed |
|---|---:|---:|---:|---:|
| normal 末段（−3 → 0 s） | 2514 MiB | 991 | 893 | 517 / 483 |
| strict（0 → +7.2 s） | **2730 MiB**（+7.2 s） | 1079 | 1007–1021 | 517 / 483 |
| strict 后至 SDK 释放（+7.2 → +11.8 s） | 2720 MiB | ↓ | ↓ | 517 / 483 → 29 / 26 |

- **V8 在整体峰值处空转**：heapUsed 从 transform facts 前 2.6 s 到 SDK session 释放完全不变（483 MiB），
  JS 主线程在 strict 段无任何分配。R6 否决的是 normal 段内的 GC 策略（checker 慢 23–50%），并测得 GC 后活跃堆
  ~280 MB 由声明 AST、应用 AST LRU、源码文本构成。normal→strict 边界处 normal 池输出已全部 owned，JS 空闲，
  一次性丢弃 AST 并做 reduce-memory GC 不在 checker 关键路径上；源码 snapshot 必须保留（tsrs 每 session 只发送
  一次全文，`WireReference.sent`），strict 段若有 syscap 未命中可由 snapshot 重解析。
- 收益上限：strict 段去掉 V8 余量后，峰值转到 normal 末段（2514 MiB），故 P2-1 单独预计 −200 MiB 左右（≈ −7%）。
  继续降低需削减 normal/strict 两段共同驻留的 Rust 活跃数据。
- **SourceStore**：启动捕获 212 MiB 文本，checker 把全部文本放进 overlay（`project_checker.rs` 构造
  `prepared.sources`），但程序图只读 ~78.5 MiB；~134 MiB 在两段峰值处无 checker 读者。overlay 对缺失键会回落读盘
  （`SourceSnapshot.disk_sources`），所以不能靠删 overlay 键实现驱逐。

阶段 2 执行顺序（修订）：
1. **P2-1 SDK session 边界收缩**：`PluginHost` 在 strict 前非阻塞提交一次 session 收缩请求（丢 AST/节点索引，
   保留 snapshot/config/plugin/class 实例，Node 22+ last-resort GC）。不改 batch 阈值 GC、不加定时器/线程，
   与“不做：V8 GC 调参”不冲突。
2. **P2-3 checker 未读文本驱逐**（用户 2026-10-14 批准契约变更：“按需重读并校验哈希，不一致则报错”）：
   SourceStore 契约从“启动时内容冻结在内存”改为“启动时以 FNV-1a + size 冻结指纹，文本可驱逐，按需重读并校验，
   不一致给类型化错误”。checker 程序创建后驱逐无租约文本，checker 释放后、bundler 前批量并行恢复，Rolldown hook
   不做文件 IO；仅限一次性构建，watch/增量/APP 子构建不变。
3. tsrs syscap 缓存 / vfs 键（原阶段 2 第 1、2 项）在 P2-1、P2-3 落地后按新峰值处的实测决定，须单独或合并
   过 tsrs 5% 门槛；`EntryVec` 只测量。

### R11 P2-1 验收（Claude，2026-10-14）

实现（Codex）：`NativeTypeSession` 持有 `PluginHost`，`arkts_strict_diagnostics` 在 strict 前经
`submit_batch` 非阻塞提交 `compactSession`（未初始化 session 时跳过；失败只告警）。JS `SdkPluginHost` 对
`compactSession` 重置 `sourceFiles`（WeakMap）与 64 项 `sourceLru`，保留 snapshot、config、plugin 与 class 实例，
随后走与 `releaseSession` 相同的 reduce-memory GC；响应统计新增 `committedBefore`。Codex 对比了“仅 GC”
（committed 487.5 → 315.3 MiB）与“丢 AST + GC”（486.0 → 166.0 MiB），选后者；strict 段 0 次新增 batch / reparse
（compaction 与 release 时均为 `parses=5556 reparses=3 batches=5250`）。

复验（沙箱外）：`cargo test -p arkdown-ets-checker --test native` 21 通过；`plugin-adapter` +
`runtime-architecture` Node 测试 29 通过；`plugin-adapter.js` 45,843 B（< 45 KiB）；fmt、checker crate clippy
`-D warnings` 通过。

交替配对 cold（基线 = 阶段 1 binding，机器负载高、swap 8.8/10 GB，wall 仅作参考）：

| 配对 | base peak / wall / checker | P2-1 peak / wall / checker |
|---|---|---|
| 1 | 2.77 GB / 78.2 s / 8.13 s | 2.61 GB / 79.7 s / 8.93 s |
| 2 | 2.88 GB / 96.7 s / 10.71 s | 2.60 GB / 76.1 s / 8.82 s |
| 3 | 2.86 GB / 93.8 s / 10.48 s | 2.59 GB / 97.3 s / 8.58 s |
| 中位 | **2.86 GB** / 93.8 s / 10.48 s | **2.60 GB** / 79.7 s / 8.82 s |

6 次构建均为 10,231 诊断，`diag=same artifacts=same hap=same`。峰值中位 −260 MB（−9%），满足阶段 2
“≤ 2.75 GB”；checker 步与 wall 未见回退。

时间线（`/tmp/arkmem/tl/p21-attr`，vmmap 2 s）：峰值 2520 MiB 移到 transform facts 完成时刻（normal 末段，
V8 heapTotal 476 MiB）；compaction 后 V8 heapTotal 476 → 215 MiB，footprint 1 s 内降到 1816 MiB，strict 段峰值
2370 MiB。下一处峰值由 normal 末段 Rust 驻留（tsrs arena / mimalloc）与 V8 声明 AST 共同决定，P2-3 针对其中
SourceStore 未读文本。

### R11 P2-3 验收（Claude，2026-10-14）

实现（Codex）：
- **契约**（`arkdown-core` `source_inventory`）：启动时冻结 size + FNV-1a 指纹；文本可驱逐，按需重读并校验，
  失败为类型化 `SourceReadError::ReloadIo` / `SnapshotMismatch`（诊断码 `ARKDOWN_SOURCE_RELOAD_IO` /
  `ARKDOWN_SOURCE_SNAPSHOT_MISMATCH`）。`SourceTextHandle` 让 checker 不依赖 session crate。
- **SourceStore**（`arkdown-session`）：每文件 `Captured / Evicted / Released` 三态。
  - `evict_unleased_texts` 在写锁下只驱逐 `Arc::strong_count == 1` 的文本；
  - `restore_evicted_texts` 按排序路径并行重读并校验；
  - `Released` 永不恢复；
  - 统计 `on_demand_reloads`。
- **checker**（`arkdown-ets-checker`）：overlay 改为 `SourceInput::{Captured, Deferred}`，保留全部键，不回落读盘；
  延迟句柄读取错误锁存在 `SourceSnapshot.read_error`，所有 program 调用经 `check_result` 上抛；增量 checker
  拒绝句柄输入。
- **编排**（`arkdown-build`）：`project_checker` 在 `create_program` 后驱逐，在 `CheckerReleased` 回收后、bundle
  plugin factory 前恢复。仅一次性构建：沿用 `owns_source_text_lifecycle`，且要求无 bundle/semantic runtime
  （watch、增量、APP 子构建不变）。

读者审计：checker 期间唯一可能读取被驱逐文本的是 native `SourceSnapshot`（实测 0 次按需重读）。checker 之后的
读者（bundler hook、`source_snapshot.rs` `text_sources_under` 等）都在恢复之后。生产代码不构造
`SourceContent::Bytes`。

复验（沙箱外）：
- core 62、session 111（1 ignored）、ets-checker 24（2 ignored 为既有环境门控的 oxc_corpus）、arkdown-build 120
  通过；
- Node 29 通过；
- `cargo fmt --check` 与 `cargo clippy --workspace --all-targets -D warnings` 通过。

交替配对 cold（基线 = P2-1 binding）：

| 配对 | base peak / wall / bundling / checker | P2-3 peak / wall / bundling / checker |
|---|---|---|
| 1 | 2.64 GB / 71.0 s / 30.5 s / 11.1 s | 2.47 GB / 73.0 s / 33.8 s / 12.9 s |
| 2 | 2.62 GB / 64.9 s / 27.3 s / 8.5 s | 2.47 GB / 66.3 s / 28.9 s / 8.4 s |
| 3 | 2.63 GB / 71.7 s / 30.3 s / 9.9 s | 2.47 GB / 68.2 s / 29.0 s / 9.0 s |
| 中位 | **2.63 GB** / 71.0 s / 30.3 s / 9.9 s | **2.47 GB** / 68.2 s / 29.0 s / 9.0 s |

- 6 次构建均为 10,231 诊断，`diag=same artifacts=same hap=same`。
- 驱逐 6117 个文件、133.9 MiB，0 次按需重读。
- 恢复耗时 314 / 616 / 518 ms，位于 checker 释放与 bundler 之间的关键路径上，但 bundling 步中位未回退。
- 峰值中位 −160 MB（−6%）。

时间线（`/tmp/arkmem/tl/p23-attr`，vmmap 1 s；vmmap 暂停使恢复耗时 1223 ms）：

| 时刻 | footprint | tsrs arena | mimalloc |
|---|---:|---:|---:|
| 驱逐前 → 后（program 创建后） | 1214 → 1119 MiB | | |
| normal 末段（整体峰值） | **2350 MiB** | 785 | 763 |
| strict 最大（facts +6.1 s） | 2336 MiB | 1079 | 873 |
| SDK 释放 + 回收后 | 601 MiB | | |
| 恢复完成 | 708 MiB | | |

normal 末段峰值处 V8 heapTotal / heapUsed 为 410 / 348 MiB，compaction 后降为 182 / 128 MiB；vmmap tag 255 为
159 MiB。

结论：
- normal 与 strict 两段峰值已基本持平，各自在 ~1.2 GB 的 program/AST 底座上增加 ~1.1 GB tsrs checker 状态。
- 把 compaction 提前到 facts 前最多再省 ~14 MiB（strict 会成为峰值），不做。
- 进一步降低必须削减 tsrs checker 驻留（syscap 缓存、vfs 键、符号表），属于阶段 2 第 3 项，须过 tsrs 5% 门槛
  并由用户决定推送与重新 pin。
- 更正（P2-4 归因）：上一条“compaction 提前最多省 ~14 MiB”的前提是 strict 峰值不变。边界 retire + 回收与 strict
  期释放 SDK 快照可把 strict 段压到 normal 末段之下，此时提前 compaction 可削掉 facts 段的 +65 MiB 尖峰，见 P2-4。

### R11 P2-4 归因与范围（Claude，2026-10-14，P2-3 提交后，探针不提交）

探针：
- b1 在 strict 前插入 `release_arkts_build_checkers` + `CheckerReleased` 回收，并在 `compactSession` 分阶段测 GC。
- b2 额外分阶段丢弃 configs / plugins / classInstances，并写 heap snapshot。
- b3 记录 SDK 快照与 locate 访问序列（`/tmp/arkmem/heap/sdkaccess.json`）。

细粒度时间线（`p23-attr`，foot 100 ms）：
- 最后一个 SDK 批次约 38.0 s，此后 V8 heapUsed 持平；normal 末段 footprint ~2283 MiB。
- facts 38.19–40.92 s，footprint +65 → **2348 MiB**（整体峰值）；此段 V8 committed 保持 ~495 MiB。
- strict 开始（retire + compaction）41.0 s 降到 1712 MiB。

compaction 时 V8 存活集（heap snapshot 支配树，~266 MiB）：

| 持有者 | MiB |
|---|---:|
| decl AST（`sourceFiles`，含 decl 文本） | ~150 |
| 业务文件快照文本（`snapshots`） | ~89 |
| 代码 / 系统 | ~22 |
| `SyscapChecker`（`SyscapConfig` Map(272)、`cacheMap`(210)、`accessCount`） | 5.6 |

更正：b1 曾得出“decl AST 未释放”，实为探针循环里的栈局部变量 `files` 持有 decl AST Map（149.7 MiB）；生产
`compactSession` 能释放它。

边界 retire + 回收（b1 vs `p23-attr`）：

| 指标 | P2-3 | b1 |
|---|---:|---:|
| normal + facts 最大 | 2350 MiB | 2359 MiB |
| strict 最大 | 2334 MiB | 2266 MiB（另含探针自身 +23 MiB V8） |
| strict 段 mimalloc（vmmap） | 963 | 906 |
| 边界处 mimalloc | 763 | 690 |

strict 峰值实降 70–90 MiB。tsrs `retire_snapshot_checkers` 内部 `take()`，strict 内再 retire 为幂等。

SDK 快照与访问（b3）：

| 类别 | 文件 | 字符 | V8 字符串 | UTF-8 | deflate-1 | 解析 |
|---|---:|---:|---:|---:|---:|---:|
| 业务 | 5250（4460 个双字节） | 39.4 M | 73.3 MiB | 40.2 MiB | 12.4 MiB | 5253 次 / 5225 ms |
| 声明 | 303 | 17.9 M | 25.8 MiB | 17.1 MiB | — | 303 次 / 881 ms |

- 全部转 UTF-8 共 45 ms，全部 deflate 共 435 ms（主线程关键路径，不采用）。
- locate 共 399,354 次，其中 decl 199,802 次；`common.d.ts`（930K 字符）62,973 次。
- **否决 decl AST LRU**：按文本字节预算模拟，70% 预算 → +630 次解析（~+934 ms）；50% → +3.4 s；35% → +8 s。

strict 期 SDK 快照可释放的源码依据（tsrs `4d7e69df`）：
- `tsrs_checker/src/checker.rs` L1856：`oh_sdk.enabled = !for_arkts_linter || strict_checker_only`，即
  非 strict-only 时 strict 池 checker 不做 SDK 检查。
- `tsrs_compiler/src/arkts_diagnostics.rs` `get_arkts_normal_diagnostics`：`need_do_ark_ts_linter` 时对 normal
  池全部 checker `reset_oh_sdk_jsdoc_check()`，后续值查询不再发 SDK pass。
- syscap 批次只来自 `oh_sdk_jsdoc.rs::check_oh_syscap_use` → `flush_oh_syscap_checks`，均受 `oh_sdk.enabled`
  门控；函数插件（value / format / distribution / regex）不读快照。
- 失败路径：`ohsdkplugin.rs::interpret` 对 `phase != 'load'` 的 syscap 错误返回 `Err`，`drain` panic，
  `tsrs_api/src/embedded.rs` L276 `catch_unwind` 转为 `ApiError`。释放后若仍有 syscap 请求，构建必然失败，
  不会静默跳过。
- `WireReference.sent`：每个 tsrs session 只发送一次源码。增量 runtime 的 session 跨重建存活，**只能在一次性
  program 上释放**。

范围（只改 arkdown）：
- **(a) normal 池在 strict 前 retire + 回收**：facts 之后、strict 之前调用 `release_arkts_build_checkers`，并在
  新边界 `BuildHeapBoundary::NormalCheckerReleased` 回收。预期 strict 段 −70~90 MiB。
- **(b) 一次性 program 在 strict 前释放 SDK 快照**，条件为 `need_do_ark_ts_linter && !strict_checker_only`。
  - JS 删除 session 的快照与 project config，保留插件注册。
  - 释放后再请求快照须给出带 session / source 的确定错误；strict-only 与增量 runtime 保持不变。
- **(c) compaction 移到 normal 诊断返回后**（只丢 AST + GC，保留快照，按需重解析）。预期 facts 尖峰消失，
  峰值回到 normal 末段。
- **(d) AST 被丢弃的快照存为 UTF-8 `Buffer`**：业务 AST 被 LRU 淘汰时、以及 compaction 时（业务与声明）。
  - 重解析时解码；文本来自 Rust `str`（无孤立代理项），往返逐字节一致。
  - 预期 normal 末段 V8 −33 MiB 存活；compaction 后快照 99 → 57 MiB（移出 V8 堆），(b) 后为 0。

目标：
- 时间线整体峰值 ≤ 2260 MiB（P2-3 为 2350）。
- strict 段 ≤ 2200 MiB，facts 段不再高于 normal 末段。
- 交替 cold 中位峰值 ≤ 2.40 GB（P2-3 为 2.47 GB），wall 回退 ≤ 3%。
- 10,231 诊断、abc / sourceMaps / HAP 条目逐一一致。

P2-4 之后的剩余杠杆（都在两段 checker 内，需 tsrs 改动、过 5% 门槛并由用户决定推送与 pin）：
- tsrs syscap 结果缓存 ~55 MiB（`tsrs_api/src/ohsdkplugin.rs` ~L398–408）。
- vfs `SyncMap<bool>` / `to_path` 键 ~72 MiB。
- `EntryVec` 容量冗余 54–65 MiB。
- checker 数量：仍列入“不做”，需用户同意以时间换内存。
- 已否决：decl AST LRU（见上）、V8 GC 调参（R6：checker 慢 23–50%）、NodeBuilder scratch region。

### R11 P2-4 验收（Claude，2026-10-14）

实现（Codex）：
- **(a) 边界**：`BuildHeapBoundary::NormalCheckerReleased`。`project_checker::check_program` 在 facts 之后、strict 之前依次执行：
  1. `release_arkts_build_checkers`；
  2. 一次性 program 上的 `release_sdk_sources_before_strict`；
  3. 回收；
  4. strict。

  私有 `SemanticProgramLifetime::{OneShot, Runtime}` 区分一次性与增量 runtime。
- **(b)**：`NativeCompilerOptions::strict_pass_reads_sdk_sources()` 返回 `!need_do_ark_ts_linter || strict_checker_only`，引用 tsrs `checker.rs` L1856。
  - 只有该值为 false 时才发 `releaseSources`。
  - JS 置空 `snapshots`，保留 project config、插件注册与 class 实例。config 只是单个对象，偏离范围无实际影响。
  - 释放后再请求快照，抛出 `SDK checker source <session>:<source>: sources were released before strict diagnostics`，经 invoke 阶段让构建硬失败。
- **(c)**：`compactSession` 从 `arkts_strict_diagnostics` 前移到 `arkts_normal_diagnostics` 成功返回后。`compactSession` 与 `releaseSources` 共用非阻塞的 `submit_session_operation`。
- **(d)**：业务 AST 被 LRU 淘汰时、以及 compaction 时（业务与声明），快照存为 UTF-8 `Buffer`；`locateNode` 重解析前解码一次并回存字符串。
- `plugin-adapter.js` 为 46,004 B，预算 < 46,080 B。

验收修正：Codex 在 `dispatch` 开头加了 `prepared.error` 提前返回，使准备失败优先于插件加载失败。
- 加载失败返回 `phase: 'load'`，tsrs 会缓存并容忍；准备失败返回 `phase: 'invoke'`，构建硬失败。原优先级是先加载、后在 invoke 中抛准备错误。
- 这项改动超出范围，已撤销，并删除相应的 pluginId 99 断言。
- 插件已注册的真实场景下，释放后的请求仍在 invoke 阶段硬失败（测试覆盖）。

复验：
- `cargo fmt --check` 与 `cargo clippy --workspace --all-targets -D warnings` 通过。
- ets-checker `--test native` 22 通过，arkdown-build 全部通过。
- Node 36 通过（6 skipped）。

测试覆盖：
- 边界顺序：`NormalCheckerReleased` 先于 `CheckerReleased`，且 `releaseSources` 在回收前入队。
- linter / strict-only / 关闭 linter 三种选项下，释放与否以及 strict 是否仍发 syscap。
- 增量 runtime 两次重建：共用一个 SDK session，不发 `releaseSources`，每次都有 normal 回收。
- JS 侧：UTF-8 编码 / 解码次数，多字节前缀的 UTF-16 偏移，释放后拒绝且 compaction 不复活快照，函数插件存活。

交替配对 cold（基线 = P2-3 binding）：

| 配对 | base peak / wall / checker | P2-4 peak / wall / checker |
|---|---|---|
| 1 | 2.45 GB / 73.4 s / 7.9 s | 2.36 GB / 68.4 s / 7.9 s |
| 2 | 2.45 GB / 72.1 s / 7.9 s | 2.34 GB / 70.3 s / 8.1 s |
| 3 | 2.46 GB / 70.2 s / 8.4 s | 2.36 GB / 78.4 s / 7.9 s |
| 中位 | **2.45 GB** / 72.1 s / 7.9 s | **2.36 GB** / 70.3 s / 7.9 s |

- 6 次构建均为 10,231 诊断，`diag=same artifacts=same hap=same`，每次都有一条 `sources released` 日志。
- 配对 3 的 wall 偏高来自签名（22.9 s 对 12.7 s，机器换页噪声），checker 与 bundling 未回退。

时间线（`/tmp/arkmem/tl/p24-attr` 对 `p23-attr`，foot 100 ms）：

| 指标 | P2-3 | P2-4 |
|---|---:|---:|
| 整体峰值 | 2350 MiB | **2252 MiB**（normal 末段 / compaction 时刻） |
| facts 段最大 | 2350 MiB | 2252 MiB（不高于 normal 末段） |
| strict 起点 | 1797 MiB | 1561 MiB |
| strict 段最大 | 2334 MiB | **2127 MiB** |
| compaction 后 V8 committed | 168 MiB | 34 MiB |
| facts 耗时 | 2736 ms | 1484 ms |

结论：目标全部达成。
- 峰值中位 2.45 → 2.36 GB（−4%）。R11 累计 2.86 → 2.36 GB（−17%）。
- 整体峰值现由 normal 段 tsrs checker 状态决定，strict 段已低 125 MiB。
- 继续降低只能削减 normal 段 tsrs 驻留（上节剩余杠杆），需 tsrs 改动并由用户决定。

### R11 P2-5 归因与后续方案（Claude，2026-10-14，未实施）

用户决定在 P2-4 处收尾：本节只有归因与方案，**未实施**，tsrs 未改动。以下范围保留为后续工作的起点。

采样堆（`/tmp/arkmem/p25-heap`，P2-4 binding）：

| 时刻 | 采样 live | tsrs | arkdown | 说明 |
|---|---:|---:|---:|---|
| normal 末段（整体峰值处） | ~608 MiB | 421 | 187 | SourceStore 107、project_checker 等 |
| strict 末段 | 696 MiB | — | — | 另含 linter NodeBuilder 39、`enter_context` 24 |

两段都存在、且只为增量或跨批复用服务的持有者：

| 持有者 | normal | strict | 性质 |
|---|---:|---:|---|
| syscap 结果缓存 `SyscapResults.values`（`ohsdkplugin.rs` 完成回调） | 54.3 | 54.3 | 跨批复用缓存 |
| cachedvfs `file_exists` / `directory_exists` 等 memo（`SyncMap<bool>` 键 `String`） | ~24 | ~24 | 纯 memo |
| `sourceFS::track` 的 `seen_files`（每次 `file_exists` 探测都记 `to_path`） | 12.2 | 12.2 | 仅增量更新读取 |
| `sourceFS::missing_directories` | ~3.4 | ~3.4 | 仅增量更新读取 |

syscap 命中探针（临时计数，已撤销，example cold）：
- 231,785 个 syscap 键，只有 **254** 次跨批命中（0.11%）。另有 ~32k 个批内重复键，已由批内 `indices` 去重。
- 结束时缓存保留 199,518 项，约 54 MiB。
- 原因：一个业务文件在 normal pass 只被一个 checker 检查一次。example 配置下 strict 不发 SDK 请求（`strict_pass_reads_sdk_sources()` 为 false），所以一次性构建里这份缓存几乎纯是驻留。
- 只有 watch/runtime（键含文件版本，未改文件跨重建命中）和“strict 也读 SDK”的配置才从中获益。

不在本轮范围：
- `EntryVec` / `SymbolMap`：增长策略已是 8 项以上 1.5×，冗余约 20–25 MiB，且属 binder/checker 核心路径。
- parse cache 索引项（~9.5 MiB）：region 同时由 program 持有，释放索引收益小，且 session 级缓存跨 snapshot 共享。
- arkdown `SourceSnapshot` 小写键（7.5 MiB）：收益小。
- 已否决或已关闭：mimalloc v3（用户否决）、减少 checker 数、V8 GC 调参（R6）、NodeBuilder scratch region、decl AST LRU、共享类型层、checker 退役（`--maxMemory`）。

**范围（tsrs 为主，arkdown 只做调用）**：

**(A) syscap 结果保留策略。**
- tsrs `SdkPluginExecutor` 增加类型化的 syscap 保留策略：
  - 现有行为：保留已完成结果，跨批、跨 pass、跨重建复用，仍是默认。
  - 新策略：只在本批内去重，完成后不保留，也不做跨批预约。
- 两种策略下都必须保持：
  - 插件链折叠（`found && !check_result` 截断）；
  - `interpret` 错误策略；
  - 只对成功结果建立复用，`skipped` 仍为传输层错误；
  - 诊断逐字一致。
- arkdown 只在一次性路径（`SemanticProgramLifetime::OneShot` 对应的 session）且 `!strict_pass_reads_sdk_sources()` 时选用新策略。runtime/watch 与“strict 读 SDK”的配置保持现状。

**(B) 一次性 program 释放增量专用状态。**
- tsrs embedded `ProgramSnapshot` 增加释放增量状态的 API：
  - 清空该 snapshot 的 cachedvfs memo；
  - 清空 compiler host `sourceFS` 的 `seen_files` 与 `missing_directories`。
- 此后以该 program 为 `previous` 调用 `update_program`，必须返回类型化的 client 错误，不能静默得到错误的增量结果。
- memo 被清空后若仍有查询，会按需重新填充，语义不变（一次性构建期间输入文件系统不可变）。
- arkdown 在一次性路径 `create_program` 成功后立即调用；runtime 路径不调用。

预期：两段各降约 −54（A）与 −40（B），整体峰值降约 90–100 MiB。

验收目标（相对 P2-4 binding，交替配对 cold ×3）：
1. 正确性：每次 10,231 条诊断逐字一致；`modules.abc`、`sourceMaps.map` 哈希与 HAP 条目一致。
2. 内存：`/usr/bin/time` peak footprint 中位降 ≥ 80 MiB；时间线整体峰值 ≤ 2160 MiB，strict 段最大 ≤ 2050 MiB。
3. 时间：wall 与 checker 阶段中位不高于基线 3%，instructions retired 不高于 +1%。
4. 测试：
   - tsrs `cargo test -p tsrs_api -p tsrs_project`（含 `TSRS_ARENA_POISON=1` 的 SDK/embedded 集成测试）、`cargo clippy -- -D warnings`、`tools/lint/source.py`；
   - arkdown `cargo test -p arkdown-ets-checker -p arkdown-build`、`node --test packages/arkdown/__test__`、clippy、fmt。
   - 新测试只保护真实行为：
     - 新策略不跨批保留但折叠一致；
     - 默认策略仍跨批命中；
     - 释放后 `update_program` 被拒绝；
     - 释放后仍能查询诊断与 facts；
     - arkdown 只在一次性且 strict 不读 SDK 时选用新策略并调用释放。
5. tsrs `notes/perf-sdk-plugin-memory.md` 增加 R11 P2-5 一节，记录探针数据、指令数与峰值。
6. 不提交 arkdown 的本地 `[patch]`。推送 tsrs、改 pin 前询问用户。

### R11 收尾（Claude，2026-10-14）

用户决定按当前进展收尾：撤销收益未证实的改动，P2-5 不实施。

保留的改动（按提交顺序；收益见上文各节）：

| 项 | 提交 | 实测收益 |
|---|---|---|
| A 阶段边界 allocator 回收（含 B3 printer scratch 释放） | `perf(build): reclaim allocator heaps at one-shot stage boundaries` | ABC 段 2.17 → 1.73 GB |
| C 源码文本在最后读取后退役 | `perf(session): retire source texts after their last reader` | ABC 段再降 ~238 MiB |
| B2 快照 load 只登记全局 watch 文件 | `perf(ets-transform): register snapshot loads only as global watch files` | B2+B3 ABC 段 ~95 MiB |
| E SDK session 释放时归还 V8 pages | `perf(napi): release V8 pages when the SDK session is released` | bundler 段 ~391 MiB |
| B1 Rolldown 退役 transform sourcemap chain | `perf(deps): drop unused Rolldown transform sourcemap chains` | E+B1 bundler 2.57 → 1.73 GB |
| P2-1 strict 前压缩 SDK session | `perf(ets-checker): compact the SDK plugin session before the strict linter` | 峰值 2.86 → 2.60 GB |
| P2-3 源码文本可驱逐并校验重载 | session / ets-checker / build 三个提交 | 峰值 2.63 → 2.47 GB |
| P2-4 normal 后压缩、UTF-8 快照、strict 前释放 | ets-checker / arkdown / build 三个提交 | 峰值中位 2.45 → 2.36 GB |

撤销：
- **D**（`NativeAnnotationFactsConverter` 的 offset map 按需构造）。采样里只少了 ~20 MiB 瞬时分配，cold footprint
  没有稳定下降。收尾时从本地历史删除；每个文件仍在转换期间构造并随即销毁 offset map。
- 上文各节的测量都在删除 D 之前的本地提交上完成；D 只影响 facts 转换的瞬时分配，不改变结论。

未实施：P2-5（上节方案）。

外部依赖 pin 不变：
- tsrs `4d7e69df`（R11 未改 tsrs）。
- Rolldown fork `ohos-rs/rolldown@63112cea`（B1，已推送）。
- hapsigner `0b4a16a`（R10）。

最终复验（删除 D 后的树）：
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo test -p arkdown-build -p arkdown-ets-checker -p arkdown-session -p arkdown-core` 全部通过。
- `node --test packages/arkdown/__test__/*.cjs`：36 passed / 6 skipped。
- example cold ×4：
  - 3 次成功，峰值 **2.38 / 2.38 / 2.36 GB**；10,231 诊断与 `modules.abc` / `sourceMaps.map` / HAP 条目一致。
  - 1 次 es2abc SIGABRT。崩溃报告显示 `AsmEmitter::EmitPrograms` 失败后 `ItemContainer::~ItemContainer` →
    `~CodeItem` double free，属 SDK 进程内部的清理路径。当时 swap 9.4/10 GB；同一树另 3 次通过，未复现。
  - wall 54–84 s，机器负载高（DevEco、swap），不作耗时结论；耗时以阶段 1 配对结果为准。

R11 累计：
- 整体峰值 2.86 → ~2.36 GB（−17%）。
- bundler 段 2.57 → 1.73 GB；ABC / pack / sign ≈ 2.1 → 0.6 GB。
- wall 67.9 → 54.9 s（阶段 1 配对中位）。

剩余峰值在 normal checker 末段（时间线 ~2.25 GB）：tsrs arena ~991 MiB、mimalloc ~738 MiB、V8 ~408 MiB。
Rolldown 段 1.78 GB，与 checker 串行，不决定峰值。继续下降需按 P2-5 改 tsrs。

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

1. 正确性：example `--no-cache` 构建成功；排序后的 warning 列表与基线逐字一致（10,231 条）；
   `modules.abc`、`sourceMaps.map` 哈希与基线一致。
2. 内存：peak footprint ≤ 3.5 GB；checker 期间 V8 heapUsed ≤ 400 MB；SDK 请求 JSON 总量 ≤ 200 MB。
3. 时间：wall time 不高于基线（同机连续测量，取两次）。
4. 测试：tsrs `cargo test -p tsrs_api`、`-p tsrs_checker` 中 SDK 相关测试、lint ratchet；arkdown
   `cargo test -p arkdown-ets-checker -p arkdown-build`、`node --test packages/arkdown/__test__`、
   `cargo clippy --workspace -- -D warnings`、`cargo fmt --check` 全部通过。
5. 范围：diff 只涉及上述文件与必要测试；无 Hvigor 兼容层、无启发式分支、无占位实现。
