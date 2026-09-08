# arkdown-ets-checker 设计

## 边界

ArkTS 1.1 的解析、绑定、类型推断、语义诊断、OH resolver、SDK JSDoc 回调和
独立 linter 由适配后的原生 TSGO 提供。当前发行基线是 TypeScript 提交
`6126025532b10e68ee2f710ef288c34ecd034669`。Arkdown 通过
`--api --async` JSON-RPC 启动它，不调用 tsgolint，也不解析 `tsc` 控制台文字。
只有 SDK 明确配置 CommonJS `apiCheckPlugin`、`annotationCheckPlugin` 或
`apiCheckPlugins` 时，TSGO 才在同一 API session 内启动精简 Node worker，保持
源码的 `require()`、RegExp、class 实例和模块缓存 ABI；Node 不承担 checker、
resolver、工程发现或构建编排。

TSGO 只做类型检查和推断。Program 固定 `noEmit`，该字段不向调用者开放；
JavaScript、声明、annotation metadata 和 ArkUI lowering 仍由
Rust/OXC/Rolldown 生成。公开命令入口仍只有 npm 的 JS CLI，TSGO 子进程是
Rust 服务内部编译后端，不是 Arkdown CLI。

Arkdown 不调用 TSGO 的 `emit`、`transpile`、printer 或 JavaScript codegen API。
`getProgramSourceGraph` 只是 checker 已解析源码闭包的只读视图，用来确定诊断和
类型事实的查询集合；它不接管 Rust resolver/Rolldown 的构建模块图。annotation
与 component 查询也只返回 immutable semantic facts，所有 AST 修改和生成文件
仍由 Rust/OXC consumer 完成。该边界由 `NativeCompilerOptions.no_emit` 的私有字段
和 wire-format 回归测试共同保护。

发布时 `tsc`（Windows 为 `tsc.exe`）与 N-API binding 一起放入对应 npm
平台包。N-API 从当前模块文件定位同目录编译器并将绝对路径传给 Rust 请求。
`ARKDOWN_TYPESCRIPT_PATH` 只允许开发和测试覆盖；正常安装不依赖环境变量，
也不会从 `PATH` 猜测一个不匹配协议的 TypeScript。

本合同只描述 ArkTS 1.1。ArkTS 1.2 的静态编译后端和版本选择不属于本 crate。

## 上游源码合同

- TypeScript `tsc/cmd/tsc/api.go`、`tsc/internal/api` 和
  `tsc/internal/ipc/protocol_jsonrpc.go` 定义进程、Program、增量、回调文件系统
  和 JSON-RPC 生命周期。
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

`NativeTypeSession` 拥有子进程、管道、请求编号、Arc 源快照、响应线程和有界
stderr 尾部。请求有超时；Drop 关闭 stdin、终止并回收子进程和线程。Go 拥有
AST、symbol、type 和 Program；Rust 不持有 Go 指针，也不跨 IPC 复制完整 AST。

`ProjectSemanticChecker` 为 build/check 构造同一份工程 roots、SDK/loader
声明、package resolution、SDK 配置和 compiler options。源码优先通过 callback
snapshot 读取，后端磁盘/内嵌 lib 只处理 snapshot 未拥有的文件。TSGO 返回：

- syntactic、semantic、declaration 和独立 ArkTS linter diagnostics；
- compiler-owned resolved source graph 与 type-reference files；
- annotation 声明、使用、import disposition、retention 与常量事实；
- property type、`.builder` receiver 和 enum member 的语义身份；
- 在 SDK callback 处已经解析 overload/alias/return type 后得到的
  `(apiModule, function)` 依赖事实。

诊断保留 code/category、UTF-16 范围、行列、message chain 和 related
information。Rust 将 UTF-16 offset 显式转换为 OXC UTF-8 span。SDK dependency
只在 Rust 中查询 SDK `depsModuleConfig` 并合并最终 module/component inventory；
Rust/OXC 不再重做 TypeScript symbol 或返回类型推断。

## 已完成能力

- `.ets/.d.ets`、struct、ArkUI DSL、annotation 语法/绑定/检查和真实 SDK 声明。
- retention/Available/SuppressWarnings、SDK version/syscap/permission/form/
  crossplatform/atomic-service/FA/Stage、`apiAvailable` 和 `@throws` 回调语义。
- SDK function-style value/format/distribution/regex 回调，以及 class-style
  syscap `check(node, declaration, projectConfig)` 回调；AST 由 SDK 自带 OH
  TypeScript 按 TSGO 的源码 range 重建，callback 选择和诊断仍由 Go owner 处理。
- `oh_modules`、`oh-package.json5`、kit、oh-exports、SDK fallback、`.so`、
  TS-to-ETS/Sendable 解析检查。
- 独立 ArkTS 1.1 linter、strictCheckerOnly、路径/OH module/Sendable/mix-compile
  筛选；OH runner 的 1004 个 source case 全部按消息和位置对比，无跳过。
- build/check compiler options、声明诊断、上游诊断筛选及 OXC 类型消费者。
- create/update/release Program、内存回调、批量查询和 deterministic facts。

旧 Rust isolated-declarations checker、SDK 伪类型推断和双 checker fallback 已
删除。它们不是兼容层，也不再作为备用路径存在。watch 目前仍按 build 调用创建
session；若要复用长驻 Program，必须以 ets2bundle watch 生命周期源码为依据，
不能改变当前诊断或失效语义。

## 发行与验证

TSGO 仓库运行 `node tools/scripts/package-arkts.mjs` 生成 macOS arm64、Linux
x64 GNU 和 Windows x64 MSVC 三个平台目录。`BUILD.json` 必须声明 clean
commit、内嵌 TypeScript libs 和二进制 SHA-256；Arkdown 发布脚本再次验证后才
复制进 npm 平台包。交叉构建只证明可打包，不等于异平台运行测试。

```sh
ARKDOWN_TYPESCRIPT_PATH=/path/to/tsc \
  cargo test -p arkdown-ets-checker --test native -- --include-ignored
ARKDOWN_TYPESCRIPT_PATH=/path/to/tsc \
  cargo test -p arkdown-build --test check_semantic
pnpm --dir packages/arkdown test
```

生产安装无需设置 `ARKDOWN_TYPESCRIPT_PATH`。真实 native tests、OH linter
corpus、workspace check/clippy 和 JS CLI check/build 共同作为门禁；任何单项
通过都不能替代其余层级。
