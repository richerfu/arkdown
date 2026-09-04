//! OXC-based ArkTS/ArkUI transform core (SPEC-048).
//!
//! This crate owns the ETS transform core, migrated from
//! `arkdown-plugin-ets-transform` (the rolldown adapter layer).
//!
//! It implements the transform stages currently wired into the Rolldown
//! adapter:
//!
//! | Stage        | SDK equivalent        | What it does                                    |
//! | ------------ | --------------------- | ----------------------------------------------- |
//! | `etsTransform` | `etsTransform`      | Desugars ArkUI decorators and struct components |
//! | `apiTransform` | `apiTransform`      | Rewrites OHOS API imports by target SDK version |
//!
//! ## Key constraints
//!
//! - CompileArkTS `.ets` and runtime `.ts` files use OXC `SourceType::ets()`
//!   by default; explicit ArkTS 1.2 plans use `SourceType::ets_static()` for
//!   both extensions.
//! - `.js` uses OXC JS parser.
//! - `.d.ts`/`.d.ets` are excluded by both `SourceClassifier` and the pipeline.
//! - Transform crate does not read SDK, profile, package, or resource files directly.

mod api_transform;
mod checker;
pub mod declaration_merge;
pub mod error;
mod kit_map;
mod lazy_import;
mod module_emit;
mod module_name;
mod module_rewrite;
mod resource_transform;
mod rolldown_plan;
mod source;
mod source_map;
mod sys_resource;
pub mod transform;
mod typescript_emit;
mod visual;
mod visual_codegen;

// Narrow re-exports per spec (lib.rs only re-exports, no new business functions).
pub use checker::{ArktsSourceChecker, EtsCheckReport, EtsChecker};
pub use module_rewrite::{ModuleAnalyzerKind, StaticModuleSpecifierRewrite};
pub use resource_transform::{
    ResourceModuleReference, ResourceReference, ResourceTransformContext,
};
pub use rolldown_plan::{
    RolldownEtsTransformOutput, RolldownEtsTransformPlan, RolldownEtsTransformer,
    TransformEntrySourceId,
};
pub use source::{
    CompileArktsSourceType, SourceClassifier, TransformResolvedStaticModule,
    TransformSourceSnapshot,
};
pub use sys_resource::SystemResourceReferenceLoader;
pub use transform::{
    ArktsTransformer, ArkuiComponentNameCollector, CheckerMode, CrossplatformSourceCollection,
    InsightIntentDeclaration, InsightIntentEntityDeclaration, InsightIntentSchemaCatalog,
    InsightIntentSourceMetadata, NavigationRoute, NavigationRouteMap, SystemApiCheckOptions,
};
pub use typescript_emit::TypeScriptDirectiveFlags;
pub use visual::VisualGeneratedSource;
pub use visual_codegen::{VisualCodeGenerator, VisualCodegenError};

pub(crate) use module_name::ModuleExportNameText;
