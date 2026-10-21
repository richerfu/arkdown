# OXC 重复解析合并验证

2026-10-21；接续 [AST 内存可行性评估](ASSESS-TSRS-AST-MEMORY.md) 的第三个方向。

结论：同一单文件阶段复用 OXC AST 可以落地。此次验证的 source map 路径减少了
23.33% 的累计分配请求，但没有降低局部最大存活分配。它适合作为局部吞吐优化，
不能作为“整体构建峰值内存下降”或“需要迁移 tsrs”的证据。

## 改动范围与源码依据

基线为 arkdown `cabacc667da43b26b14cb5c3a821ba76940ec19e`。
OXC pin 为 `3f2b4b7d5338c10014c973e0db874ec2d3b655db`，本次没有修改依赖。

已阅读当前 ets2bundle `45b3c707d1d173a6d81a514512ef2c831eaf3ec5`
的 README、compiler README 和以下 owner：

- `rollup-plugin-ets-typescript.ts::transform` 与 `process_kit_import.ts`：转换顺序。
- `process_lazy_import.ts`：lazy import 的 AST 行为。
- `module_source_file.ts::processTransformedTsModuleRequest`：模块请求更新。
- `process_module_files.ts::genContentAndSourceMapInfo`：同一个 SourceFile 交给
  `printer.writeFile(node, writer, sourceMapGenerator)` 生成文本和映射。
- 当前安装 SDK 的 `typescript.js::createPrinter`、SourceFile 写出路径：上游委托的 printer 实现。
- pinned OXC `oxc_parser::config::TokensParserConfig`：收集 lexer token 的 parser 配置。

生产修改仅在 [source_map.rs](../../crates/arkdown-ets-transform/src/source_map.rs)：
`HvigorStandardSourceMapBuilder` 原先解析生成文本三次、原始文本三次；现在各解析一次，
在同一 AST 上收集 token、import 前缀、原始 import binding span 和最后一条语句的类型。
两份 AST 依次创建、释放，只有 token 与独立所有权的 facts 离开解析作用域。
解析失败仍返回 `None`；token 对齐、映射策略、打印和 fallback 顺序不变。

删除了四处冗余解析，没有新增缓存、长驻 AST、线程调度器或兼容层。
这只覆盖普通 TS/ETS 的 standard source map 路径；不代表所有 ArkUI 或 JS emit 都减少四次解析。

## 对照方法

环境：macOS arm64，rustc 1.99.0，workspace release profile。
临时 Rust 集成探针同时编译基线文件和候选文件，使两者使用同一个工具链、依赖和输入；
探针运行后移出仓库，生产代码没有计数器或对照分支。

输入来自本机真实大型 Harmony 工程及已物化依赖的 TS/ETS 源文件，排除 build、node_modules、
.hvigor、.git 和 declaration 文件。文件路径排序去重后共 14,146 个：

- 41 个不能解码为 UTF-8，4 个 OXC 初始解析有诊断，明确排除。
- 14,101 个进入比较，原始文本共 63,483,241 bytes。
- 输入读取和 `HvigorTypescriptPrinter::print_with_scoping` 准备生成文本不计入测量。
- 逐文件比较基线与候选的完整 `Option<String>` source map，包括 `None`。
  5,116 个返回映射，8,985 个在两边均返回 `None`；并未把 fallback 情况剔除以放大收益。
- 该语料不等于实际构建的可达模块集合；准备过程也未模拟所有项目转换。
  此实验验证此函数对这些输入的等价性与资源开销，不是整个应用构建的 A/B。

每种实现完成预热后，交替测量六组 A/B、B/A。每轮依次处理同一语料，消费并释放每个输出。
分配版使用委托给 `System` 的全局分配器，记录请求字节、次数和相对轮次起点的最大存活分配。
`realloc` 按新请求的完整大小计数。该数字不是 RSS、footprint 或实际物理内存流量。
计时另编译一份移除全局分配统计的相同探针，避免原子计数影响结论。

## 已测结果

| 指标（完整语料每轮） | 基线 | 合并后 | 变化 |
| --- | ---: | ---: | ---: |
| 两份文本均解析成功时的 parse 次数 / 文件 | 6 | 2 | −4 |
| 无分配统计的耗时中位数 | 3,997 ms | 2,921.5 ms | −26.91% |
| 累计分配请求 | 12,107,853,698 B | 9,282,946,514 B | −23.33%，约 2.63 GiB |
| 分配 / 重分配次数 | 1,103,822 | 1,008,290 | −8.65% |
| 最大存活分配增量 | 31,220,704 B | 31,220,860 B | +156 B，约 29.77 MiB，基本不变 |
| 返回映射总大小 | 9,511,009 B | 9,511,009 B | 逐字一致 |

六轮的分配数值完全一致。从代码生命周期判断，原来的多次解析大部分依次创建和释放，
而单次解析时 arena、lexer token 和自有 token 缓冲的共存仍然存在。
减少重复解析能减少累计分配和 CPU 工作，却不会自然降低最大同时存活量；
本次测量确实未观察到局部峰值下降。此解释来自代码分析，并非分配栈采样归因。

无统计计时的六组原始毫秒数（基线 / 合并后）：3909 / 2918、3913 / 2749、
3992 / 2925、4059 / 3133、4002 / 2816、4200 / 3259。
该耗时只对应本次 source map 函数调用集合，不是整个 transform 阶段或构建用时。

确定性检查：独立 Rayon pool 的 1 / 8 线程结果均与顺序基线逐字一致，
排序后完整结果（包含 `None` 标记）的 SHA-256 均为
`5ec50b3aebf4730c881b91448461273847c230b701bf0fee4fd32f7af193b728`。
该哈希包含本机 source ID，只用于同一输入集合的 A/B 与线程确定性比较。

完成的仓库检查：

- `cargo test --locked -p arkdown-ets-transform`：482 项测试通过，0 失败。
- `cargo clippy --locked -p arkdown-ets-transform --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check` 与 `git diff --check`：通过。

已有标准映射、import binding、类结束位置、模块源码重打印和 transform 合同测试保持通过。
本次没有执行全应用冷构建、ABC/HAP 对比或安装验证，因此不声明这些层面的实测收益。

## 决策

保留这处小范围解析合并作为性能优化。既有映射策略与生命周期保持不变，
删除冗余解析本身足以成立，无需迁移 tsrs。

当前没有整包峰值改善的证据，因此不为这一点启动大范围 AST 迁移或重构整个 transform pipeline。
一次性构建在 bundle 前已释放 tsrs Program；历史 workload 的主峰值位于 checker，
本次变化不触及该阶段。不能把 23.33% 的累计分配下降外推成整包内存下降。

更大范围的 prepare → lowering → finalize → lazy → print 合并在技术上可继续研究，
但必须保持 synthetic node/source span、map composition、shouldEmitJs 与
CompileArkTS/Rolldown 双源码合同。它比本次只读语法事实复用涉及更多输出语义，
尚未实现或测量，不能承诺收益。tsrs 的查询候选收集属于另一处 API/生命周期改动，
本次结果同样不能证明其峰值收益。
