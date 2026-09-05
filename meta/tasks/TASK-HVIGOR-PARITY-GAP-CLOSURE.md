# hvigor 行为差异审计与清零清单

状态：实施中
审计基线：2026-09-05，`09ec955 feat(cache): persist OXC transform artifacts`
首要范围：默认 ArkTS 1.1、Stage/ESMODULE、唯一 JS CLI 入口
上游基线：`/Users/ranger/Desktop/project/southorange/developtools_ace_ets2bundle`

## 1. 当前结论

arkdown 的 ArkTS 1.1 现代构建主链已经可以构建现有 HAP/HSP/HAR、Test/OHOS
Test 和 APP 场景，但当前证据不足以宣称“完全替换 hvigor”或“所有行为完全无差异”。

完成声明必须同时满足以下条件：

1. 真实 hvigor 对比不依赖未过期 allowlist；确定性差异必须为零。
2. ArkTS 1.1 transform、checker、system API、插件、ABC、资源、打包和签名的
   源码行为矩阵全部有可执行证据。
3. HAP/HSP/HAR/APP、Test/OHOS Test、debug/release、增量、watch/reload 的声明
   范围全部通过产物与运行时验证。
4. 构建业务只在 Rust domain/application owner 中实现；JS 只保留唯一 CLI、原生
   binding 入口和用户 TS/JS 插件 ABI。
5. 内部不存在为了保留 hvigor 类、task、worker、facade 或历史模块名而存在的兼容层。

“JS 层轻”不等于禁止 JS。用户提供的 JS/TS 插件仍在 Node/V8 中执行，JS adapter
负责模块加载和函数句柄转换；项目发现、任务编排、缓存、SDK 命令和产物生成不得进入
JS adapter。

## 2. 审计证据

本清单只使用用户指定的新源码工程，不使用旧 hvigor 反编译代码和旧 specs。

实现证据规则：产物和 cache 对比只用于定位差异，不能作为创造实现逻辑的依据。每项修复
必须先从新源码的公开 hook/task 追到实际 owner，读清其 AST range、配置、状态与输出算法，
再迁移到 arkdown 对应 Rust owner。禁止根据单个 fixture、VLQ token 或“能够通过测试”的
结果增加无上游源码依据的启发式分支。若新源码没有定义该行为，必须停止实现并向用户确认，
不得自行补规则或保留推测性兼容层。

主要入口：

- `docs/kb/06-hvigor调用链分析.md`
- `compiler/compile_plugin.js`
- `compiler/src/fast_build/ets_ui/rollup-plugin-ets-typescript.ts`
- `compiler/src/fast_build/ets_ui/rollup-plugin-ets-checker.ts`
- `compiler/src/process_module_files.ts`
- `compiler/src/fast_build/ark_compiler/`
- `arkui-plugins/interop-plugins/index.ts`
- `arkui-plugins/ui-plugins/index.ts`
- `arkui-plugins/ui-syntax-plugins/index.ts`
- `arkui-plugins/memo-plugins/index.ts`
- `compiler/test/` 和 `arkui-plugins/test/`

当前 arkdown 证据：

- `crates/arkdown-ets-transform/tests/source-parity/arkts11.json`：129 个源码契约。
- `packages/arkdown/tests/hvigor_parity.rs`：真实 hvigor 分层产物比较器。
- `packages/arkdown/tests/parity_allowlist.json`：当前仅保留 1 个有真实产物证据的差异场景。
- `crates/arkdown-ets-checker`：嵌入式 OXC project checker；由 Rust 直接建立项目、
  package alias 和 SDK declaration type environment。
- `crates/arkdown-build/src/static_arkts.rs`：ArkTS 1.2 SDK Node driver。

2026-09-05 使用 DevEco Studio 6.1.1.280 的真实 `hvigorw`、同版本 SDK、release
N-API binding 和空 allowlist 串行复验：31 项中 30 项通过。HAP/HSP/HAR、Test/OHOS Test、
debug/release、source/ByteCode HAR、widget、多模块、share/startup、ComponentV2 冷构建与增量
构建均已零差异；当前唯一失败项是 release ArkGuard fixture。

## 3. 已完成且不得回退

| 能力 | 当前契约 |
|---|---|
| 入口 | npm `packages/arkdown/js/arkdown-cli.js` 是唯一 CLI；Rust 不提供 binary、Clap 或 argv parser |
| ArkTS 版本 | 未指定时固定 ArkTS 1.1；只有显式 `--arkts-version 1.2` 或对应 N-API 字段才进入 1.2 |
| JS 边界 | 手写 JS 仅保留 CLI、runtime entry、TS/JS plugin adapter |
| 内建构建 | resolver、Rolldown graph、ArkTS 1.1 transform、resource/profile/loader、ABC、package/sign 由 Rust owner 负责 |
| ArkGuard | source mode 使用 OXC；bytecode mode 由 Rust 直接调用 SDK `panda_guard`，不得恢复 CJS bridge |
| Visual | Rust parser/IR/OXC codegen，禁止恢复 Node runner |
| Watch | 单进程使用 Rolldown retained graph；不得把 module ID 清单冒充可恢复 graph cache |
| 跨进程 transform cache | 保存 allocator-independent OXC code/map/diagnostics/facts，成功 scan 后原子提交 |

## 4. ArkTS 1.1 阻断项

### P0-1：清除真实产物 allowlist

旧 allowlist 的描述已经过期：空 allowlist 实测已清除 11 个场景，当前只剩 1 个场景对应
1 个失败测试。当前真实差异如下：

| allowlist 场景 | 当前确定性差异 | 下一 owner |
|---|---|---|
| `release-obfuscation-hap` | ArkGuard merged config/reserved names 过度混淆路径、标识符和属性，连带改变 ABC、map、filesInfo/cache/nameCache | `arkdown-obfuscate` / build obfuscation plan |

本轮已从 allowlist 删除并以真实 hvigor 空白名单确认：`minimal-hap`、
`minimal-hap-release`、`bc-har-dep-hap`、`widget-hap`、`share-config-hap`、
`bytecode-har`（同时覆盖 debug/release）、`startup-config-hap`、
`minimal-hap-ohos-test`、`multi-module-hap`、`component-v2-lifecycle-hap` 和
`component-v2-lifecycle-incremental`。ComponentV2 修复依据
`process_struct_componentV2.ts::createPropertyAssignNode` 的 factory-node/source-initializer
所有权，在 AST boundary inventory 中保留 synthetic assignment 占位；同时删除了扫描生成
文本并手工改写 token 的 `ComponentV2SourceMapProjector`。

处理规则：

- 产物差异只用于定位；每个 source-map 修复必须追溯到新源码中的 AST 构造/range 保留、
  TypeScript printer 调用和 map 合成 owner。源码未定义时停止并咨询，不从 VLQ 差异反推规则。
- source map 必须按 `process_module_files.ts::genContentAndSourceMapInfo` 中 TypeScript
  printer/source-map generator 的实际 token 行为对齐，不得手写固定 mappings。
- OHOS Test 必须分别修正 template emit span/格式和 resource cluster/ID owner。
- `filesInfo.txt` 不复制 hvigor 的竞争顺序。比较器必须结构化归一化该非语义顺序，同时
  保持 arkdown 输出确定性。
- 每修复一个差异必须删除对应 allowlist entry；allowlist 为空后删除 allowlist 机制或仅
  保留 schema/expiry 自测，不允许用空泛原因重新扩张。

退出条件：`ARKDOWN_PARITY_STRICT=1` 完整矩阵在空 allowlist 下通过。

### P0-2：补齐 OXC checker 深层类型语义

已把 OXC eager TypeScript checker 接入 ArkTS 1.1 build/check 主链，并补入 ArkTS `.ets`、
`.d.ets`、component struct、package alias、OHPM module path、SDK `globalModulePaths` 和
`skipLibCheck` 项目环境。`checker=sdk`、SDK checker planner/runner、Node 参数、
`sdk-ets-checker-bridge.cjs` 及其 CLI/N-API 字段已经整组删除；build/check 不再存在 SDK
checker 分支。

仍需用上游 `compiler/test` 扩大 conditional/indexed access type、完整结构兼容、复杂表达式
控制流、泛型/重载选择的长尾 corpus。新用例必须进入 OXC type owner，禁止恢复 SDK checker
作 fallback 或比较后端。

实现约束：

- typed query 必须进入 OXC fork 或 `arkdown-ets-transform` 的 semantic owner。
- 一个源文件只 parse/semantic 一次；规则共享 symbol/type facts，不得每条规则重扫 AST。
- 诊断比较 code、severity、span 和稳定顺序，不能只比较 message 文本。
- SDK checker bridge、配置字段、N-API/CLI 暴露已删除，不得因新增 checker 用例恢复。

退出条件：选定源码 checker corpus 全绿，默认和完整 ArkTS 1.1 构建均不启动 checker
Node，仓库不存在 SDK checker bridge。

### P0-3：完成原有 TS/JS 插件 ABI

当前已覆盖 build/watch/source graph 主生命周期，但 `plugin-adapter.js` 明确拒绝
`options`、`outputOptions` 和 `shouldTransformCachedModule`；output/render hooks 在当前
ESMODULE scan-only 链路中 dormant。

实现约束：

- 先从新源码和真实业务插件建立 hook 使用矩阵。
- 新源码实际需要的 `options`/`outputOptions` 必须接入 Rolldown 原生 hook，JS 只转换
  structured values 和函数句柄。
- 新源码未使用的 `shouldTransformCachedModule` 不得凭空模拟；若用户插件依赖该 hook，
  必须基于 Rolldown cache 生命周期定义真实语义后再开放。
- scan-only 不调用的 render/output hook 不宣称支持；若现代产物链切换为 Rolldown output，
  再按真实生命周期开放。
- 内建插件不得经 JS adapter 执行。

退出条件：源码和真实用户 JS/TS 插件 fixture 覆盖 hook 顺序、context、异步返回、错误、
source-map chain、watchChange、cache invalidation 和 close；所有公开 hook 都有真实调用点。

### P0-4：扩大源码与项目行为矩阵

129 个最小 ArkTS 1.1 契约不能证明覆盖完整上游语料。必须建立可追踪 manifest，每条记录：

- upstream owner/file/test；
- ArkTS version、pipeline stage 和输入；
- transform/diagnostic/artifact/runtime expectation；
- arkdown owner 和状态；
- 对应回归测试。

优先补齐复杂组件、system API 类型流、worker/router/InsightIntent、source/bytecode HAR、
多版本依赖、mock/sendable/lazy import、声明合并、preview/reload/quickfix 和 APP。

退出条件：所有声明支持的源码目录/规则族均有 manifest 覆盖；不得用测试数量代替行为域
覆盖率。

## 5. ArkTS 1.2 阻断项

版本分流已完成，但实际构建仍由 `static_arkts.rs` 启动 Node SDK driver。当前没有完整实现：

1. interop parsed → UI parsed；
2. type/semantic checkpoint；
3. interop checked → UI checked → UI syntax checked；
4. memo checked → recheck；
5. permissions、SDK alias、interop SDK path、declgen 等完整配置；
6. Rust 直接编排 es2panda、dependency analyzer、ark_link；
7. static widget/reload，以及声明范围内的 preview/HAR/HSP 行为。

最终路径必须是 OXC static passes + Rust native command plan。Node SDK driver 只能作为迁移期
对照，完成后必须删除，不保留双轨兼容层。

## 6. 架构、缓存和性能缺口

这些不都改变冷构建产物，但属于“完全替换”的发布门禁：

| 编号 | 缺口 | 最终 owner 与验收 |
|---|---|---|
| P1-1 | ABC 仅有整体 task cache/SDK ProgramCache，缺少按 compile record 的可信细粒度缓存 | `arkdown-abc` 保存 record input/result；key 包含 source/options/import records 和 native tool identity；artifact-first/manifest-last 原子提交 |
| P1-2 | SDK fingerprint 未覆盖所有原生二进制内容/版本身份 | `arkdown-session` startup inventory 一次 fingerprint `es2abc`、`es2panda`、`ark_link`、restool、`panda_guard` 等实际使用工具 |
| P1-3 | 新进程仍需 Rolldown resolve/parse/scan | 只有 Rolldown 提供 allocator-independent snapshot API 后才接入；此前明确标为性能限制，不制造伪 cache |
| P1-4 | module coordinator 和 native wave 仍存在顺序循环 | `arkdown-build` 的 `StageExecutor` 按 dependency wave 并行，返回 `(ModuleId, StageOutput)` 后确定性归并 |
| P1-5 | HAR dedup/duplicate check 等 hot collector 使用 `Arc<Mutex>` | Rolldown adapter 产生 per-module typed facts，bundle 完成后按 module ID 排序归并 |
| P1-6 | PreBuild/profile/loader/module map 仍有重复读盘或扫描 | `BuildSession` 成为唯一 startup inventory，后续 owner 只消费 immutable snapshot/plan |
| P1-7 | coordinator 大量 `anyhow/bail`，诊断结构不统一 | domain crate 返回 typed error；application 统一转换为 code/stage/module/path/span/severity/suggestion 并稳定排序 |
| P1-8 | OHPM 只满足 build-time acquire，不是完整 install | 若 CLI 承诺替代 hvigorw bootstrap，`arkdown-ohpm` 需支持多版本 lock graph、项目 materialization、auth/proxy/mirror/workspace/override/peer/dev 语义；否则明确要求外部 `ohpm install` |

性能实现必须同时证明 `RAYON_NUM_THREADS=1` 与 `8` 产物字节一致，不能用并发换取非确定
输出。资源二进制不得进入长生命周期 BuildSession；OXC allocator AST 不得跨任务或持久化。

## 7. 发布验证缺口

- 增加 APP 真实 hvigor parity fixture；当前主 parity 类型没有 APP。
- 为 HAP/HSP/HAR/OHOS Test 增加真实 SDK 版本矩阵，不只验证一个本机版本。
- HAP 验证必须执行卸载目标 bundle → 安装生成 HAP → 启动 ability → 收集日志/页面 smoke；
  禁止覆盖安装掩盖旧数据。
- archive、ABC 和签名中的非语义字段必须结构化归一化；不得忽略整个文件。
- 对不存在本地 SDK/hvigor 的环境允许明确 skip，但发布门禁环境不得 skip。

## 8. 明确非目标和删除原则

以下不是缺失能力，不得重新引入：

- webpack、JSBUNDLE、FA、Terser/Babel 旧链路；
- hvigorfile 任意 task/project facade 运行时；
- daemon/worker sticky state、GC plugin、waitAsync bridge；
- 只写不读的 `rollup_cache.json`、`bundle.json` 或 module-ID-only graph cache；
- 为保留历史类型名、crate、module 或测试而存在的 wrapper/re-export；
- 模仿 hvigor 并发竞争产生的不确定文件顺序。

项目若依赖自定义 hvigor task，必须迁移为 typed arkdown configuration/domain capability 或
公开 TS/JS plugin ABI；内部不得保留 hvigor task 解释器作为兼容层。

## 9. 执行顺序与完成记录

严格按以下顺序推进；一项未有源码/产物证据时不得标记完成：

1. `[进行中：12 → 1]` 复验并清零 ArkTS 1.1 parity allowlist。已对齐 TypeScript
   trailing-comma `lastSourcePosition`、合成零 span token、ByteCode HAR 生成 profile map、
   多模块单次 AST print 和 ComponentV2 synthetic assignment 遍历；唯一剩余项是 release
   ArkGuard，必须先追到新源码 obfuscation task/config owner 后再修改。
2. `[进行中：原生 project checker 与 SDK checker 删除已完成]` 继续按源码 corpus 补齐
   OXC 深层类型语义；禁止恢复 SDK checker fallback。
3. `[未开始]` 完成源代码实际需要的 TS/JS plugin hooks 和兼容 corpus。
4. `[未开始]` 扩展 ArkTS 1.1 source/project/runtime matrix，补 APP 和设备验证。
5. `[未开始]` 实现细粒度 ABC cache 和 native tool fingerprint。
6. `[未开始]` 实现 StageExecutor、无锁 plugin facts 和 BuildSession 单次读取。
7. `[未开始]` 统一 typed diagnostics。
8. `[未开始]` 完成显式 ArkTS 1.2 全 Rust/OXC 流水线并删除 Node SDK driver。
9. `[未开始]` 执行最终 wrapper/bridge/flag/doc 删除审计和全量发布门禁。

每个完成记录必须包含 commit、上游 source owner、测试命令、真实 parity 结果和已删除对象。
