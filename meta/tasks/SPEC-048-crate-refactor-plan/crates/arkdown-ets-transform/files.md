# File API Plan: crates/arkdown-ets-transform

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

本 crate 是 ArkTS/ArkUI transform core。它从当前 `arkdown-plugin-ets-transform/src/*` 迁出所有非 rolldown adapter 的逻辑，并对齐 SDK `compile_plugin.js` 中 `visualTransform -> etsTransform -> apiTransform` 的语义。更细的上游流程见 [../../arkts-transform-audit.md](../../arkts-transform-audit.md)。

核心要求：

- `.ets` 使用 OXC `SourceType::ets()`，保留 `StructStatement` 和 ArkUI AST，不再靠文本猜测。
- `.ts` 使用 OXC TS parser；`.js` 使用 OXC JS parser；`.d.ts/.d.ets` 必须被 classifier 和 hook 双重排除。
- transform crate 不能读取 SDK、profile、package、resource 或 visual 文件；这些输入必须来自 `ModuleCompileContext`、`SdkSnapshot`、`SourceStore`、`ResourceInventory`。
- 输出不能只是 JS 字符串；必须带 `TransformFacts`，用于 ABC、profile、package、cache。
- raw string replace 只允许作为上游 regex 兼容的 span patch；主路径必须是 OXC AST/IR/codegen。
- OXC 只替代 parse/AST/codegen 主路径；上游 TypeScript Program、compiler host、ArkTS checker、Arkoala、const enum invalidation 这些语义必须以独立结构体补齐。
- `shouldEmitJs`、`UseTsHar`、bytecode HAR、singleFileEmit、coverage、preview/watch/hot reload 都必须在 `EtsTransformPlan` 中显式建模，不能由 transform hook 根据文件扩展名临时决定。

## 源码类型和输出合同

| 类型 | 输入来源 | 输出 |
| --- | --- | --- |
| `.ets` | `compileEntry/otherCompileFiles/card/workers` | `TransformOutput::Js` 或 `TransformOutput::TsForAbc`，带 ArkUI/component/API/resource facts |
| `.ts` | 同上 | `TransformOutput::Js` 或 `TransformOutput::TsForAbc`，不跑 `.ets` 专属 validation |
| `.js` | loader input 或 bundle graph | `TransformOutput::Js` 或 passthrough，带 API/libso/lazy import facts |
| `.json` | bundle graph | 不由本 crate transform；只在 `arkdown-ts2abc` 作为 module source |
| `.d.ts/.d.ets` | `declarationEntry`、dependency metadata | `TransformOutput::DeclarationOnly`，不进入 rolldown transform hook |
| `.visual` | `VisualFileSnapshot` | 不是 module；作为 `.ets` visual prepass 输入 |

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `EtsTransformer`, `EtsTransformPlan`, `TransformOutput`, `TransformFacts`, `SourceClassifier` | `EtsTransformer::new(plan).transform_source(source)`, `SourceClassifier::classify(path)` | current `lib.rs` facade narrowed | SDK `visualTransform/etsTransform/apiTransform` typed facade |
| `plan.rs` | `EtsTransformPlan`, `TransformFilePlan`, `CompileEntryOrigin`, `EntryRole`, `TransformPipelineKind`, `ShouldEmitJsPolicy`, `TransformSideOutputs` | `from_module_context`, `file_plan_for`, `should_emit_js`, `side_output_paths` | new from loader/profile/session context | `GenerateLoaderJson`, SDK `init_config.js`, `getShouldEmitJs` |
| `options.rs` | `EtsTransformOptions`, `UiTransformOptions`, `ApiTransformOptions`, `CheckerOptions`, `VisualOptions`, `DeclarationEmitOptions` | `from_plan`, `validate`, `for_file_kind`, `is_release`, `supports_partial_update` | scattered plugin options | `ArkCompile.initDefaultArkCompileConfig`, SDK `main.js` |
| `version.rs` | `VersionGate`, `PartialUpdateMode`, `ArkTsVersion`, `CompatibleApi`, `ArkuiBackend` | `from_profile_metadata`, `partial_update_enabled`, `optimize_component_enabled`, `use_ts_har`, `needs_js_api10_compat` | new | `main.js::partialUpdateController`, metadata `UseTsHar/ArkTSVersion/ArkoalaPlugin` |
| `source.rs` | `SourceText`, `SourceFileKind`, `SourceClassifier`, `SourceIdentity`, `SourceEditSet`, `LineIndex` | `classify`, `is_declaration`, `is_ets_transform_input`, `apply_edits`, `line_index` | `source_text.rs`, `source_edit.rs` | SDK filters `/(?<!\.d)\.(ets|ts)$/`, `filterCrossplatform` |
| `loader_inputs.rs` | `CompileEntrySet`, `CompileEntry`, `DeclarationInput`, `OtherCompileFile`, `WorkerEntry`, `CardEntry` | `from_loader_json`, `dedupe_realpaths`, `reject_declarations_for_transform`, `entry_origin` | new from current loader_json consumption | `GenerateLoaderJson::processCodeFiles`, `init_config.js` |
| `pipeline.rs` | `EtsTransformer`, `TransformContext`, `TransformOutput`, `TransformFacts`, `TransformStageTiming` | `new`, `transform`, `run_visual`, `run_checker`, `run_arkui`, `run_api`, `emit` | `transform/mod.rs`, `api_transform/*` | SDK `visualTransform/etsTransform/apiTransform` |
| `diagnostics.rs` | `EtsTransformError`, `DiagnosticSeverity`, `EtsDiagnostic`, `DiagnosticCode` | `from_oxc`, `from_upstream_rule`, `fatal`, `with_span`, `merge` | `error.rs`, checker errors | SDK checker diagnostics and hvigor error codes |
| `sdk_snapshot.rs` | `SdkTransformTables`, `SystemApiTable`, `KitImportTable`, `ComponentMetadata`, `SysResourceTable` | `from_session_snapshot`, `system_api`, `kit_map`, `component_meta`, `resource_refs` | `kit_map.rs`, `sys_resource.rs` | SDK data/config read by ets-loader |
| `oxc_parse.rs` | `OxcSourceParser`, `ParsedSource`, `ParsedEts`, `ParsedTs`, `ParsedJs`, `ParseMode` | `OxcSourceParser::parse_ets()`, `OxcSourceParser::parse_ts()`, `OxcSourceParser::parse_js()`, `OxcSourceParser::parse_for_kind()` | current OXC parser calls | TypeScript `createSourceFile` / `transpileModule` replacement |
| `semantic.rs` | `SemanticProgramBridge`, `SemanticProgramPlan`, `SemanticSourceFile`, `CompilerHostSnapshot`, `ConstEnumDependencySet` | `SemanticProgramBridge::new`, `prepare_program`, `source_file`, `check_relate_to_const_enum`, `semantic_diagnostics`, `emit_semantic_facts` | new from TypeScript compiler-host behavior | `rollup-plugin-ets-typescript.js::getCompilerHost/getArkoalaTsProgram/checkRelateToConstEnum`, TypeScript Program semantic path |
| `arkoala.rs` | `ArkoalaTransformGate`, `ArkoalaProgramInput`, `ArkoalaPluginMode`, `ArkoalaDiagnostic` | `ArkoalaTransformGate::from_options`, `requires_arkoala_program`, `prepare_arkoala_inputs`, `emit_arkoala_diagnostics` | new | `rollup-plugin-ets-typescript.js::getArkoalaTsProgram`, `arkoala-plugin.js`, metadata `ArkoalaPlugin` |
| `transform_cache.rs` | `TransformCacheDecision`, `TransformCacheKey`, `TransformChildDependencySet`, `IncrementalTransformState`, `RawFileResourceDependency` | `from_plan`, `should_disable_cache`, `record_child_dependency`, `record_raw_file_resource`, `invalidate_for_const_enum`, `cache_key_material` | current plugin cache/child dependency gaps | `disableNonEntryFileCache`, `judgeCacheShouldDisabled`, `setIncrementalFileInHar`, `checkRelateToConstEnum`, SDK rawFileResource collection |
| `emit_policy.rs` | `ShouldEmitJsPolicy`, `EmitJsDecision`, `TsForAbcPolicy`, `DeclarationOnlyPolicy` | `ShouldEmitJsPolicy::from_plan`, `get_should_emit_js`, `emit_ts_for_abc`, `declaration_only`, `single_file_emit_enabled` | split from plan/options | `rollup-plugin-ets-typescript.js::getShouldEmitJs/shouldEmitJsFlagById`, HAR `UseTsHar`, bytecode HAR `processTs` |
| `visual.rs` | `VisualTransform`, `VisualFileSnapshot`, `VisualInjection`, `VisualSourceMap` | `find_for_source`, `transform`, `inject_imports`, `inject_build`, `inject_about_to_appear` | new from SDK behavior | `rollup-plugin-visual.js`, `process_visual.js` |
| `checker.rs` | `EtsChecker`, `CheckerRule`, `CheckerReport`, `ArkTsLinterMode`, `BuilderCheckMode` | `check_file`, `validate_ets`, `run_linter_rules`, `emit_report` | `checker/*` | `rollup-plugin-ets-checker.js`, `validate_ui_syntax.js`, `do_arkTS_linter.js` |
| `ui_syntax.rs` | `UiSyntaxPass`, `UiSyntaxFacts`, `ArkuiSyntaxNodeKind` | `preprocess_ets`, `validate`, `lower` | `transform/syntax.rs`, `process_ui_syntax` parity | SDK `preprocessExtend`, `preprocessNewExtend`, `processUISyntax` |
| `decorator.rs` | `DecoratorInfo`, `DecoratorKind`, `DecoratorSet` | `collect_decorators`, `strip_runtime_only`, `validate_decorator_order` | current `decorator.rs` | SDK `process_decorator.js`, `validate_ui_syntax` |
| `component/mod.rs` | exports component modules | `analyze_component`, `lower_component`, `emit_component` | transform component files | SDK `process_component_*` |
| `component/analysis.rs` | `ComponentInfo`, `ComponentFacts`, `CustomComponentInfo`, `ImportedComponentFacts`, `BuilderMethodInfo`, `StylesMethodInfo` | `collect_components`, `collect_custom_component_names`, `collect_builders`, `collect_styles` | `component_analysis.rs`, `component.rs` | SDK `component_map.js`, `external_component_map.js` |
| `component/fields.rs` | `ComponentField`, `StateField`, `ParamField`, `RegularField`, `ComponentPropKind`, `WatchInfo` | `collect_fields`, `state_var_backend`, `param_fields`, `watch_handlers` | `component_field.rs`, `component.rs` | SDK `process_component_member.js` |
| `component/build_fn.rs` | `BuildFunctionLowerer`, `BuildFunctionFacts`, `ArkuiCallKind`, `ComponentCreationBlock` | `lower_build_fn`, `lower_builder_method`, `lower_styles_method`, `lower_custom_component`, `lower_builtin_component` | `transform/build_fn/*` | SDK `process_component_build.js`, `process_custom_component.js` |
| `component/constructor.rs` | `ConstructorEmitter`, `ConstructorParams`, `PartialUpdateConstructorMode` | `emit_constructor`, `emit_super_call`, `emit_update_with_value_params` | current ViewPU emit helpers | SDK `process_component_constructor.js` |
| `component/class.rs` | `ComponentClassEmitter`, `LifecycleMethod`, `EntryRegistration` | `emit_class`, `emit_lifecycle`, `emit_entry_registration` | `component.rs`, `entry.rs` | SDK ViewPU/ViewV2 class generation |
| `viewpu/mod.rs` | `ViewPuTransform`, `ViewPuFacts`, `ViewPuMode` | `analyze`, `emit`, `finalize` | `viewpu_emit.rs`, `viewpu_finalize.rs` | SDK partial update ViewPU output |
| `viewpu/emit.rs` | `ViewPuEmitter`, `ViewPuEmitState` | `emit_component`, `emit_render`, `emit_update_state_vars`, `emit_purge`, `emit_about_to_be_deleted` | `viewpu_emit.rs` | SDK generated ViewPU methods |
| `viewpu/finalize.rs` | `ViewPuFinalizer`, `HelperImportSet` | `inject_imports`, `dedupe_helpers`, `finalize_source` | `viewpu_finalize.rs` | final SDK output shape |
| `viewpu/source_map.rs` | `ViewPuSourceMapBuilder`, `GeneratedSpan`, `OriginalSpan` | `map_generated_span`, `merge_visual_map`, `finish` | `viewpu_source_map.rs` | SDK source map preservation |
| `viewv2.rs` | `ViewV2Transform`, `StructV2Facts`, `ReuseRecycleFacts` | `lower_struct_v2`, `collect_reuse`, `collect_recycle` | current partial V2 logic | SDK `process_struct_componentV2.js` |
| `sendable.rs` | `SendableTransform`, `SendableFacts` | `analyze`, `lower`, `validate` | new/current sendable gaps | SDK `process_sendable.js` |
| `typescript_emit.rs` | `TypescriptEmitter`, `TsEmitOptions`, `TsForAbc` | `emit_js`, `emit_ts_for_abc`, `emit_declaration_stub` | current `typescript_emit.rs` | TS Program emit / transformNodes replacement |
| `declaration.rs` | `DeclarationEmitter`, `DeclarationEmitOutput`, `IncrementalHarFileMap` | `emit_declarations`, `emit_component_declarations`, `map_incremental_har_files` | `declaration_emit.rs`, `typescript_emit.rs` | `setIncrementalFileInHar`, HAR declaration output |
| `api_transform/mod.rs` | `ApiTransform`, `ApiTransformFacts`, `SystemModuleImport`, `UseOsFile`, `KitModuleReplacementSet` | `rewrite_api_imports`, `rewrite_libso_imports`, `collect_component_modules`, `compare_kit_modules`, `replace_kit_modules`, `emit_package_facts` | `api_transform/*` | SDK `apiTransform`, `rollup-plugin-system-api.js::collectKitModules/compareKitModules/replaceKitModules` |
| `api_transform/system_api.rs` | `SystemApiRewriter`, `SystemApiImport`, `SdkPrefix` | `rewrite_import_decl`, `rewrite_import_equals`, `rewrite_require_call`, `check_module_exist` | `api_transform` | `processSystemApi`, `processSystemApiAndLibso` |
| `api_transform/libso.rs` | `LibSoRewriter`, `NativeImport`, `NativeRequireTarget` | `rewrite_import`, `record_use_os_file`, `to_package_native_ref` | current libso handling gaps | SDK `processLibso` |
| `kit_import.rs` | `KitImportTransform`, `KitImportFacts`, `KeepTsDecision`, `KitModuleComparer` | `process_kit_imports`, `check_has_keep_ts`, `collect_kit_modules`, `compare_kit_modules`, `rewrite_to_kit_imports` | `kit_map.rs`, `api_transform/version_table.rs` | SDK `process_kit_import.js`, `rollup-plugin-system-api.js::compareKitModules/replaceKitModules` |
| `import_path.rs` | `ImportPathExpander`, `ImportRewrite`, `OhmSpecifier` | `expand_all_import_paths`, `rewrite_imports`, `record_rewrites` | `module_rewrite.rs` | SDK `import_path_expand.js` |
| `lazy_import.rs` | `LazyImportTransform`, `LazyImportFacts` | `process_lazy_import`, `check_reexport`, `emit_diagnostics` | new from ts2abc handoff | SDK `process_lazy_import.js` |
| `resource_transform.rs` | `ResourceTransform`, `ResourceReference`, `ResourceCall`, `RawfileReference` | `rewrite_resource_refs`, `collect_resource_ids`, `rewrite_rawfile_refs` | current `resource_transform.rs` | SDK `processResourceData` behavior |
| `module_rewrite.rs` | `ModuleRewriter`, `ModuleRewriteFacts` | `rewrite_imports`, `rewrite_exports`, `emit_module_wrapper` | `module_rewrite.rs`, `module_emit.rs` | SDK module rewrite/package helpers |
| `global_rewrite.rs` | `GlobalRewriter`, `GlobalRewriteFacts` | `rewrite_global_refs`, `inject_helpers` | `global_rewrite.rs` | SDK global helper rewrite |
| `obfuscation.rs` | `ReservedNameCollector`, `ObfuscationFacts` | `collect_reserved_names`, `record_file_path_obfuscation_inputs` | current scattered obfuscation hooks | SDK `collectReservedNameForObf` |
| `source_map.rs` | `EtsSourceMap`, `SourceMapBuilder`, `SourceMapSegment`, `SourceMapMergeInput` | `new`, `add_mapping`, `merge`, `to_json`, `from_oxc` | plugin `source_map.rs` | SDK source map transforms |
| `facts.rs` | `TransformFacts`, `ComponentCollection`, `ModuleCollection`, `UseOsFiles`, `IntentFacts`, `ResourceFacts` | `merge`, `is_empty`, `into_side_outputs` | new | SDK global maps/sets replacement |
| `tests/upstream_parity.rs` | test helpers | `fixture_from_sdk`, `assert_transform_matches_snapshot`, `assert_branch_fact` | new | parity suite for SDK branches |

## 迁移映射

| 当前文件/目录 | 目标文件 |
| --- | --- |
| `transform/mod.rs` | `pipeline.rs`, `ui_syntax.rs`, `component/*`, `viewpu/*` |
| `transform/build_fn/*` | `component/build_fn.rs` |
| `transform/component*.rs` | `component/analysis.rs`, `component/fields.rs`, `component/class.rs` |
| `transform/viewpu_emit.rs` | `viewpu/emit.rs` |
| `transform/viewpu_finalize.rs` | `viewpu/finalize.rs` |
| `transform/viewpu_source_map.rs` | `viewpu/source_map.rs` |
| `transform/decorator.rs` | `decorator.rs` |
| `transform/entry.rs` | `component/class.rs` |
| `declaration_emit.rs` | `declaration.rs` |
| `api_transform/*` | `api_transform/mod.rs`, `system_api.rs`, `libso.rs` |
| `kit_map.rs`, `sys_resource.rs` | `sdk_snapshot.rs`，必要时保留 loader utilities |
| `resource_transform.rs` | `resource_transform.rs` |
| `module_rewrite.rs`, `module_emit.rs` | `module_rewrite.rs` |
| `global_rewrite.rs` | `global_rewrite.rs` |
| `source_text.rs`, `source_edit.rs` | `source.rs` |
| `source_map.rs` | `source_map.rs` |
| `checker/*` | `checker.rs` + future `checker/rules.rs` if rules expand |

## 必补测试

- `.ets` JSBUNDLE：`preprocessExtend/preprocessNewExtend/validateUISyntax/processUISyntax` parity。
- `.ets` ESMODULE：`shouldEmitJs=true` 和 `shouldEmitJs=false` 两条路径。
- `.ts` ESMODULE：不跑 `validateEts`，但 import path、kit import、obfuscation facts 生效。
- `.js`：不进入 ETS transform；crossplatform API transform 允许 `.js`。
- `.d.ts/.d.ets`：loader 和 classifier 均排除 transform。
- `.visual`：空 `build()` 注入成功，非空 `build()` 诊断。
- API <= 10：JS module source 交给 `arkdown-ts2abc::js_compat`。
- HAR `UseTsHar`、bytecode HAR、singleFileEmit、coverage、widgetCompile、preview/watch/hot reload 分支。
