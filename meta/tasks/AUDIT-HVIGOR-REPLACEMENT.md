# Hvigor 替换全链路审计

审计日期：2026-09-09。Arkdown 起始基线：
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
| checker-driven transform | `ohApi.ts`、component transforms、`checkCrossplatformValue` | TSGO batched facts + OXC consumers | annotation、builder、enum、SDK dependency 已接通；Rust 不重做类型推断 |
| compile-plugin | arkts-compose `getPlugins/doWrapper/getCompilePluginContext` | JS function ABI + N-API + Rolldown adapter | hook、顺序、share、cache、watch、TS/JS module loading 已接通 |
| plugin `this.parse` | Rollup 4.23.0 `ParseAst` | OXC ESTree bridge | 已按源码只消费 `allowReturnOutsideFunction`；未知 Acorn 字段忽略；错误为 `RollupError/PARSE_ERROR` |
| SDK/native tools | ets2bundle command builders、SDK binaries | typed Rust plans | restool/ABC/linker/signer/PandaGuard 等直接调用实际工具；不是 JS 兼容层 |
| source ArkGuard | ets2bundle obfuscation config + arkcompiler ArkGuard source | `arkdown-obfuscate` OXC implementation | Rust 原生 owner；name/file/property cache 与 consumer rules 进入 build stage |
| cache/watch | ets2bundle cache/watch hooks | task cache、OXC facts、Rolldown retained graph、watch coordinator | 单轮行为已接；下述有性能收口 |
| HAP/HSP/HAR/APP | OHOS packaging/signing tasks | `arkdown-pack`、`arkdown-sign`、app coordinator | 现有 1.1 strict fixtures 进入真实产物链 |

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

当前适配后的 TSGO 提交 `94e2465a5f92c50eb86a000eb1c4fb95adac3fd0` 已覆盖 OH parser、
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
代码改写。watch 尚未复用长驻 Program 是性能差异，不是单轮能力差异。

checker roots 已按 ets2bundle `loadEntryObj/getEntryObj/rootFileNamesCollect` 改为
loader 的 `entryObj + ohExports`，由 TSGO 扩展为可达 source graph；递归扫描模块
目录并把未被入口引用的文件送入 checker 的旧路径已经删除。TSGO 同时补齐 OH 4.9
generic candidate/constraint、common-supertype、signature relation 和 union subtype
reduction 顺序，真实业务 `Map<string, ICanvasDraw<any>>` 构造不再被错误收窄。
含计算成员的 numeric enum 同样按 OH 4.9 建模，不会触发新版 enum-union 的错误
穷举。ObjectLink 的 nullable/basic/ObservedV2/Function 身份由 TSGO 返回，Rust
不再从类型语法猜测。

SDK function/class 回调现在由 TSGO checker workers 聚合成有序 transport batch，
Rust typed boundary 转发给 JS runtime；batch 不去重、不缓存结果，每个源码调用仍按序
执行。Source/projectConfig 只做 wire interning，并由 Rust 为每个 TSGO 进程分配唯一
host scope，避免多模块并行构建中进程本地 session id 重复。真实 12333-source 工程
在不可变 SDK/project 文件分类缓存后以 214.21 秒完成，旧路径超过 735 秒仍未完成；
剩余耗时采样集中于正常类型关系、表达式和签名检查，不再是路径归一化或 JS transport
阻塞。

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
coverage records。ArkTS 1.1 strict Hvigor suite 当前 33/33 通过，其中 19 个为
真实构建报告场景，覆盖 HAP、HSP、HAR、ByteCode HAR、test、ohosTest、
multi-module、incremental、release、startup/share/widget，其余为 comparator、
diagnostic 与 harness 合同测试。

本轮严格矩阵曾定位出 3 个真实差异：HSP debug/release 的 interface HAR 和
widget 场景生成的 HAR 多写了 `module.packageName`。当前 DevEco
`PrepareSharedHarRes.generateModuleJson()` 通过显式字段白名单重建 interface
HAR 的 `module.json`，白名单不包含 `packageName`；Arkdown 已在
`HspInterfaceModuleJson` 按同一白名单删除该字段并增加归档内容回归。修复后
HSP debug、HSP release、widget 定向对比及完整 33 项 strict suite 均通过。

### R3：缺少独立真实矩阵的 1.1 路径

当前 strict fixture 没有分别触发 AOT、reload/QuickFix 执行、复杂多模块 APP、
所有签名材料组合和 ArkGuard 全规则/name-cache 复用。这里缺的是对应真实工具与
产物验收，不表示这些 owner 不存在。每个场景加入时必须比较其上游任务实际消费
和生成的结构，不可只检查 command/config 或 `contains` 片段。

## 性能项，不作为产物能力缺失

- watch 每轮重建 TSGO session；可按 ets2bundle watch 生命周期接入已有
  `updateProgram`，但不得改变 diagnostics/type-facts invalidation 顺序。
- Rolldown 暂无当前接入可安全使用的跨进程完整 graph deserialize API；新进程
  仍需 resolve/parse/scan。不得创建空 graph cache 或私有伪格式。
- `duplicate_check` 与 `har_dedup` 仍使用共享写锁 collector，应改为局部 facts
  和确定性 merge。
- 模块 dependency wave 仍可增加并行度；提交前必须按 typed ID 排序，保证产物
  与诊断稳定。

## 下一步顺序

1. 按具体 owner 扩充 R3 真实矩阵；发现差异即在领域 crate 修复。
2. 功能门禁通过后处理 watch/锁/并行性能项。
3. 只有取得当前 task/system plugin 权威源码后，才能决定是否扩展公开插件合同。

完成门槛是 JS CLI/Node API 进入 typed Rust service、ArkTS 1.1 strict fixture
零跳过/零 allowlist、目标插件 ABI 回归、workspace test/clippy/format、真实
HAP/HSP/HAR/APP 结构与签名验证，以及没有内部 Hvigor 兼容层。不得用“尚未证明
全部兼容”代替上述具体结果，也不得用单个 sample 成功宣称全行为完成。
