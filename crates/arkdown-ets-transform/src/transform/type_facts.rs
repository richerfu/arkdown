use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use arkdown_path::{canonicalize_or_owned, lexical_normalize};
use oxc_span::Span;
use rustc_hash::FxHashMap;

use super::CrossplatformSourceCollection;

#[derive(Debug, Clone, Default)]
pub struct ArktsTypeFactsSnapshot {
    files: Arc<FxHashMap<PathBuf, ArktsFileTypeFacts>>,
    lookup_paths: Arc<FxHashMap<PathBuf, PathBuf>>,
}

impl ArktsTypeFactsSnapshot {
    pub fn new(files: FxHashMap<PathBuf, ArktsFileTypeFacts>) -> Self {
        let mut indexed_files = FxHashMap::default();
        let mut lookup_paths = FxHashMap::default();
        for (path, facts) in files {
            let lexical = lexical_normalize(&path);
            let canonical = canonicalize_or_owned(&lexical);
            indexed_files.entry(canonical.clone()).or_insert(facts);
            lookup_paths.insert(path, canonical.clone());
            lookup_paths.insert(lexical, canonical.clone());
            lookup_paths.insert(canonical.clone(), canonical);
        }
        Self {
            files: Arc::new(indexed_files),
            lookup_paths: Arc::new(lookup_paths),
        }
    }

    pub(crate) fn file_facts(&self, source_id: &str) -> Option<&ArktsFileTypeFacts> {
        let source_id = source_id
            .split_once('?')
            .map_or(source_id, |(path, _)| path);
        let path = Path::new(source_id);
        self.lookup_paths
            .get(path)
            .or_else(|| self.lookup_paths.get(&lexical_normalize(path)))
            .and_then(|indexed| self.files.get(indexed))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArktsFileTypeFacts {
    pub declarations: Vec<SpannedAnnotationInfo>,
    pub uses: Vec<SpannedAnnotationUse>,
    pub imports: Vec<SpannedAnnotationImport>,
    pub properties: Vec<SpannedPropertyTypeFacts>,
    pub builder_accesses: Vec<SpannedTypeIdentity>,
    pub member_accesses: Vec<SpannedTypeIdentity>,
    /// SDK declaration dependencies collected at the source checker callback
    /// where TypeScript has already resolved overloads, aliases, and return
    /// types. OXC only merges the resulting artifact inventory.
    pub crossplatform_dependencies: CrossplatformSourceCollection,
}

impl ArktsFileTypeFacts {
    pub(crate) fn declaration(&self, span: Span) -> Option<&AnnotationInfo> {
        Self::matching(&self.declarations, span).map(|fact| &fact.info)
    }

    pub(crate) fn annotation_use(&self, span: Span) -> Option<&AnnotationUse> {
        Self::matching(&self.uses, span).map(|fact| &fact.annotation)
    }

    pub(crate) fn annotation_import(&self, span: Span) -> Option<AnnotationImportDisposition> {
        Self::matching(&self.imports, span).map(|fact| fact.disposition)
    }

    pub(crate) fn property(&self, name_span: Span) -> Option<&PropertyTypeFacts> {
        Self::matching(&self.properties, name_span).map(|fact| &fact.facts)
    }

    pub(crate) fn builder_receiver(&self, span: Span) -> Option<&ResolvedTypeIdentity> {
        Self::matching(&self.builder_accesses, span).map(|fact| &fact.identity)
    }

    /// Source owner: ets2bundle
    /// `process_component_build.ts::isRegularAttrNode`. The TypeScript
    /// checker fallback classifies the complete `Identifier.Member` access,
    /// not its receiver.
    pub(crate) fn is_enum_member_access(&self, span: Span) -> bool {
        Self::matching(&self.member_accesses, span).is_some_and(|fact| fact.identity.is_enum)
    }

    /// Source owner: ets2bundle
    /// `process_component_build.ts::{isWrappedBuilder,isMutableBuilder}`.
    /// Both helpers require a scalar non-null receiver type whose symbol name
    /// exactly matches the runtime wrapper class; WrappedBuilder additionally
    /// requires API 11 or newer.
    pub(crate) fn builder_access_kind(
        &self,
        span: Span,
        api_version: u32,
    ) -> Option<BuilderAccessKind> {
        let receiver = self.builder_receiver(span)?;
        if receiver.non_nullable_symbol_is("MutableBuilder") {
            return Some(BuilderAccessKind::Mutable);
        }
        (api_version >= 11 && receiver.non_nullable_symbol_is("WrappedBuilder"))
            .then_some(BuilderAccessKind::Wrapped)
    }

    fn matching<T: HasFactSpan>(facts: &[T], span: Span) -> Option<&T> {
        facts
            .iter()
            .find(|fact| fact.fact_span().end == span.end)
            .or_else(|| {
                facts.iter().find(|fact| {
                    let fact = fact.fact_span();
                    fact.start <= span.start && fact.end >= span.end
                        || span.start <= fact.start && span.end >= fact.end
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuilderAccessKind {
    Wrapped,
    Mutable,
}

#[derive(Debug, Clone)]
pub struct SpannedPropertyTypeFacts {
    pub span: Span,
    pub facts: PropertyTypeFacts,
}

impl HasFactSpan for SpannedPropertyTypeFacts {
    fn fact_span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub struct PropertyTypeFacts {
    pub declared_type: Option<ResolvedTypeIdentity>,
}

#[derive(Debug, Clone)]
pub struct SpannedTypeIdentity {
    pub span: Span,
    pub identity: ResolvedTypeIdentity,
}

impl HasFactSpan for SpannedTypeIdentity {
    fn fact_span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedTypeIdentity {
    pub is_nullable: bool,
    pub is_union: bool,
    pub is_enum: bool,
    pub is_basic: bool,
    pub is_observed_v2: bool,
    pub is_function: bool,
    pub symbol_name: String,
    pub types: Vec<ResolvedTypeIdentity>,
}

impl ResolvedTypeIdentity {
    fn non_nullable(&self) -> Vec<&Self> {
        if !self.is_union || self.types.is_empty() {
            return vec![self];
        }
        self.types
            .iter()
            .filter(|identity| !identity.is_nullable)
            .collect()
    }

    /// Exact value-side result of ets2bundle
    /// `process_component_member.ts::getDeclarationType`. The caller handles
    /// its initial syntax-only primitive/enum fast path before consulting this.
    pub(crate) fn is_simple_declaration_type(&self) -> bool {
        let types = self.non_nullable();
        if types.len() == 1 {
            let identity = types[0];
            return identity.is_enum
                || !identity.types.is_empty() && identity.types.iter().all(|item| item.is_basic);
        }
        types.iter().all(|identity| {
            identity.is_basic
                || !identity.types.is_empty() && identity.types.iter().all(|item| item.is_basic)
        })
    }

    pub(crate) fn non_nullable_symbol_is(&self, expected: &str) -> bool {
        let types = self.non_nullable();
        types.len() == 1 && types[0].symbol_name == expected
    }

    /// Source owner: ets2bundle
    /// `process_component_member.ts::{checkObjectLinkType,checkTypes}`.
    /// Nullable union members are removed first; every remaining resolved
    /// constituent must be a non-basic, non-ObservedV2, non-Function type.
    pub(crate) fn is_valid_object_link_type(&self) -> bool {
        self.non_nullable()
            .into_iter()
            .all(Self::is_valid_object_link_constituent)
    }

    fn is_valid_object_link_constituent(&self) -> bool {
        if !self.types.is_empty() {
            return self.types.iter().all(|identity| {
                !identity.is_basic && !identity.is_observed_v2 && !identity.is_function
            });
        }
        !self.is_basic && !self.is_observed_v2 && !self.is_function
    }
}

trait HasFactSpan {
    fn fact_span(&self) -> Span;
}

#[derive(Debug, Clone)]
pub struct SpannedAnnotationInfo {
    pub span: Span,
    pub info: AnnotationInfo,
}

impl HasFactSpan for SpannedAnnotationInfo {
    fn fact_span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub struct SpannedAnnotationUse {
    pub span: Span,
    pub annotation: AnnotationUse,
}

impl HasFactSpan for SpannedAnnotationUse {
    fn fact_span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub struct SpannedAnnotationImport {
    pub span: Span,
    pub disposition: AnnotationImportDisposition,
}

impl HasFactSpan for SpannedAnnotationImport {
    fn fact_span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub struct AnnotationUse {
    pub runtime_retained: bool,
    pub info: AnnotationInfo,
}

#[derive(Debug, Clone)]
pub struct AnnotationInfo {
    pub source_retention: bool,
    pub properties: Vec<AnnotationProperty>,
}

#[derive(Debug, Clone)]
pub struct AnnotationProperty {
    pub name: String,
    pub name_text: String,
    pub initializer: Option<AnnotationConstant>,
    pub argument: Option<AnnotationConstant>,
    pub array_depth: usize,
    pub enum_first_value: Option<AnnotationConstant>,
    pub type_text: String,
    pub element_type_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationConstant {
    Number(String),
    String(String),
    Boolean(bool),
    Array(Vec<AnnotationConstant>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationImportDisposition {
    Rename,
    Remove,
}
