# ArkTS 1.1 原生 TypeScript 对齐合同

## 结论与范围

Arkdown 的 ArkTS 1.1 类型检查和推断已经统一到适配后的 TSGO；当前发行提交为
`6126025532b10e68ee2f710ef288c34ecd034669`。TSGO 不生成 Arkdown 的 JS、
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
| SDK callbacks | OH checker + ets2bundle `api_check_utils.ts`、`collectExternalApiCheckPlugin/collectExternalApiChecker` | Available/SuppressWarnings/since/syscap/permission/form/crossplatform/atomic-service/FA/Stage/apiAvailable/throws 已实现；Rust options 提供 closure inputs 和完整 compile `projectConfig`；TSGO session 以精简 Node worker 执行 SDK CommonJS function/class ABI，Go 保留选择、比较和诊断 owner；ArkTS linter 的 strict checker clone 保留同一宿主 callback executor |
| SDK dependency collection | `checkCrossplatformValue/getApiPathFromNode` | checker 在真实 declaration-use 回调记录 `(apiModule,function)`；Rust 按 SDK `depsModuleConfig` 合并 module/component，不再伪造类型推断 |
| ArkTS linter | `ArkTSLinter_1_1/LinterRunner.ts` | 独立 strict checker、ETS/TS 规则、strictCheckerOnly、OH module/path、Sendable/mix-compile 与筛选已实现；OH runner 定义的 1004 个 source case 全部执行并精确比对，无跳过 |
| 诊断合同 | ets2bundle `processBuildHap` | syntax → linter/semantic → declaration，保留 category/code/UTF-16/chain/related info；Rust 应用源码定义的 ignored-file/UI compatibility 筛选 |
| 类型消费者 | ets2bundle component/annotation transforms | property、Mutable/WrappedBuilder、enum member、annotation 和 SDK dependency facts 已批量接入 OXC；已删除 Rust pseudo checker |
| 生命周期 | TSGO API session/callbackfs | create/update/release、old Program/file changes、内存 snapshot、compiler-owned source graph、超时和进程回收已实现 |
| 发布 | TSGO package script、npm platform packages | 三平台 clean build + SHA-256 元数据；`tsc/tsc.exe` 与 N-API 同目录发布；环境变量仅作开发覆盖 |

## 架构删除结果

以下旧路径已经删除，禁止恢复：

- Rust isolated-declarations surface/IR/link/infer/relate/print checker；
- Rust/OXC 对 SDK declarations 的 symbol、overload、alias、factory-return 伪推断；
- 将 annotation SDK 文本删除、改写或通过过滤 TS1146/TS1434 使样例通过；
- 双 checker fallback、PATH 探测和用户必须设置 TypeScript 环境变量的安装合同。

正常 npm 调用由 N-API 根据自身模块位置传入平台包编译器路径。
`ARKDOWN_TYPESCRIPT_PATH` 只用于仓库测试或开发覆盖。

## 尚可优化但不属于能力差异

build/check 已共享同一套配置和语义服务。watch 每轮仍创建并销毁 TSGO session，
没有复用已有 `updateProgram` 能力；这是性能和增量生命周期优化，不会令单轮诊断、
类型事实或产物语义缺失。实现该优化前仍须读取 ets2bundle watch 的 session 与
失效顺序，不能自行定义跨轮缓存规则。

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
ARKDOWN_TYPESCRIPT_PATH=/path/to/6126025532/arm_darwin/tsc \
  cargo test -p arkdown-ets-checker --test native -- --include-ignored
cargo check --workspace --all-targets
```

Linux/Windows 产物已交叉构建并验证 hash/元数据；未把交叉构建表述为对应平台运行
验证。Rust native boundary 当前 12/12 通过；真实 HarmonyOS SDK 验证
`5.0.5(17)` 接受 API 11/12、拒绝 API 20。JS CLI、build semantic 和最终
package smoke 结果记录在总体替换任务中。
