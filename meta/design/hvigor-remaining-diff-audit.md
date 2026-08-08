# arkdown ↔ hvigor 剩余差异审计报告

审计日期：2026-08-08。基准：DevEco Studio 内置 hvigor 6.24.3（minified 源码）+ ts-readable 6.24.2（可读重建）+ SDK ets-loader。方法：hvigor 全部 151 个任务、hvigor-arkts-compose 全部 Rollup 配置/插件、SDK ets-loader 全部模块的逐项静态对比 + 真实 hvigor 构建验证（parity 26/26 全绿，含多模块 fixture）。

## 结论概览

**核心生产路径（标准 Stage 模型 HAP/HSP/HAR 构建）已完全对齐**：parity 26/26 真实 hvigor 逐产物对比全绿（module.json/loader.json 逐字节、cache TS 零差异、modules.abc md5 一致、HAP/HSP/HAR 结构一致；含本轮新增多模块 fixture `multi-module-hap`：entry HAP + source HAR 依赖）。

**A 类缺口（6 项可影响生产）本轮已全部关闭**。剩余差异归为：B. 设计取舍/范围外（N/A，15 项）、C. 可选后端未完成（4 项）、D. 已知记录差异（4 项）、E. 已实现待端到端验证。

---

## A. 可影响生产的小缺口（本轮已全部关闭）

| # | 缺口 | hvigor 行为 | arkdown 现状 | 状态 |
|---|---|---|---|---|
| A1 | **pre-build 17 项 JSON 校验** | `pre-build.ts` 构建前校验 appJson5/moduleJson5/systemTheme/startWindow/form/pages/routerMap/shareConfig/sysCap/mockConfig/insightIntent/targetESVersion 等 17 项 | `PreBuildValidator`（`pre_build.rs`，18 项）：SDK modulecheck 模式校验 + srcEntry/executable 路径存在性 + UI 语法 API 门 + ability 重名 + dynamic-import（runtimeOnly）约束 + OHM URL/autoLazyImport API 门 + mockConfig/shareConfig URI + syscap/insightIntent 模式 + targetESVersion | ✅ |
| A2 | **pre-check-syscap 校验** | `PreCheckSyscap::doTaskAction`：JSON_PROFILE_MISSING（deviceTypes 空 + 无 `src/main/syscap.json`）/ SYSCAP_FIELD_NOT_EMPTY（deviceTypes 空 + `devices.general` 非空）/ DEVICE_TYPE_NOT_INCONSISTENT（`devices.general` 与 deviceTypes 排序后元素不一致，`checkArrayElementIsSame`）；HarmonyOS 目标跳过 | `validate_pre_check_syscap()` 全三码；另补 `validateHybridDeviceTypes`（INCONSISTENT_DEVICE_TYPES / UNEQUAL_MINI_SYSTEM_DEVICE_TYPES——lite 类型被 SDK schema 先行拒绝，与 hvigor 链序一致，实际不可达）。已用 OpenHarmony fixture 逐一触发验证三码 | ✅ |
| A3 | **packing-check 独立内容校验** | `packing-check-util` 包内容检查 | `PackingCheck`（`packing_check.rs`）：SO 文件存在性（ABI 过滤）+ 打包源文件收集，接在 pack 任务后 | ✅ |
| A4 | **do-native-binxo** | OHOS_ENABLE_BINXO 开关驱动 binxo 动作 | `NativeLibCollector::binxo_libs()`：`-DOHOS_ENABLE_HWASAN=ON` + `-DOHOS_ENABLE_BINXO=ON` 时对 process-libs 输出跑 SDK `hms/native/BinXO/binxo`；`nativeLib.excludeSoFromBinXO` 匹配项直接拷贝；工具缺失/失败回退拷贝；缓存 key 含 binxo 目录 hash | ✅ |
| A5 | **onwarn 过滤** | `arkts-pack.js::onRollupWarn(message, code, isFaMode)`：恒过滤 SHIMMED_EXPORT；FA 模式额外过滤 SOURCEMAP_BROKEN（N/A）；其余 `console.warn` 黄色输出 | `Bundler::bundle()` 扫描后遍历 `ModuleSourceScanOutput.warnings`（此前整体丢弃），恒过滤 `SHIMMED_EXPORT`，其余以 kind+message（含模块 id）经 `tracing::warn!` 透出——ESMODULE 下 SOURCEMAP_BROKEN 照常透出，与 hvigor 一致 | ✅ |
| A6 | **sourcemapExcludeSources** | hvigor `sourcemapExcludeSources: true`（map 不含 sourcesContent） | 生产路径 `write_hvigor_source_maps()` 的 `entry_json` 本就不写 `sourcesContent`；与真实 hvigor golden（`tests/fixtures/golden/minimal-hap/sourceMaps.map`）逐字节一致。审计原记录过时 | ✅（原记录过时） |

## B. 设计取舍 / 范围外（N/A，不影响 Stage 模型生产构建）

| 项 | 说明 |
|---|---|
| FA 模型 / JSBUNDLE 全套（legacy-tasks、format cjs、babel、terser-release-JSBUNDLE、sourcemapPathTransform-JSBUNDLE） | CLAUDE.md 明确范围外 |
| unitTest 全套（11 任务）/ ohosTest 全套（6 任务） | session 有 Inventory 模型，CLI 无 test 目标 |
| 设备覆盖率报告链（generate-device-coverage 等） | **插桩已实现**（coverage_instrument.rs，F20）；报告生成未实现 |
| 设备测试链（device-test-hook / onDeviceTest） | 未实现 |
| 预览器管线（previewer-ark-compile / preview-compile-resource 等 10 任务） | `--preview` 已实现资源重编译 + preview_changes.json（M3） |
| Quickfix 链（prepare-quickfix / package-hqf / sign-hqf） | 未实现（assembleDevHqf） |
| fast-package / fast-package-hsp | 未实现（增量快速重打） |
| 远程 HSP 签名（sign-module-remote-hsp / sign-project-remote-hsp） | ✅ 已实现（`remote_hsp_sign.rs`）：有签名配置时签名 `oh_modules/.hsp` 依赖到 `build/cache/<product>/remote_hsp/` + `remoteHspCache.json` |
| validate-sign（ValidateSigning） | keystore 校验在 sign crate 内部，任务边界无 |
| ark-linter（LintArkTS，HAR lint） | 未实现（SDK etsChecker 是另一条线） |
| build-info 任务 | ✅ 已实现（`arkdown build-info` 命令，与 hvigor 逐字段一致） |
| hot/cold reload 增量 ABC patch（buildConfig.json patchConfig、watchWorker） | watch 模式是"跳过资源/Profile 的全模块重打"，无增量 patch（IDE hotReload WebSocket 事件已实现） |
| copyPlugin（isFaMode） | FA 范围外 |
| useAcornLoose（!shouldEmitJs 宽松解析） | OXC 恒宽松解析——等效 |

## C. 可选后端未完成（不影响生产——生产 100% 走 SDK es2abc，与 hvigor 相同）

| 项 | 状态 |
|---|---|
| H2：Rust ABC 优化器全 pass 栈（Cleanup/ConstantPropagation/ValNum/RegAlloc 等） | 🔄 已覆盖 register-frame 压缩/range 操作数/opt-try-catch-func；全栈未完成 |
| H10：`AbcCompiler::compile` 多源 non-merge | 🔄 低层 API 仍单输出 |
| H11/J3：ABC 增量缓存 protoBin parity | 🔄 已实现 upstream 结构（ProgramCache/AbcProtoProgramsCache/Murmur3）；大 fixture 验证待做 |
| arkdown-abc 生产未启用（Auto/Sdk 后端选择恒走 SDK） | 设计使然（CLAUDE.md 规则 3） |

## D. 已知记录差异（已记录于 hvigor-full-gap-analysis.md）

| 项 | 状态 |
|---|---|
| sourceMaps.map import 行映射粒度 | ✅ 已对齐：expandImportPath 挪进 transform（AST 级，map 同步）；含 import/export 文件的行映射与 hvigor 一致 |
| lazy 场景 source map 行映射 | ✅ 已对齐：autoLazyImport 的 `import lazy` 重写随 AST 级展开同机制（map 同步）；`source_emit.rs` 的 mapper 边界规则修正（Method key 锚定/Property 全 span） |
| pkgSdkInfo.json 手动 oh_modules 链接场景 | arkdown 依赖图解析 file: 依赖更完整；真实 ohpm 安装应一致 |
| expandImportPath 本地包范围 | ✅ 已对齐（2026-08-08）：快照 `with_package_roots` 显式传入依赖包根（file: 依赖 canonical 路径不再漏判）、package_root 推断修复（多余 parent 致双重前缀）、decl 在入口的 symbol 展开到入口、远程 HSP/HAR 包（OHM 映射）不展开。multi-module-hap + expandImportPath 的 modules.abc md5 与 hvigor 一致 |
| HmosWorld 大工程 source-layout/DebugInfo goldens | fixture 软链断裂不可用；验证类工作 |

## E. widget 端到端验证（2026-08-08 完成）

标准双模块 widget fixture（`tests/fixtures/widget-hap/`：entry HAP + widget HSP
`formExtensionModule` 双向映射，API 20）已构造并通过真实 hvigor parity
（`widget_hap_hvigor_parity_report`，parity 套件 27/27）：

| 产物 | 状态 |
|---|---|
| `widgets.abc` | ✅ md5 一致 |
| `widget_loader.json`（含 pkgName2OtherCompileFiles） | ✅ 一致 |
| `filesInfo.txt` / `modules.abc`（widget 页面进主编译 es2abc 输入） | ✅ 一致（修复：form sources 仅进 widget 模块主编译，`needSubmitArkTsWidget` 语义） |
| loader.json `compileEntry` | ✅ 一致（非全量编译仅 ResourceTable.ts，form sources 不入 compileEntry） |
| 归档 `module.json` 字段顺序 | ✅ 一致（`formExtensionModule` 在 dependencies 前） |
| `component_collection.json` | ✅ 一致 |
| `etsFortgz/*.d.ets` 声明内容 | ✅ md5 一致（自研 `declare struct` 生成：装饰器分行、成员签名、方法默认 `: void`、CRLF；OXC IsolatedDeclarations 不处理 ArkUI struct，走自研路径） |
| `intermediates/res/ark_module.json` | ✅ 一致（`merge_form_widget_abilities`：form widget 模块合并 formExtensionModule 的 form abilities，name/type/metadata 精简字段） |
| sourceMaps.map 行映射 | 🔄 收敛至最后 1 类（2026-08-08）：操作符（BinaryExpression `+` 等）与分号已对齐（mapper 的 Operator 边界 + Property end 映射）；src 位置/行归属全对齐。剩余：合成调用（组件 pop 的闭合映射——hvigor 映射到源组件块 `}`；块闭合 `}`——TS lastSourcePosition 内部机制；constructor `}` → src 0:45 fallback）。需 mapper 的组件闭合边界扩展（源组件 AST 访问 + pop LIFO 匹配）。普通 class 文件逐字节对齐 |

修复项：`WidgetCompileArktsPass::plan()` 不再跳过 Shared 模块、form sources 不入
compileEntry（loader_json.rs）、widget 模块主编译 es2abc 输入含 form sources
（CompileContextInfoWriter form_widget_module）、打包重排含 formExtensionModule。

## F. 本轮（2026-08-08 晚）对齐增量

- **A2** `validate_pre_check_syscap()` + `validateHybridDeviceTypes()` 落入 `PreBuildValidator`（`pre_build.rs`），OpenHarmony fixture 逐一触发三码 + INCONSISTENT_DEVICE_TYPES 验证；UNEQUAL_MINI_SYSTEM_DEVICE_TYPES 与 hvigor 一致不可达（lite 类型被 modulecheck schema 先行拒绝）
- **A5** rolldown 扫描警告不再整体丢弃：`Bundler::bundle()` 遍历 `scan.warnings`，恒过滤 `SHIMMED_EXPORT`（`onRollupWarn` ESMODULE 语义），其余经 `tracing::warn!` 透出
- **build-info** 命令（`arkdown build-info`）：hvigor `BuildInfo` 任务完整复刻——project/module 投影、`-p json=`/`-p buildOption=`/`-p module=` 语义、`resolve_display_value()`（保留 name/copyFrom/debuggable、原始相对路径、buildOptionSet 占位合成、ohosTest target→default 绑定），与真实 hvigor 逐字段一致 + 3 个 oracle 测试
- **远程 HSP 签名**（`remote_hsp_sign.rs`）：有签名配置时扫描 `oh_modules/.hsp` 依赖过滤签名，输出 `build/cache/<product>/remote_hsp/<dir>/*-signed.hsp` + `remoteHspCache.json`（hvigor `{name, material}` 形状），接入 app 流程 SignRemoteHsp 生命周期；`--target app` CLI 路由（此前仅 napi 路径）
- **widget 端到端**（E 节）：双模块 fixture + parity，修复 4 处（Shared 跳过、form sources 入 compileEntry、widget 主编译 es2abc 输入、打包字段顺序）
- **expandImportPath 对齐**：快照 package_roots 显式化 + 包根推断修复 + 入口 decl 展开 + 远程 HSP/HAR 跳过（modules.abc md5 与 hvigor 一致）
- **widget d.ets 声明**：自研 `declare struct` 生成（OXC IsolatedDeclarations 不处理 ArkUI struct），md5 与 hvigor 一致（CRLF）
- **ark_module.json**：form widget 模块合并 formExtensionModule 的 form abilities（name/type/metadata），与 hvigor 一致
- 新 parity 测试 `multi_module_hap_hvigor_parity_report` + `widget_hap_hvigor_parity_report`，套件 27/27 全绿
- 审计修正：A6（sourcesContent）生产路径本就不含 sourcesContent，golden 一致——原记录过时

## 审计方法备注

- hvigor-arkts-compose 6.24.3 vs 6.24.2：唯一行为差异是 widgetCardPlugin（数据面已覆盖）——其余完全一致
- F17 文档的 "Event-emitter call-pattern rewrites not implemented" 描述过时——SDK 本身无 call-pattern 重写，arkdown 的 import 路径重写（@ohos:emitter → @ohos.events.emitter）已覆盖实际行为
- 多模块（entry + HAR 依赖）在 module.json 字段顺序、loader.json、cache TS、abc 层面逐字节一致（本轮修复 module.json app/module 段字段顺序 + loader.json 尾部换行）
