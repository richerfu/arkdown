# arkdown-ets-checker 设计

## 边界

ArkTS 1.1 的解析、绑定、类型推断、语义诊断、OH resolver、SDK JSDoc 回调和
独立 linter 由适配后的原生 tsrs 提供。发行版本由 `third_party/tsrs` gitlink 固定。Arkdown 通过
Cargo path dependency 将 `tsrs_api::embedded` 链接进 N-API binding；创建、更新和查询使用强类型 Rust 调用。
只有 SDK 明确配置 CommonJS `apiCheckPlugin`、`annotationCheckPlugin` 或
`apiCheckPlugins` 时，tsrs 才通过 `SdkPluginHost` 发出类型检查回调。
Rust 只路由强类型请求；npm 的 JS 入口进程负责 `require()`、RegExp、class 实例
和模块缓存 ABI。tsrs 不启动 Node、不嵌入 JS 源码，JS 也不承担 checker、resolver、
工程发现、代码转换或构建编排。

tsrs 只做类型检查和推断。Program 固定 `noEmit`，该字段不向调用者开放；
JavaScript、声明、annotation metadata 和 ArkUI lowering 仍由
Rust/OXC/Rolldown 生成。公开命令入口仍只有 npm 的 JS CLI。

Arkdown 不调用 tsrs 的 `emit`、`transpile`、printer 或 JavaScript codegen API。
`getProgramSourceGraph` 只是 checker 已解析源码闭包的只读视图，用来确定诊断和
类型事实的查询集合；它不接管 Rust resolver/Rolldown 的构建模块图。annotation
与 component 查询也只返回 immutable semantic facts，所有 AST 修改和生成文件
仍由 Rust/OXC consumer 完成。该边界由 `NativeCompilerOptions.no_emit` 的私有字段
和编译器选项与集成回归共同保护。

发布时编译器 crate 与内嵌 TypeScript 标准库已经链接在平台 N-API binding 内。
`compiler-source.mjs` 校验子模块 clean revision 与 parent gitlink，并附带 LICENSE、NOTICE 和来源记录。
不再分发或查找 `tsrs/tsrs.exe`，开发和生产使用同一份固定源码。

本合同只描述 ArkTS 1.1。ArkTS 1.2 的静态编译后端和版本选择不属于本 crate。

## 上游源码合同

- tsrs `crates/tsrs_api/src/embedded` 和 `tsrs_project` 定义强类型快照、增量、查询与所有权。
- OpenHarmony `third_party_typescript` 的 parser/binder/checker/ohApi、
  `ArkTSLinter_1_1` 和 `moduleNameResolver.ts` 定义 ArkTS/OH 语义。
- ets2bundle `ets_checker.ts::setCompilerOptions/processBuildHap` 定义工程 roots、
  compiler options、语法/语义/linter/声明诊断顺序及筛选。
- ets2bundle `api_check_utils.ts::checkCrossplatformValue/getApiPathFromNode` 定义
SDK dependency 回调；`process_component_member.ts`、
  `process_component_build.ts` 和 `ohApi.ts::transformAnnotation` 定义 Rust
  lowering 所需的 checker facts。

`ets_checker.ts::setCompilerOptions` 的 compile-mode 分支同样属于合同：支持的
ESMODULE 工程使用 TypeScript `ModuleKind.ES2020`；CommonJS 只保留给源码定义的
非 ESMODULE 分支。SDK class checker 获得的 `projectConfig` 直接复用 Rust 为同一
module compile-plugin 构造的完整快照，不能在 checker 层重建较小对象。

不得从生成结果或失败样例创造 checker 规则。实现差异前必须读取上述当前源码和
实际委托 owner；源码未定义时停止并询问。

## 所有权和数据流

`NativeTypeSession` 拥有进程内 `Compiler`、当前源码 inventory、删除 tombstones 和 SDK executor。
每个程序持有不可变 `SourceSnapshot`：内存源码优先，未覆盖输入由磁盘/内嵌标准库提供。
更新成功才提交 inventory。`ProgramSnapshot` 与 `TypeHandle` 的 clone 保留原快照；最后一个 lease
释放时回收 snapshot、options region、checker 与 host，不暴露可悬空的 arena 指针。

每个 Compiler 的专用执行线程保留 tsrs 原生 API 所需的大栈，避免使用 Node/Tokio 小栈进行递归检查。
N-API 在 blocking task 调用编译器，SDK `ThreadsafeFunction` 在 JS 线程执行 CommonJS ABI；
编译器请求和响应不编码为 JSON。仅 SDK 参数/结果使用既有 JS ABI。插件来源与配置 ID 的 scope
在进程内唯一，避免多个 compiler 实例复用错误对象。同步插件调用不添加人为超时。

SDK JSDoc 插件的“一次”是一次 normal semantic 遍历，不是整个工程只调用一次
插件函数。OH `checker.ts::createTypeChecker` 在普通模式为 strict linter checker
清除 `getJsDocNodeCheckedConfig`，`LinterRunner.ts::runArkTSLinter` 在 normal、
syntactic、strict 三个完整诊断阶段结束后又对两个 checker 调用
`resetJsDocCheck`。因此普通模式只允许 normal checker 在每个源码定义的 identifier、
property access 或 overload 检查点执行一次 SDK function/class callback；strict
checker、后续 linter 收集和同一 Program 上的 type-fact 查询不得重复执行。只有
`strictCheckerOnly=true` 时 callback 才由 strict checker 执行。callback executor
仍随 compiler options 一起保留，因为 retention、apiAvailable 等 checker callback
不属于这个 JSDoc reset surface。

`ProjectSemanticChecker` 为 build/check 构造同一份工程 roots、SDK/loader
声明、package resolution、SDK 配置和 compiler options。工程 roots 来自上游
`loadEntryObj/getEntryObj/rootFileNamesCollect` 的 `entryObj + ohExports`，再由 tsrs
解析出可达 source graph；不得用模块目录递归扫描扩大 checker 输入。源码优先通过不可变
snapshot 读取，后端磁盘/内嵌 lib 只处理 snapshot 未拥有的文件。tsrs 返回：

- syntactic、semantic、declaration 和独立 ArkTS linter diagnostics；
- compiler-owned resolved source graph 与 type-reference files；
- annotation 声明、使用、import disposition、retention 与常量事实；
- property type、`.builder` receiver 和 enum member 的语义身份；
- ObjectLink 所需的 nullable/basic/ObservedV2/Function type identity；
- 在 SDK callback 处已经解析 overload/alias/return type 后得到的
  `(apiModule, function)` 依赖事实。

完整 build 使用 tsrs 的 `getArkTSBuildDiagnostics` 一次返回 linter、syntactic 和
semantic 三个通道。该请求严格保留
`ArkTSLinter_1_1/TSDiagnostics.ts::doAllGetDiagnostics` 的 normal semantic →
syntactic → strict semantic 执行顺序，并把 normal semantic 结果直接交给后续
`ets_checker.ts::processBuildHap` 消费，不能再通过第二次查询 重跑普通语义检查。
API/LSP 的单 checker 诊断池不适用于构建；build 请求使用 tsrs 自带的依赖图分区
编译池 pool，并按 Program source index 确定性合并诊断。

JS CLI 的 `build --skip-checker` 是用户明确要求的 Arkdown opt-in 构建模式，不改变
默认的 Hvigor 对齐路径。它跳过 ArkTS linter、syntactic/semantic/declaration
diagnostics 及其 SDK JS 回调，但仍查询 OXC lowering 必需的 annotation/component
类型事实；完全停用 tsrs 会改变 `@ObjectLink`、跨文件 `@Builder`、enum 等生成行为，
因此不属于可接受的“直接打包”。独立 `check` 服务始终执行完整检查。

诊断保留 code/category、UTF-16 范围、行列、message chain 和 related
information。`DiagnosticResponse.pos/end` 按 tsrs 的有符号 `int` 保留，因此无源码
compiler diagnostic 的 `UndefinedTextRange(-1, -1)` 不会在类型边界被误判为非法数字。
Rust 将有源码的 UTF-16 offset 显式转换为 OXC UTF-8 span。SDK dependency
只在 Rust 中查询 SDK `depsModuleConfig` 并合并最终 module/component inventory；
Rust/OXC 不再重做 TypeScript symbol 或返回类型推断。

## 已完成能力

- `.ets/.d.ets`、struct、ArkUI DSL、annotation 语法/绑定/检查和真实 SDK 声明。
- retention/Available/SuppressWarnings、SDK version/syscap/permission/form/
  crossplatform/atomic-service/FA/Stage、`apiAvailable` 和 `@throws` 回调语义。
- SDK function-style value/format/distribution/regex 回调，以及 class-style
  syscap `check(node, declaration, projectConfig)` 回调；AST 由 SDK 自带 OH
  TypeScript 按 tsrs 转换后的 UTF-16 source range 重建，callback 选择和诊断仍由
  tsrs owner 处理，插件加载与 build-scoped 实例缓存只存在于 JS ABI 边界。
- `oh_modules`、`oh-package.json5`、kit、oh-exports、SDK fallback、`.so`、
  TS-to-ETS/Sendable 解析检查。
- 独立 ArkTS 1.1 linter、strictCheckerOnly、路径/OH module/Sendable/mix-compile
  筛选；OH runner 的 1004 个 source case 全部按消息和位置对比，无跳过。
- build/check compiler options、声明诊断、上游诊断筛选及 OXC 类型消费者。
- OH 4.9 的 generic candidate/constraint、common-supertype 与 subtype-union
  allocation-order 语义；修复了 ArkTS `Map<string, ICanvasDraw<any>>` 等真实泛型
  构造调用被新版结构排序错误收窄的问题。
- OH 4.9 computed numeric enum 的普通 enum 建模；不会被新版 member-literal union
  控制流错误穷举。ObjectLink 仅消费 tsrs resolved type facts，Rust 不推断类型。
- create/update/release Program、内存回调、批量查询和 deterministic facts。

旧 Rust isolated-declarations checker、SDK 伪类型推断和双 checker fallback 已
删除。它们不是兼容层，也不再作为备用路径存在。watch 按 ets2bundle
`etsChecker` 生命周期由 Rust 长驻每个模块/目标的 session，后续 source snapshot
通过 old Program 与 modified/created/deleted file changes 更新；新 Program 创建后
释放旧 snapshot。默认构建每轮完整重取 linter、diagnostics、source graph 和 facts；
`--skip-checker` 的 watch 重建继续跳过诊断、保留 facts。JS 只继续承载 SDK
CommonJS callback ABI。

## 发行与验证

先初始化固定的 `third_party/tsrs` 子模块，运行 `pnpm --dir packages/arkdown build`。
Cargo 一次构建包含编译器的 N-API binding。`compiler-source.mjs --verify` 校验源码，
`--package npm` 写入三平台包的许可证和来源记录；三平台运行测试由各自 CI runner 执行。

```sh
cargo test --locked -p arkdown-ets-checker
cargo test --locked -p arkdown-build --test check_semantic
ARKDOWN_OXC_SOURCE=/path/to/oxc cargo test --locked -p arkdown-ets-checker --test oxc_corpus -- --include-ignored
pnpm --dir packages/arkdown test
```

原生集成测试、OH linter corpus、workspace check/clippy 和 JS CLI check/build
共同作为门禁；任何单项通过都不能替代其余层级。
