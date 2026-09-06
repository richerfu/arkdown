# OXC/Rolldown 替换 Hvigor：当前架构与剩余工作

状态：进行中
更新日期：2026-09-06

本文档是替换工作的唯一状态文档。完成项不保留迁移流水账；历史以 Git
提交为准。工程规则以根目录 [`AGENTS.md`](../../AGENTS.md) 为准。

## 1. 目标和证据规则

最终目标是由 OXC、Rolldown 和 Rust 领域 crate 直接实现现代
Stage/ESMODULE 工程的构建行为，并从公开 JS CLI 完成 HAP、HSP、HAR、
APP 和 test 输出。Hvigor 是行为目标，不是需要保留的内部架构。

行为实现必须先阅读下列源码中的实际 owner：

- 当前 `developtools_ace_ets2bundle` 源码 checkout
- 当前 `arkcompiler_ets_frontend` 源码 checkout

文档、旧 Hvigor 反编译代码、旧 specs 和历史实现只能用于定位，不能作为
行为依据。缓存和构建产物只能用于核验源码理解。若源码没有定义行为，停止
实现并向用户确认；禁止根据现象猜测、补 fallback 或创造新的兼容语义。

每项差异必须遵循同一流程：

1. 找到上游源码 owner、调用入口、输入和输出。
2. 将行为归属到 arkdown 的 Rust 领域 owner；只在公开插件 ABI 需要时使用
   JS/N-API。
3. 添加保护该源码契约的单元、fixture 或真实 Hvigor 对比测试。
4. 删除已经失去生产调用者的旧 wrapper、adapter、alias、re-export、测试、
   依赖和文档。
5. 无法按源码实现时返回 typed diagnostic，不能静默降级。

## 2. 最终边界

```text
packages/arkdown/js/arkdown-cli.js
  argv / help / stdio / exit code / TS-JS plugin loading
                         │
                         ▼
                  typed N-API boundary
                         │
                         ▼
                 arkdown-build application
        config + immutable BuildSession snapshot
                         │
             Rolldown graph + Rust plugins
                         │
              OXC transforms and checker
                         │
       resource/profile/native/ABC/obfuscation
                         │
                  package and sign
```

- `packages/arkdown/js/arkdown-cli.js` 是唯一 CLI。Rust 不得增加 binary、
  argv parser、命令生命周期或 CLI-shaped N-API export。
- JS 可以并且应当承担入口、值转换、stdio/exit policy，以及原有 TS/JS
  插件函数与上下文对象的兼容。
- 项目发现、配置合并、任务顺序、缓存、SDK 命令和产物生成必须由 Rust
  直接拥有。
- 内部不保留“Hvigor 兼容层”。与 Hvigor 同名但没有独立领域职责的 facade、
  task wrapper 和双轨实现必须删除。
- 构建不隐式执行 `ohpm install`。arkdown 消费已经物化的 package graph；
  git/workspace 等安装协议属于 `ohpm-rs`，不能复制进本仓库。

## 3. 当前实现

### ArkTS 1.1

ArkTS 1.1 是省略版本参数时的默认路径。只有明确传入
`--arkts-version 1.2` 或等价 N-API 字段才进入 1.2；源码类型、manifest、
依赖或 SDK 内容不得隐式升级版本。

当前 1.1 主链已经由 Rust owner 完成项目快照、解析、OXC 变换、Rolldown
模块图、ABC、资源、profile、native、混淆、缓存、watch、打包和签名。
公开入口只有 JS CLI。真实 Hvigor fixture 矩阵在 strict 模式下要求所有
归一化产物相等，不再允许 allowlist。

这表示当前 fixture 覆盖范围内为零差异，不表示未被源码语料覆盖的行为可以
凭空视为完成。新增源码契约时必须继续扩充矩阵。

### TS/JS 插件

JS/N-API 边界保留活的函数句柄和上下文对象，Rust/Rolldown 拥有 hook 顺序、
模块图、watch 事件和缓存生命周期。ArkTS 1.1 已接通 graph、Hvigor 生命周期、
cache 和 watch hooks。ESMODULE 上游设置 `generateModuleGraphOnly` 和
`disableGenerateBundle`，因此 output hooks 可以被识别但不执行；不能伪造
JavaScript chunk 来触发它们。

TS、MTS、CTS 插件由 OXC/Rolldown 编译。插件 `meta`、side effects、watch
files、共享状态和逐插件 cache 必须继续通过 typed bridge 持久化，JS 不能
取得构建编排所有权。

### Checker

`arkdown-ets-checker` 是嵌入式 OXC 项目 checker。它消费显式 roots、项目
alias、module paths、tsconfig 和 SDK declaration roots，不启动 SDK Node
checker，也没有 SDK checker 兼容桥。其精确能力和保守的 `Unknown` 语义见
[`crates/arkdown-ets-checker/DESIGN.md`](../../crates/arkdown-ets-checker/DESIGN.md)。

### ArkGuard

源码混淆由 `arkdown-obfuscate` 中的 OXC 实现直接处理 source/name/file cache
和 source map；字节码混淆由 Rust 构造计划并调用 SDK 的 PandaGuard 原生工具。
不得恢复 JS ArkGuard bridge、单文件 facade 或 Hvigor task wrapper。

### Cache 和 watch

当前已有版本化的跨进程 OXC transform/fact cache，以及同一 JS CLI watch
进程内的 Rolldown retained graph。Rolldown 目前没有供 arkdown 使用的安全
磁盘 graph deserialize API，因此新进程仍需 resolve/parse/scan。这是明确的
上游能力边界，不得用空 `rollup_cache.json` 或伪图文件冒充命中。

## 4. ArkTS 1.2 的源码执行模型

`ets/static` 是 SDK 组件目录，不是可执行文件。上游普通工程链路为：

1. Node 执行 `ets/static/build-tools/driver/build-system/dist/entry.js`。
2. driver 加载 `ets/static/build-tools/libarkts/lib/libarkts.js`。
3. `Es2pandaNativeModule.ts::findNativeModule` 通过 Node native addon 加载
   `es2panda.node`，由 native binding 驱动 parser/checker/plugin 状态机。
4. driver 只在对应阶段启动 `dependency_analyzer` 和 `ark_link`；启用静态
   混淆时还会启动 `ark_guard`。

SDK 中可能存在独立 `es2panda` 可执行文件，但上游普通构建不直接执行它；它
只出现在 worker 崩溃后的本地诊断路径。因此 arkdown 当前
`StaticArktsCompiler::validate_sdk` 强制检查独立 `es2panda` 是已确认缺口，
应改为按源码校验 `libarkts.js`、`es2panda.node` 和实际子进程工具。

当前 `StaticArktsCompiler` 已生成 source inventory、依赖 module/alias 信息和
普通构建的 UI syntax/UI/memo 插件顺序，并通过 Node driver 生成
`modules_static.abc`。这是过渡接入，不是最终 Rust/OXC 实现。

## 5. 剩余阻塞项

### P0：ArkTS 1.2 完整替换

- 按上述源码模型修正 SDK layout 验证，删除独立 `es2panda` 的错误硬要求。
- 将 libarkts 的 parsed/checked/recheck 生命周期映射为 Rust 驱动的 OXC pass：
  interop、UI、UI syntax、memo 的顺序和共享状态必须逐项来自源码。
- 补齐 SDK alias、permissions、declaration generation 和 interop 输入。
- 按静态源码 owner 接入 `dependency_analyzer`、`ark_link` 和 `ark_guard` 的
  typed Rust command plan、缓存键、诊断及中间产物。
- 补齐 1.2 的 preview、reload/quick-fix、widget、HAR/HSP 和多模块路径；源码
  未定义或尚未实现的组合必须 typed reject。
- 增加可用 `ets/static` SDK 的真实构建 fixture。缺少 `static/` 的 SDK 不能
  用于宣称 1.2 运行时闭环。

### P1：1.1 长尾与架构收口

- 按 ets2bundle 源码测试继续扩充 UI transform、system API、Visual、
  ByteCode HAR、router、worker、InsightIntent 和 HSP 的边界语料。
- 扩展 OXC checker 对复杂泛型、conditional/mapped/indexed/typeof、声明合并、
  global augmentation、namespace、control-flow narrowing 等源码需要的语义；
  不恢复 SDK checker。
- 将 `duplicate_check` 和 `har_dedup` 的 `Arc<Mutex>` collector 改为局部 facts
  加确定性归并，避免在 Rolldown hot path 上共享写锁。
- 在 Rolldown 提供结构化磁盘图接口后再实现跨进程 graph cache；在此之前不
  自创序列化格式。
- 按源码补充更细粒度的 ABC invalidation 和依赖 wave；所有并发结果按 typed
  ID 排序后提交。
- 统一剩余 SDK stderr、插件异常和阶段错误为携带 stage/module/path/span/code
  的 typed diagnostics。

## 6. 完成门槛

只有同时满足以下条件，才可声明相应范围能够替换 Hvigor：

- 所有入口均从 npm JS CLI 或 Node API 进入 typed N-API；不存在 Rust CLI。
- 真实 Hvigor fixture 矩阵在 `ARKDOWN_PARITY_STRICT=1` 下零差异且无 allowlist。
- TS/JS 插件契约、独立 Node 进程 cache 恢复和 watch 重建测试通过。
- 源码契约测试、workspace test、Clippy 和 format 全部通过。
- HAP/HSP/HAR/APP 按变更范围完成结构、签名以及设备/模拟器验证。
- 没有空 adapter、fake cache、双轨实现或只为保留 Hvigor 内部形状而存在的代码。
- ArkTS 1.2 只有在上述 P0 全部完成并通过真实 SDK/产物验证后，才可标记为
  replacement-complete；在此之前默认 1.1 行为不得受影响。
