# ArkTS 1.1 原生 TypeScript 对齐合同

## 结论与范围

Arkdown 的 ArkTS 1.1 类型检查和推断已经统一到适配后的 TSGO；当前发行提交为
`0780074afa51745e9be859be1ad9400a7f61dbf6`。TSGO 不生成 Arkdown 的 JS、
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
| SDK callbacks | OH checker + ets2bundle `api_check_utils.ts`、`collectExternalApiCheckPlugin/collectExternalApiChecker` | Available/SuppressWarnings/since/syscap/permission/form/crossplatform/atomic-service/FA/Stage/apiAvailable/throws 已实现；Rust options 提供 closure inputs 和完整 compile `projectConfig`；TSGO 保留选择、比较和诊断 owner并经双向 API 批量回调，Rust 只路由，JS CLI 进程执行 SDK CommonJS function/class ABI 与 build-scoped 实例缓存；每次源码调用均执行，不缓存插件结果；strict checker clone 保留同一 callback executor |
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
ARKDOWN_TYPESCRIPT_PATH=/path/to/0780074afa/arm_darwin/tsc \
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

真实业务工程的带 trace 构建中，Program 含 13205 个 source，1739 个 OXC 文件产生
transform type queries。旧链路分别执行 linter 136.266 秒、semantic 64.767 秒和
type facts 5.329 秒，checker 合计约 206.4 秒。build diagnostic batch 复用 normal
semantic 并改用 TSGO 编译 checker pool 后，diagnostic batch 为 126.780 秒、facts
为 6.682 秒，checker 合计约 133.4 秒，减少约 73 秒（35%）；完整缓存构建 208.78 秒并
生成签名 HAP。`--skip-checker` 的 facts 阶段为 7.191 秒，完整无缓存构建 167.56 秒并
生成签名 HAP。测量用于验证阶段归属，不建立工程路径或固定耗时合同。
缓存只覆盖 Program 生命周期内不可变的 project/SDK 文件分类。SDK CommonJS 请求以
transport batch 合并，但仍按请求顺序逐次执行全部插件函数和 class `check`。Rust 为
每个 TSGO 子进程分配独立 host scope，避免多模块并发时进程内 `sessionId` 重复污染
JS source/projectConfig intern 表。
