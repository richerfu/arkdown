# ArkTS 1.1 原生 TypeScript 对齐合同

## 结论与范围

Arkdown 的 ArkTS 1.1 类型检查和推断已经统一到适配后的 TSGO；当前发行提交为
`583e9e8b5450c6de49bbe985bd836d7941c7a961`。TSGO 不生成 Arkdown 的 JS、
声明或 annotation runtime metadata，Program 固定 `noEmit`。ArkTS 1.2 不在
本专项范围内。

Arkdown 集成层不调用 TSGO 的 emit/transpile/printer/codegen API。TSGO 返回的
source graph 只界定 checker 的 resolved source closure，annotation/component
结果只包含 immutable semantic facts；Rust resolver、OXC transform、Rolldown
module graph 和 Rust artifact writers 始终拥有实际代码处理。

证据基线：

- OpenHarmony `third_party_typescript` 提交
  `9cc62fe98f47c0bf113676e3fb33fe932b493052`；
- 当前 ets2bundle `compiler/src/ets_checker.ts`、`fast_build/system_api`、
  `process_component_member.ts`、`process_component_build.ts`；
- TSGO `tsc/internal/{parser,binder,checker,module,compiler,api}`；
- Arkdown `arkdown-ets-checker::NativeTypeSession` 和
  `arkdown-build::ProjectSemanticChecker`。

OXC corpus 只验证 OH 已定义的输入，不用于发明 checker 规则。所有行为修改都
必须从上述源码 owner 追踪；产物、缓存、诊断和 sourcemap 只能定位或验证差异。

## 完成矩阵

| 能力 | 上游 owner | 当前实现与验收 |
|---|---|---|
| annotation parser/binder/checker | OH parser/binder/checker | `.ets/.d.ets`、声明/属性/应用/导入导出、常量与 enum、Retention symbol 规则已在 Go frontend 实现；真实 native tests 覆盖 |
| annotation retention/type facts | OH `ohApi.ts::transformAnnotation` | TSGO 批量返回 declaration/use/import、retention、默认值和类型事实；Rust/OXC 完成 runtime lowering |
| struct 与 ArkUI DSL | OH parser/checker、SDK `EtsOptions` | struct constructor/property、Builder/Styles/Extend/component callback、虚拟节点、ETS lib 隔离按 SDK tables 实现；OH/OXC corpus 覆盖 |
| 工程 compiler options | ets2bundle `ets_checker.ts::setCompilerOptions` | target/lib/types/paths、ESMODULE→ES2020 module、Stage→`moduleJson` source model、JS HAR、flow depth、SDK/runtime/bundle/device、strict/Sendable/linter 和 diagnostic switches 从 Rust typed config 传入 |
| OH resolver | OH `moduleNameResolver.ts`、ets2bundle resolver | `oh_modules`/`node_modules`、oh-package exports、kit/SDK fallback、external API、`.js → .d.ets`、`.so`、TS↔ETS/Sendable 已在 compiler owner 实现 |
| 工程 root/source graph | ets2bundle `loadEntryObj/getEntryObj/rootFileNamesCollect` | Rust 从 loader 的 `entryObj + ohExports` 收集入口，TSGO 返回其可达 source graph；build/check 均不递归扫描整个模块目录作为 compiler roots |
| compiler/snapshot 文件身份 | ets2bundle `ets_checker.ts::getCanonicalFileName`、TSGO `tspath.GetCanonicalFileName` | checker 请求始终使用 Program 返回的路径；`SourceStore` 在启动时一次性把 symlink alias 和 TypeScript 大小写规范路径映射回 Rolldown 实际快照路径，查询区间取自快照源码，返回 facts 也只按快照身份发布；热路径不访问文件系统 |
| SDK callbacks | OH checker + ets2bundle `api_check_utils.ts`、`collectExternalApiCheckPlugin/collectExternalApiChecker` | Available/SuppressWarnings/since/syscap/permission/form/crossplatform/atomic-service/FA/Stage/apiAvailable/throws 已实现；Rust options 提供 closure inputs 和完整 compile `projectConfig`；TSGO 保留选择、比较和诊断 owner并经双向 API 批量回调，Rust 只路由，JS CLI 进程执行 SDK CommonJS function/class ABI 与 build-scoped 实例缓存；JSDoc 插件只在 normal semantic 的每个源码检查点执行，strict checker 在普通模式禁用并在完整诊断后 reset；`strictCheckerOnly` 才改由 strict checker 执行；不缓存插件结果 |
| SDK dependency collection | `checkCrossplatformValue/getApiPathFromNode` | checker 在真实 declaration-use 回调记录 `(apiModule,function)`；Rust 按 SDK `depsModuleConfig` 合并 module/component，不再伪造类型推断 |
| ArkTS linter | `ArkTSLinter_1_1/LinterRunner.ts` | 独立 strict checker、ETS/TS 规则、strictCheckerOnly、OH module/path、Sendable/mix-compile 与筛选已实现；OH runner 定义的 1004 个 source case 全部执行并精确比对，无跳过 |
| 诊断合同 | OH `ArkTSLinter_1_1/TSDiagnostics.ts`、ets2bundle `processBuildHap`、TSGO `DiagnosticResponse` | build 专用请求按 normal semantic → syntactic → strict semantic 执行一次并返回 linter/syntactic/semantic 三通道，`processBuildHap` 复用 normal 结果；随后按源码规则处理 declaration，保留 category/code/UTF-16/chain/related info；Rust 应用 ignored-file/UI compatibility 筛选 |
| 类型消费者 | ets2bundle component/annotation transforms | property、Mutable/WrappedBuilder、enum member、annotation 和 SDK dependency facts 已批量接入 OXC；已删除 Rust pseudo checker |
| OH 4.9 类型关系与推断 | OH checker `getInferredType/getCommonSupertype/removeSubtypes/compareSignaturesRelated` | generic candidate/constraint、common-supertype、strict/subtype signature mode 与 union subtype reduction 使用 OH 4.9 语义；union 仅在 subtype reduction 阶段按 allocation id 处理，之后恢复 TSGO canonical order，避免真实 ArkTS 泛型 `Map` 构造被错误收窄 |
| OH 4.9 enum flow | OH checker `getEnumKind/getDeclaredTypeOfEnum` | 含计算 numeric member 的 enum 保持普通 numeric enum，不采用新版 TS 的 member-literal union 穷举收窄；普通 TypeScript 模式仍使用 TSGO 默认语义 |
| ObjectLink 类型事实 | ets2bundle `checkObjectLinkType/checkTypes/isObservedV2/isFunctionType` | TSGO 返回 nullable/basic/ObservedV2/Function 与 union constituent identity；Rust 只执行源码布尔规则，不解析类型文本或重做推断 |
| 生命周期 | TSGO API session/callbackfs | create/update/release、old Program/file changes、内存 snapshot、compiler-owned source graph、EOF/协议失败和进程回收已实现；与源码一样不对完整工程 checker 增加固定超时 |
| 发布 | TSGO package script、npm platform packages | 三平台 clean build + SHA-256 元数据；`tsc/tsc.exe` 与 N-API 同目录发布；环境变量仅作开发覆盖 |

## 架构删除结果

以下旧路径已经删除，禁止恢复：

- Rust isolated-declarations surface/IR/link/infer/relate/print checker；
- Rust/OXC 对 SDK declarations 的 symbol、overload、alias、factory-return 伪推断；
- TSGO 内嵌 `ohSdkPluginWorker`、Node 子进程和 `nodeExecutable` SDK 插件通道；
- 将 annotation SDK 文本删除、改写或通过过滤 TS1146/TS1434 使样例通过；
- 双 checker fallback、PATH 探测和用户必须设置 TypeScript 环境变量的安装合同。

正常 npm 调用由 N-API 根据自身模块位置传入平台包编译器路径。
`ARKDOWN_TYPESCRIPT_PATH` 只用于仓库测试或开发覆盖。

## Watch 增量生命周期

build/check 共享同一套配置和语义服务。watch 按
`rollup-plugin-ets-checker.ts::etsChecker` 复用每个模块/目标的 TSGO session：首轮
创建 Program，后续轮次使用 old Program 和 watcher 的 modified/created/deleted
事件更新 snapshot；新 Program 建立后释放旧 snapshot。默认构建每轮依然重新执行
source graph、build diagnostic batch、declaration diagnostics 和 OXC 所需类型事实
查询，未引入跨轮诊断或 facts 缓存。`build --skip-checker` 在首轮和 watch 重建中都
跳过 linter/syntax/semantic/declaration，只保留保证生成语义的类型事实；独立
`check` 不接受该开关。

真实 JS CLI fixture 验证一个 watch 周期只执行一次 `initialize`；二次构建执行增量
`createProgram` 与旧 snapshot `release`，不产生第二个 TSGO 进程。

产品配置覆盖语言版本是合法行为，不是 checker 缺口；不得在后续审计中再次列为
问题。

## 本轮验证命令

```sh
# TypeScript repo
go test ./tsc/internal/...
OH_TYPESCRIPT_SOURCE=/tmp/arkdown-oh-typescript-source \
  go test ./tsc/internal/compiler -run TestArkTSLinterOpenHarmonyCorpus -count=1
node tools/scripts/package-arkts.mjs

# Arkdown repo
ARKDOWN_TYPESCRIPT_PATH=/path/to/583e9e8b54/arm_darwin/tsc \
  cargo test -p arkdown-ets-checker --test native -- --include-ignored
cargo check --workspace --all-targets

# JS CLI watch：首轮完成并显示 session-watcher started 后修改一个 .ets 文件
RUST_LOG=arkdown_watcher=debug,arkdown_ets_checker=info,arkdown_build=info \
  node packages/arkdown/js/arkdown-cli.js build --project /path/to/project --watch
```

Linux/Windows 产物已交叉构建并验证 hash/元数据；未把交叉构建表述为对应平台运行
验证。Rust native boundary 当前 16/16 通过；真实 HarmonyOS SDK 验证
`5.0.5(17)` 接受 API 11/12、拒绝 API 20。N-API release、JS 28/28、完整 Rust
workspace、Clippy、rustfmt 和 diff check 均通过；清除开发覆盖环境变量后，JS CLI
能从本地 macOS 平台包发现 TSGO 并完成 `check`，npm dry-run 确认 `tsc`、clean
revision 元数据和许可证进入平台 tarball。

真实业务工程的 Program 含 13205 个 source，1739 个 OXC 文件产生 transform type
queries。2026-09-11 在 Apple M2 Pro（12 核、16 GiB）上使用真实 Hvigor 6.21.1 和
Arkdown 0.0.1-beta.11 各完成一轮 debug HAP 对比；两端都从 JS CLI 启动，使用同一
product/module/target，关闭 Hvigor daemon，并将“冷构建”定义为对应公开 `clean` 后
构建、“热构建”定义为源码不变时立即重复构建：

| 实现 | clean 后墙钟 | 无变更热构建 | clean 后峰值 RSS | 热构建峰值 RSS |
| --- | ---: | ---: | ---: | ---: |
| Hvigor | 445.27 s | 4.42 s | 4.23 GiB | 0.55 GiB |
| Arkdown 完整 checker | 305.37 s | 167.92 s | 3.54 GiB | 3.40 GiB |
| Arkdown `--skip-checker` | 158.21 s | 36.53 s | 2.45 GiB | 2.21 GiB |
| Arkdown `--skip-checker`（消除 transform 热点） | 71.40 s | 38.73 s | 2.14 GiB | 2.05 GiB |
| Arkdown `--skip-checker`（复用 checker 池、6 路上限） | 66.86 s | 34.87 s | 2.62 GiB | 2.29 GiB |

完整 checker 的 clean 构建比 Hvigor 少 139.90 秒（31.42%，1.46 倍），显式
`--skip-checker` 少 287.06 秒（64.47%，2.81 倍）。热构建则分别比 Hvigor 慢
37.99 倍和 8.26 倍：完整 checker 每个新进程仍执行 130.510 秒 build diagnostics
及 5.881 秒 facts；skip 路径仍执行 6.139 秒 facts，且两条路径都会重新 pack/sign。
Hvigor 的 CompileArkTS、PackageHap、SignHap 均命中 UP-TO-DATE。

本轮 clean 完整构建中，Arkdown build diagnostics 为 133.447 秒、facts 为 6.923
秒、Rolldown bundle 为 122.240 秒、ABC 为 9.176 秒、pack/sign 为 17.430 秒；skip
路径依次为 6.274、109.048、9.999、18.639 秒。Hvigor CompileArkTS 为 400.796 秒，
PackageHap 为 13.181 秒，SignHap 为 21.631 秒。以上是单轮阶段快照，用于定位优化
所有权，不建立工程路径、固定耗时或跨机器性能合同；OS page cache、已安装依赖和 SDK
保持物化，未将其伪装成首次下载环境。

本轮 `--skip-checker` 使用当前可用的 DevEco 26 SDK，因此与上表旧 SDK 数据不是
严格同环境对比；同一 Arkdown 工程和公开 clean 口径下，墙钟由 158.21 秒降至
71.40 秒，减少 86.81 秒（54.87%，2.22 倍）。源码快照、export binding kind 和
symlink identity 现均在 Rolldown 启动前并行预计算，transform hook 不再递归解析
re-export 或逐文件访问文件系统；Rolldown scan 从原 53–61 秒降至 2.645 秒。
content-addressed transform object 不再逐个执行 `sync_all`，只在 manifest 发布边界
持久化并原子替换，11841 个 object 的 commit 从 37–48 秒降至 3.042 秒。该轮 facts
为 6.649 秒、bundle 12.372 秒、ABC 12.120 秒、pack/sign 17.557 秒；Node 进程线程数
由约 151 降至 38。热构建已绕过 Rolldown 与 ABC，但仍由 facts 6.551 秒和 pack/sign
18.850 秒主导，所以 38.73 秒不能表述为热路径完成。
缓存只覆盖 Program 生命周期内不可变的 project/SDK 文件分类。SDK CommonJS 请求以
transport batch 合并，但仍按请求顺序逐次执行全部插件函数和 class `check`。Rust 为
每个 TSGO 子进程分配独立 host scope，避免多模块并发时进程内 `sessionId` 重复污染
JS source/projectConfig intern 表。

在相同 DevEco 26 SDK 和工程上，普通 build checker 与 transform facts 现共用一个
随 Program 保留的依赖分区 checker 池；facts 也按分区并行、按请求索引稳定合并。
`--skip-checker` 冷构建进一步由 71.40 秒降至 66.86 秒，facts 由 6.649 秒降至
3.895 秒；无变更热构建为 34.87 秒。该轮冷阶段为 bundle stage 15.869 秒
（Rolldown bundler 11.436 秒）、ABC 9.010 秒、pack/sign 22.552 秒。pack/sign
处理的是约 1.5 GB debug HAP：当前产品明确配置
`nativeLib.debugSymbol.strip=false`，不能为了追回旧 benchmark 耗时而擅自剥离符号。
旧 README 的 49.25 秒中位数使用 API 23，且未记录当时 HAP 尺寸或阶段耗时，不能把
当前多出的 17.61 秒全部归因给 Rolldown 回归。

完整 checker 的独立 `check` 在 4/6/8 个 checker 上分别为
121.51/112.81/110.82 秒。8 路相对 6 路只再减少 1.99 秒，并把最大 RSS 提高到约
3.92 GiB，因此 Arkdown 根据主机可用并行度设置 checker 数量、上限为 6。6 路将
43,390 次 facts SDK hook 从约 25,800 批合并为 17,604 批。后续源码审计与真实
Hvigor 6.26.4 冷计数确认，OH 普通模式的 SDK JSDoc hook 只属于 normal semantic：
主模块在完整诊断边界触发 since 417,202、syscap 415,200、regex 208 次，widget
分别为 2,425、2,649、0 次；strict semantic、`getTscDiagnostics` 和
`processBuildHap` 均未增加。`serviceChecker` 在默认内存模式释放 type checker 后，
旧 transform 链重新创建 checker，主模块另触发 7,868/7,753 次；这不是第二轮完整
诊断。

TSGO 先前错误地让 normal、strict 和后续 transform-fact 查询都执行 JSDoc hook，
同工程累计触发 1,300,919 次。发行提交 `583e9e8b54` 按
`createTypeChecker`/`LinterRunner` 源码关闭普通模式 strict callback 并在完整诊断后
reset，降为 450,598 次，减少 850,321 次（65.36%）。SDK 插件仍在 normal semantic
的每个 identifier/property/overload 源码检查点逐次执行，没有按入参缓存或省略。
清除 `ARKDOWN_TYPESCRIPT_PATH`、从 npm 平台包发现该 clean revision 后，真实工程
`check` 墙钟为 59.58 秒；对比修复前同 SDK 的 114.38 秒减少 54.80 秒（47.91%）。
