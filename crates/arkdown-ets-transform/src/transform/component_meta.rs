use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    fs,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, Program, Statement, StructElement, StructStatement,
    TSEnumDeclaration,
};
use oxc_ast_visit::Visit;
use oxc_parser::{ArkTsAttributeCallback, ArkTsOptions, Parser, ParserReturn};
use oxc_span::SourceType;
use serde::Deserialize;
use serde_json::Value;

use crate::source::{CompileArktsSourceType, SourceClassifier, SourceFileKind};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};

mod sdk_semantics;

use sdk_semantics::SdkArkUiSemanticSnapshot;

#[derive(Debug, Clone, Default)]
pub struct ComponentMetadata {
    arkts_parser: ArkTsParserConfig,
    atomic_components: FxHashSet<String>,
    need_pop_components: FxHashSet<String>,
    bind_object_properties: FxHashMap<String, FxHashSet<String>>,
    create_bind_components: FxHashSet<String>,
    custom_builder_properties_without_key: FxHashSet<String>,
    style_double_dollar_attrs: FxHashSet<String>,
    component_double_dollar_properties: FxHashMap<String, FxHashSet<String>>,
    bind_popup_attrs: FxHashSet<String>,
    bind_drag_attrs: FxHashSet<String>,
    gesture_attr_names: FxHashSet<String>,
    gesture_type_names: FxHashSet<String>,
    enum_names: FxHashSet<String>,
    sdk_components: FxHashMap<String, FxHashMap<String, SdkComponentInfo>>,
    sdk_builder_functions: FxHashMap<String, FxHashSet<String>>,
    sdk_enum_exports: FxHashMap<String, FxHashSet<String>>,
    sdk_namespace_enum_exports: FxHashMap<String, FxHashMap<String, FxHashSet<String>>>,
    runtime_names: ArkuiRuntimeNames,
}

impl ComponentMetadata {
    pub fn from_ets_loader_path(ets_loader_path: &Path) -> anyhow::Result<Self> {
        Self::from_ets_loader_path_with_external_declaration_roots(ets_loader_path, &[])
    }

    /// Builds the semantic snapshot that ets-loader obtains from its own API
    /// declarations plus `globalModulePaths` supplied by the build plan.
    ///
    /// HarmonyOS builds keep the OpenHarmony ets-loader as the compiler owner
    /// but add sibling SDK declaration roots (for example `hms/ets/api`) to
    /// the TypeScript program. Those declarations may export `@Builder`
    /// functions, so the transform must include them before lowering ArkUI
    /// child calls.
    pub fn from_ets_loader_path_with_external_declaration_roots(
        ets_loader_path: &Path,
        external_declaration_roots: &[PathBuf],
    ) -> anyhow::Result<Self> {
        ComponentMetadataLoader::new(ets_loader_path, external_declaration_roots).load()
    }

    pub fn needs_pop(&self, name: &str) -> bool {
        !self.atomic_components.contains(name) || self.need_pop_components.contains(name)
    }

    pub(crate) fn parse_ets<'a>(
        &self,
        allocator: &'a Allocator,
        source: &'a str,
        source_type: SourceType,
    ) -> ParserReturn<'a> {
        self.arkts_parser.parse(allocator, source, source_type)
    }

    pub fn is_known_enum(&self, name: &str) -> bool {
        self.enum_names.contains(name)
    }

    pub fn is_builder_attribute_method(&self, component_name: &str, method_name: &str) -> bool {
        self.bind_object_properties
            .get(component_name)
            .is_some_and(|methods| methods.contains(method_name))
            || self
                .bind_object_properties
                .get(SdkArkUiSemanticSnapshot::ALL_COMPONENTS)
                .is_some_and(|methods| methods.contains(method_name))
            || (method_name == self.runtime_names.method_create
                && self.create_bind_components.contains(component_name))
    }

    pub fn is_create_bind_component(&self, component_name: &str) -> bool {
        self.create_bind_components.contains(component_name)
    }

    pub fn is_builder_attribute_without_key(&self, attr_name: &str) -> bool {
        self.custom_builder_properties_without_key
            .contains(attr_name)
    }

    pub fn is_style_double_dollar_attr(&self, attr_name: &str) -> bool {
        self.style_double_dollar_attrs.contains(attr_name)
    }

    pub fn is_component_double_dollar_property(
        &self,
        component_name: &str,
        attr_name: &str,
    ) -> bool {
        self.component_double_dollar_properties
            .get(component_name)
            .is_some_and(|properties| properties.contains(attr_name))
    }

    pub fn is_component_double_dollar_property_name(
        &self,
        component_name: &str,
        property_name: &str,
    ) -> bool {
        self.is_component_double_dollar_property(component_name, property_name)
    }

    pub fn is_bind_popup_attr(&self, attr_name: &str) -> bool {
        self.bind_popup_attrs.contains(attr_name)
    }

    pub fn is_bind_drag_attr(&self, attr_name: &str) -> bool {
        self.bind_drag_attrs.contains(attr_name)
    }

    pub fn is_gesture_type_name(&self, name: &str) -> bool {
        self.gesture_type_names.contains(name)
    }

    pub fn is_gesture_attribute_name(&self, name: &str) -> bool {
        self.gesture_attr_names.contains(name)
    }

    pub fn runtime_names(&self) -> &ArkuiRuntimeNames {
        &self.runtime_names
    }

    pub(crate) fn sdk_component(
        &self,
        specifier: &str,
        binding: &str,
    ) -> Option<&SdkComponentInfo> {
        self.sdk_components
            .get(specifier)
            .and_then(|components| components.get(binding))
    }

    pub(crate) fn is_sdk_builder_function(&self, specifier: &str, binding: &str) -> bool {
        self.sdk_builder_functions
            .get(specifier)
            .is_some_and(|builders| builders.contains(binding))
    }

    pub(crate) fn is_sdk_enum_export(&self, specifier: &str, binding: &str) -> bool {
        self.sdk_enum_exports
            .get(specifier)
            .is_some_and(|enums| enums.contains(binding))
    }

    pub(crate) fn sdk_namespace_enum_exports(
        &self,
        specifier: &str,
        binding: &str,
    ) -> Option<&FxHashSet<String>> {
        self.sdk_namespace_enum_exports
            .get(specifier)
            .and_then(|bindings| bindings.get(binding))
    }
}

#[derive(Debug, Clone, Default)]
struct ArkTsParserConfig {
    options: Option<ArkTsOptions>,
}

impl ArkTsParserConfig {
    fn from_sdk(options: ArkTsOptions) -> Self {
        Self {
            options: Some(options),
        }
    }

    fn parse<'a>(
        &self,
        allocator: &'a Allocator,
        source: &'a str,
        source_type: SourceType,
    ) -> ParserReturn<'a> {
        let parser = Parser::new(allocator, source, source_type);
        match &self.options {
            Some(options) => parser.with_arkts_options(options.clone()).parse(),
            None => parser.parse(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SdkComponentInfo {
    pub(crate) update_props: FxHashSet<String>,
    pub(crate) link_props: FxHashSet<String>,
    pub(crate) reusable: bool,
    pub(crate) is_custom_dialog: bool,
}

#[derive(Debug, Clone)]
pub struct ArkuiRuntimeNames {
    pub component_for_each: String,
    pub component_lazy_for_each: String,
    pub component_repeat: String,
    pub repeat_each: String,
    pub repeat_template: String,
    pub component_list_item: String,
    pub component_grid_item: String,
    pub component_arc_list_item: String,
    pub component_button: String,
    pub component_tab_content: String,
    pub component_navigation: String,
    pub component_nav_destination: String,
    pub component_gesture_group: String,
    pub component_gesture: String,
    pub component_if: String,
    pub component_xcomponent: String,
    pub xcomponent_type: String,
    pub xcomponent_type_container: String,
    pub attribute_id: String,
    pub attribute_state_styles: String,
    pub can_retake: String,
    pub visual_state: String,
    pub method_create: String,
    pub method_create_with_child: String,
    pub method_create_with_label: String,
    pub method_create_recycle: String,
    pub method_render: String,
    pub method_pop: String,
    pub method_bind: String,
    pub component_context: String,
    pub component_common: String,
    pub component_recycle: String,
    pub attribute_modifier: String,
    pub content_modifier: String,
    pub menu_item_content_modifier: String,
    pub builder_param_proxy: String,
    pub method_call: String,
    pub double_dollar_this: String,
    pub double_dollar_new_value: String,
    pub double_dollar_value: String,
    pub double_dollar_change_event: String,
    pub custom_dialog_controller_builder: String,
    pub header: String,
    pub indicator_builder: String,
    pub footer: String,
    pub start: String,
    pub end: String,
    pub preview: String,
    pub title: String,
    pub gesture_priority: String,
    pub gesture_priority_low: String,
    pub is_initial_render: String,
    pub elmt_id: String,
    pub undefined: String,
    pub view_stack_processor: String,
    pub start_get_access_recording_for: String,
    pub stop_get_access_recording: String,
    pub observe_component_creation2: String,
    pub item_creation: String,
    pub item_creation2: String,
    pub observed_deep_render: String,
    pub item_binding: String,
    pub foreach_item_gen_function: String,
    pub foreach_update_function: String,
    pub lazy_foreach_item_gen_function: String,
    pub lazy_foreach_item_id_func: String,
    pub lazy_foreach_create_version: String,
    pub deep_render_function: String,
    pub if_else_branch_update_function: String,
    pub component_call: String,
    pub params_lambda: String,
    pub params_generator_prefix: String,
    pub update_state_vars_of_child_by_elmt_id: String,
    pub observe_recycle_component_creation: String,
    pub recycle_node: String,
    pub about_to_reuse_internal: String,
    pub about_to_reuse: String,
    pub rerender: String,
    pub function_type: String,
    pub observed_object: String,
    pub get_raw_object: String,
    pub name: String,
    pub module_name: String,
    pub page_path: String,
    pub is_user_create_stack: String,
    pub builder_property: String,
    pub reuse_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArkuiRuntimeComponentKind {
    ForEach,
    LazyForEach,
    ListItem,
    GridItem,
    ArcListItem,
    Button,
    TabContent,
    Navigation,
    NavDestination,
    GestureGroup,
    XComponent,
    Other,
}

impl ArkuiRuntimeComponentKind {
    pub fn is_for_each(self) -> bool {
        matches!(self, Self::ForEach)
    }

    pub fn is_lazy_for_each(self) -> bool {
        matches!(self, Self::LazyForEach)
    }

    pub fn is_list_item(self) -> bool {
        matches!(self, Self::ListItem)
    }

    pub fn is_item_component(self) -> bool {
        matches!(self, Self::ListItem | Self::GridItem | Self::ArcListItem)
    }

    pub fn is_nav_router(self) -> bool {
        matches!(self, Self::NavDestination | Self::Navigation)
    }

    pub fn creates_child_body(self) -> bool {
        matches!(self, Self::TabContent)
    }

    pub fn is_navigation(self) -> bool {
        matches!(self, Self::Navigation)
    }

    pub fn is_gesture_group(self) -> bool {
        matches!(self, Self::GestureGroup)
    }
}

impl Default for ArkuiRuntimeNames {
    fn default() -> Self {
        Self {
            component_for_each: "ForEach".to_string(),
            component_lazy_for_each: "LazyForEach".to_string(),
            component_repeat: "Repeat".to_string(),
            repeat_each: "each".to_string(),
            repeat_template: "template".to_string(),
            component_list_item: "ListItem".to_string(),
            component_grid_item: "GridItem".to_string(),
            component_arc_list_item: "ArcListItem".to_string(),
            component_button: "Button".to_string(),
            component_tab_content: "TabContent".to_string(),
            component_navigation: "Navigation".to_string(),
            component_nav_destination: "NavDestination".to_string(),
            component_gesture_group: "GestureGroup".to_string(),
            component_gesture: "Gesture".to_string(),
            component_if: "If".to_string(),
            component_xcomponent: "XComponent".to_string(),
            xcomponent_type: "XComponentType".to_string(),
            xcomponent_type_container: "COMPONENT".to_string(),
            attribute_id: "id".to_string(),
            attribute_state_styles: "stateStyles".to_string(),
            can_retake: "canRetake".to_string(),
            visual_state: "visualState".to_string(),
            method_create: "create".to_string(),
            method_create_with_child: "createWithChild".to_string(),
            method_create_with_label: "createWithLabel".to_string(),
            method_create_recycle: "createRecycle".to_string(),
            method_render: "render".to_string(),
            method_pop: "pop".to_string(),
            method_bind: "bind".to_string(),
            component_context: "Context".to_string(),
            component_common: "__Common__".to_string(),
            component_recycle: "__Recycle__".to_string(),
            attribute_modifier: "attributeModifier".to_string(),
            content_modifier: "contentModifier".to_string(),
            menu_item_content_modifier: "menuItemContentModifier".to_string(),
            builder_param_proxy: "makeBuilderParameterProxy".to_string(),
            method_call: "call".to_string(),
            double_dollar_this: "$$this".to_string(),
            double_dollar_new_value: "newValue".to_string(),
            double_dollar_value: "value".to_string(),
            double_dollar_change_event: "changeEvent".to_string(),
            custom_dialog_controller_builder: "builder".to_string(),
            header: "header".to_string(),
            indicator_builder: "indicatorBuilder".to_string(),
            footer: "footer".to_string(),
            start: "start".to_string(),
            end: "end".to_string(),
            preview: "preview".to_string(),
            title: "title".to_string(),
            gesture_priority: "GesturePriority".to_string(),
            gesture_priority_low: "Low".to_string(),
            is_initial_render: "isInitialRender".to_string(),
            elmt_id: "elmtId".to_string(),
            undefined: "undefined".to_string(),
            view_stack_processor: "ViewStackProcessor".to_string(),
            start_get_access_recording_for: "StartGetAccessRecordingFor".to_string(),
            stop_get_access_recording: "StopGetAccessRecording".to_string(),
            observe_component_creation2: "observeComponentCreation2".to_string(),
            item_creation: "itemCreation".to_string(),
            item_creation2: "itemCreation2".to_string(),
            observed_deep_render: "observedDeepRender".to_string(),
            item_binding: "_item".to_string(),
            foreach_item_gen_function: "forEachItemGenFunction".to_string(),
            foreach_update_function: "forEachUpdateFunction".to_string(),
            lazy_foreach_item_gen_function: "__lazyForEachItemGenFunction".to_string(),
            lazy_foreach_item_id_func: "__lazyForEachItemIdFunc".to_string(),
            lazy_foreach_create_version: "1".to_string(),
            deep_render_function: "deepRenderFunction".to_string(),
            if_else_branch_update_function: "ifElseBranchUpdateFunction".to_string(),
            component_call: "componentCall".to_string(),
            params_lambda: "paramsLambda".to_string(),
            params_generator_prefix: "paramsGenerator_".to_string(),
            update_state_vars_of_child_by_elmt_id: "updateStateVarsOfChildByElmtId".to_string(),
            observe_recycle_component_creation: "observeRecycleComponentCreation".to_string(),
            recycle_node: "recycleNode".to_string(),
            about_to_reuse_internal: "aboutToReuseInternal".to_string(),
            about_to_reuse: "aboutToReuse".to_string(),
            rerender: "rerender".to_string(),
            function_type: "function".to_string(),
            observed_object: "ObservedObject".to_string(),
            get_raw_object: "GetRawObject".to_string(),
            name: "name".to_string(),
            module_name: "moduleName".to_string(),
            page_path: "pagePath".to_string(),
            is_user_create_stack: "isUserCreateStack".to_string(),
            builder_property: "builder".to_string(),
            reuse_id: "reuseId".to_string(),
        }
    }
}

impl ArkuiRuntimeNames {
    pub fn component_kind(&self, name: &str) -> ArkuiRuntimeComponentKind {
        if name == self.component_for_each {
            return ArkuiRuntimeComponentKind::ForEach;
        }
        if name == self.component_lazy_for_each {
            return ArkuiRuntimeComponentKind::LazyForEach;
        }
        if name == self.component_repeat {
            return ArkuiRuntimeComponentKind::Other;
        }
        if name == self.component_list_item {
            return ArkuiRuntimeComponentKind::ListItem;
        }
        if name == self.component_grid_item {
            return ArkuiRuntimeComponentKind::GridItem;
        }
        if name == self.component_arc_list_item {
            return ArkuiRuntimeComponentKind::ArcListItem;
        }
        if name == self.component_button {
            return ArkuiRuntimeComponentKind::Button;
        }
        if name == self.component_tab_content {
            return ArkuiRuntimeComponentKind::TabContent;
        }
        if name == self.component_navigation {
            return ArkuiRuntimeComponentKind::Navigation;
        }
        if name == self.component_nav_destination {
            return ArkuiRuntimeComponentKind::NavDestination;
        }
        if name == self.component_gesture_group {
            return ArkuiRuntimeComponentKind::GestureGroup;
        }
        if name == self.component_xcomponent {
            return ArkuiRuntimeComponentKind::XComponent;
        }
        ArkuiRuntimeComponentKind::Other
    }

    pub fn component_attribute_requires_bind(&self, attr_name: &str) -> bool {
        attr_name == self.attribute_modifier
            || attr_name == self.content_modifier
            || attr_name == self.menu_item_content_modifier
    }

    pub fn is_builder_object_property_name(&self, property_name: &str) -> bool {
        property_name == self.custom_dialog_controller_builder
            || property_name == self.header
            || property_name == self.indicator_builder
            || property_name == self.footer
            || property_name == self.start
            || property_name == self.end
            || property_name == self.preview
            || property_name == self.title
    }
}

struct ComponentMetadataLoader<'a> {
    ets_loader_path: &'a Path,
    external_declaration_roots: &'a [PathBuf],
}

impl<'a> ComponentMetadataLoader<'a> {
    fn new(ets_loader_path: &'a Path, external_declaration_roots: &'a [PathBuf]) -> Self {
        Self {
            ets_loader_path,
            external_declaration_roots,
        }
    }

    fn load(&self) -> anyhow::Result<ComponentMetadata> {
        let arkts_parser = ArkTsParserConfig::from_sdk(
            SdkArkTsOptionsLoader::new(self.ets_loader_path.join("tsconfig.json")).load()?,
        );
        let atomic_components = self.load_atomic_components()?;
        let semantics = SdkArkUiSemanticSnapshot::bundled();
        let enum_names = SdkEnumNameLoader::new(self.ets_loader_path).load()?;
        let sdk_declarations =
            SdkComponentLoader::new(self.ets_loader_path, self.external_declaration_roots)
                .load()?;

        let mut bind_object_properties = semantics.bind_object_properties;
        if !semantics.custom_builder_properties.is_empty() {
            bind_object_properties
                .entry(SdkArkUiSemanticSnapshot::ALL_COMPONENTS.to_string())
                .or_default()
                .extend(semantics.custom_builder_properties);
        }

        Ok(ComponentMetadata {
            arkts_parser,
            atomic_components,
            need_pop_components: semantics.need_pop_components,
            bind_object_properties,
            create_bind_components: semantics.create_bind_components,
            custom_builder_properties_without_key: semantics.custom_builder_properties_without_key,
            style_double_dollar_attrs: semantics.style_double_dollar_attrs,
            component_double_dollar_properties: semantics.component_double_dollar_properties,
            bind_popup_attrs: semantics.bind_popup_attrs,
            bind_drag_attrs: semantics.bind_drag_attrs,
            gesture_attr_names: semantics.gesture_attr_names,
            gesture_type_names: semantics.gesture_type_names,
            enum_names,
            sdk_components: sdk_declarations.components,
            sdk_builder_functions: sdk_declarations.builder_functions,
            sdk_enum_exports: sdk_declarations.enum_exports,
            sdk_namespace_enum_exports: sdk_declarations.namespace_enum_exports,
            runtime_names: ArkuiRuntimeNames::default(),
        })
    }

    fn load_atomic_components(&self) -> anyhow::Result<FxHashSet<String>> {
        let components_dir = self.ets_loader_path.join("components");
        let mut atomic_components = FxHashSet::default();
        for entry in fs::read_dir(&components_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            let Some(name) = value.get("name").and_then(Value::as_str) else {
                continue;
            };
            if value.get("atomic").and_then(Value::as_bool) == Some(true) {
                atomic_components.insert(name.to_string());
            }
        }
        Ok(atomic_components)
    }
}

struct SdkArkTsOptionsLoader {
    path: PathBuf,
}

impl SdkArkTsOptionsLoader {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> anyhow::Result<ArkTsOptions> {
        let source = fs::read_to_string(&self.path)?;
        let config: SdkTsConfig = json5::from_str(&source).map_err(|error| {
            anyhow::anyhow!(
                "failed to parse SDK ArkTS options at {}: {error}",
                self.path.display()
            )
        })?;
        Ok(config.compiler_options.ets.into_oxc())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SdkTsConfig {
    compiler_options: SdkCompilerOptions,
}

#[derive(Deserialize)]
struct SdkCompilerOptions {
    ets: SdkEtsOptions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SdkEtsOptions {
    components: Vec<String>,
    render: SdkRenderOptions,
    extend: SdkExtendOptions,
    styles: SdkStylesOptions,
    syntax_components: SdkSyntaxComponentOptions,
}

impl SdkEtsOptions {
    fn into_oxc(self) -> ArkTsOptions {
        ArkTsOptions {
            components: self.components,
            render_methods: self.render.method,
            render_decorators: self.render.decorator,
            extend_decorators: self.extend.decorator,
            styles_decorator: Some(self.styles.decorator),
            parameter_ui_callbacks: self.syntax_components.params_ui_callback,
            attribute_ui_callbacks: self
                .syntax_components
                .attr_ui_callback
                .into_iter()
                .map(|callback| ArkTsAttributeCallback {
                    component: callback.name,
                    attributes: callback.attributes,
                })
                .collect(),
            annotations: true,
        }
    }
}

#[derive(Deserialize)]
struct SdkRenderOptions {
    method: Vec<String>,
    decorator: Vec<String>,
}

#[derive(Deserialize)]
struct SdkExtendOptions {
    decorator: Vec<String>,
}

#[derive(Deserialize)]
struct SdkStylesOptions {
    decorator: String,
}

#[derive(Deserialize)]
struct SdkSyntaxComponentOptions {
    #[serde(rename = "paramsUICallback")]
    params_ui_callback: Vec<String>,
    #[serde(rename = "attrUICallback")]
    attr_ui_callback: Vec<SdkAttributeUiCallback>,
}

#[derive(Deserialize)]
struct SdkAttributeUiCallback {
    name: String,
    attributes: Vec<String>,
}

struct SdkComponentLoader<'a> {
    ets_loader_path: &'a Path,
    external_declaration_roots: &'a [PathBuf],
}

#[derive(Default)]
struct SdkDeclarationMetadata {
    components: FxHashMap<String, FxHashMap<String, SdkComponentInfo>>,
    builder_functions: FxHashMap<String, FxHashSet<String>>,
    enum_exports: FxHashMap<String, FxHashSet<String>>,
    namespace_enum_exports: FxHashMap<String, FxHashMap<String, FxHashSet<String>>>,
}

impl<'a> SdkComponentLoader<'a> {
    fn new(ets_loader_path: &'a Path, external_declaration_roots: &'a [PathBuf]) -> Self {
        Self {
            ets_loader_path,
            external_declaration_roots,
        }
    }

    fn load(&self) -> anyhow::Result<SdkDeclarationMetadata> {
        let mut declarations = SdkDeclarationMetadata::default();
        let primary_ets_root = self.primary_ets_root();
        self.load_api_declarations(&primary_ets_root.join("api"), &mut declarations)?;
        for root in self.external_declaration_roots {
            if root.starts_with(&primary_ets_root) {
                continue;
            }
            self.load_api_declarations(root, &mut declarations)?;
        }
        for kit_root in self.kit_config_roots(&primary_ets_root) {
            self.load_kit_declarations(&kit_root, &mut declarations)?;
        }
        Ok(declarations)
    }

    fn primary_ets_root(&self) -> PathBuf {
        self.ets_loader_path
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.ets_loader_path.join("../.."))
    }

    fn kit_config_roots(&self, primary_ets_root: &Path) -> Vec<PathBuf> {
        let mut roots = vec![self.ets_loader_path.join("kit_configs")];
        for declaration_root in self.external_declaration_roots {
            let Some(ets_root) = declaration_root
                .ancestors()
                .find(|path| path.join("sdkConfig.json").is_file())
            else {
                continue;
            };
            if ets_root == primary_ets_root {
                continue;
            }
            let kit_root = ets_root
                .join("build-tools")
                .join("ets-loader")
                .join("kit_configs");
            if !roots.contains(&kit_root) {
                roots.push(kit_root);
            }
        }
        roots
    }

    fn load_api_declarations(
        &self,
        api_root: &Path,
        declarations: &mut SdkDeclarationMetadata,
    ) -> anyhow::Result<()> {
        let mut paths = Vec::new();
        self.collect_declaration_files(&api_root, &mut paths)?;
        paths.sort();

        for path in paths {
            let source = fs::read_to_string(&path)?;
            let allocator = Allocator::default();
            let source_type = CompileArktsSourceType::from_path(&path);
            let parsed = Parser::new(&allocator, &source, source_type).parse();
            // The platform API directory contains declarations accepted by the
            // SDK's TypeScript version but not yet accepted by OXC. OXC still
            // returns a recovered AST, and the metadata below only consumes
            // complete enum and ArkUI struct declarations from that tree. An
            // unrelated recovery diagnostic must not make all SDK metadata
            // unavailable to a build.

            let components = SdkComponentDeclarationCollector::new(&parsed.program).collect();
            let builders = SdkBuilderFunctionDeclarationCollector::new(&parsed.program).collect();
            let enums = SdkModuleEnumDeclarationCollector::new(&parsed.program).collect();

            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            for specifier in SdkDeclarationSpecifier::new(file_name).forms() {
                if !components.is_empty() {
                    declarations
                        .components
                        .insert(specifier.clone(), components.clone());
                }
                if !builders.is_empty() {
                    declarations
                        .builder_functions
                        .insert(specifier.clone(), builders.clone());
                }
                if !enums.exports.is_empty() {
                    declarations
                        .enum_exports
                        .insert(specifier.clone(), enums.exports.clone());
                }
                if !enums.namespace.is_empty() {
                    declarations
                        .namespace_enum_exports
                        .entry(specifier)
                        .or_default()
                        .insert("default".to_string(), enums.namespace.clone());
                }
            }
        }
        Ok(())
    }

    fn load_kit_declarations(
        &self,
        kit_root: &Path,
        declarations: &mut SdkDeclarationMetadata,
    ) -> anyhow::Result<()> {
        if !kit_root.is_dir() {
            return Ok(());
        }

        let mut paths = fs::read_dir(kit_root)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();

        for path in paths {
            let Some(kit_specifier) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            let Some(symbols) = value.get("symbols").and_then(Value::as_object) else {
                continue;
            };
            for (local_name, symbol) in symbols {
                let Some(source) = symbol.get("source").and_then(Value::as_str) else {
                    continue;
                };
                let Some(binding) = symbol.get("bindings").and_then(Value::as_str) else {
                    continue;
                };
                let source_specifiers = SdkDeclarationSpecifier::new(source).forms();
                if let Some(info) = source_specifiers.iter().find_map(|specifier| {
                    declarations
                        .components
                        .get(specifier)
                        .and_then(|components| components.get(binding))
                        .cloned()
                }) {
                    declarations
                        .components
                        .entry(kit_specifier.to_string())
                        .or_default()
                        .insert(local_name.clone(), info);
                }
                if source_specifiers.iter().any(|specifier| {
                    declarations
                        .builder_functions
                        .get(specifier)
                        .is_some_and(|builders| builders.contains(binding))
                }) {
                    declarations
                        .builder_functions
                        .entry(kit_specifier.to_string())
                        .or_default()
                        .insert(local_name.clone());
                }
                if source_specifiers.iter().any(|specifier| {
                    declarations
                        .enum_exports
                        .get(specifier)
                        .is_some_and(|enums| enums.contains(binding))
                }) {
                    declarations
                        .enum_exports
                        .entry(kit_specifier.to_string())
                        .or_default()
                        .insert(local_name.clone());
                }
                if let Some(enums) = source_specifiers.iter().find_map(|specifier| {
                    declarations
                        .namespace_enum_exports
                        .get(specifier)
                        .and_then(|bindings| bindings.get(binding))
                        .cloned()
                }) {
                    declarations
                        .namespace_enum_exports
                        .entry(kit_specifier.to_string())
                        .or_default()
                        .insert(local_name.clone(), enums);
                }
            }
        }

        Ok(())
    }

    fn collect_declaration_files(
        &self,
        root: &Path,
        paths: &mut Vec<PathBuf>,
    ) -> anyhow::Result<()> {
        if !root.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_declaration_files(&path, paths)?;
            } else if matches!(
                SourceClassifier::classify_path(&path),
                SourceFileKind::Declaration
            ) {
                paths.push(path);
            }
        }
        Ok(())
    }
}

struct SdkDeclarationSpecifier<'a> {
    raw: &'a str,
}

impl<'a> SdkDeclarationSpecifier<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn forms(&self) -> Vec<String> {
        let dot_form = self.without_declaration_suffix().unwrap_or(self.raw);
        let mut forms = vec![dot_form.to_string()];
        if let Some(colon_form) = Self::colon_form(dot_form) {
            forms.push(colon_form);
        }
        forms
    }

    fn without_declaration_suffix(&self) -> Option<&str> {
        self.raw
            .strip_suffix(".d.ets")
            .or_else(|| self.raw.strip_suffix(".d.ts"))
    }

    fn colon_form(dot_form: &str) -> Option<String> {
        let (prefix, module) = dot_form.split_once('.')?;
        if !prefix.starts_with('@') || module.is_empty() {
            return None;
        }
        Some(format!("{prefix}:{module}"))
    }
}

struct SdkBuilderFunctionDeclarationCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> SdkBuilderFunctionDeclarationCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashSet<String> {
        let mut builders = FxHashSet::default();
        for statement in &self.program.body {
            match statement {
                Statement::FunctionDeclaration(function) => {
                    self.collect_function(function, &mut builders);
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::FunctionDeclaration(function) = &export.declaration {
                        self.collect_function(function, &mut builders);
                    }
                }
                _ => {}
            }
        }
        builders
    }

    fn collect_function(
        &self,
        function: &oxc_ast::ast::Function<'_>,
        builders: &mut FxHashSet<String>,
    ) {
        if !function
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER)
        {
            return;
        }
        if let Some(id) = &function.id {
            builders.insert(id.name.to_string());
        }
    }
}

#[derive(Default)]
struct SdkModuleEnumDeclarations {
    exports: FxHashSet<String>,
    namespace: FxHashSet<String>,
}

struct SdkModuleEnumDeclarationCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> SdkModuleEnumDeclarationCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> SdkModuleEnumDeclarations {
        let mut declarations = SdkModuleEnumDeclarations::default();
        for statement in &self.program.body {
            match statement {
                Statement::TSEnumDeclaration(declaration) => {
                    declarations.exports.insert(declaration.id.name.to_string());
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::TSEnumDeclaration(declaration) = &export.declaration {
                        declarations.exports.insert(declaration.id.name.to_string());
                    }
                }
                _ => {}
            }
        }
        let mut nested = SdkNestedEnumNameCollector::default();
        nested.visit_program(self.program);
        declarations.namespace = nested.names;
        declarations
    }
}

#[derive(Default)]
struct SdkNestedEnumNameCollector {
    names: FxHashSet<String>,
}

impl<'a> Visit<'a> for SdkNestedEnumNameCollector {
    fn visit_ts_enum_declaration(&mut self, declaration: &TSEnumDeclaration<'a>) {
        self.names.insert(declaration.id.name.to_string());
    }
}

struct SdkComponentDeclarationCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> SdkComponentDeclarationCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashMap<String, SdkComponentInfo> {
        let mut components = FxHashMap::default();
        for statement in &self.program.body {
            match statement {
                Statement::StructStatement(statement) => {
                    self.insert(statement, &mut components);
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::StructStatement(statement) = &export.declaration {
                        self.insert(statement, &mut components);
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::StructStatement(statement) =
                        &export.declaration
                    {
                        if let Some(info) = self.component_info(statement) {
                            components.insert("default".to_string(), info);
                        }
                    }
                }
                _ => {}
            }
        }
        components
    }

    fn insert(
        &self,
        statement: &StructStatement<'_>,
        components: &mut FxHashMap<String, SdkComponentInfo>,
    ) {
        if let Some(info) = self.component_info(statement) {
            components.insert(statement.id.name.to_string(), info);
        }
    }

    fn component_info(&self, statement: &StructStatement<'_>) -> Option<SdkComponentInfo> {
        let is_component = statement.decorators.iter().any(|decorator| {
            ArkuiDecoratorName::is_component_struct(ArkuiDecorator::new(decorator).name())
        });
        if !is_component {
            return None;
        }

        let mut update_props = FxHashSet::default();
        let mut link_props = FxHashSet::default();
        for element in &statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let Some(property_name) = property.key.static_name() else {
                continue;
            };
            for decorator in &property.decorators {
                match ArkuiDecorator::new(decorator).name() {
                    name if ArkuiDecoratorName::is_custom_component_update_prop(name) => {
                        update_props.insert(property_name.to_string());
                    }
                    ArkuiDecoratorName::LINK => {
                        link_props.insert(property_name.to_string());
                    }
                    _ => {}
                }
            }
        }

        Some(SdkComponentInfo {
            update_props,
            link_props,
            reusable: statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE
            }),
            is_custom_dialog: statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_DIALOG
            }),
        })
    }
}

struct SdkEnumNameLoader<'a> {
    ets_loader_path: &'a Path,
}

impl<'a> SdkEnumNameLoader<'a> {
    fn new(ets_loader_path: &'a Path) -> Self {
        Self { ets_loader_path }
    }

    fn load(&self) -> anyhow::Result<FxHashSet<String>> {
        let roots = [
            self.ets_loader_path.join("declarations"),
            self.ets_loader_path.join("..").join("..").join("component"),
        ];
        let mut paths = Vec::new();
        for root in roots {
            self.collect_declaration_files(&root, &mut paths)?;
        }
        paths.sort();

        let mut names = FxHashSet::default();
        for path in paths {
            let source = fs::read_to_string(&path)?;
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &source, SourceType::ts()).parse();
            if !parsed.diagnostics.is_empty() {
                anyhow::bail!(
                    "failed to parse SDK enum declarations at {}",
                    path.display()
                );
            }
            names.extend(SdkEnumDeclarationCollector::new(&parsed.program).collect());
        }
        Ok(names)
    }

    fn collect_declaration_files(
        &self,
        root: &Path,
        paths: &mut Vec<PathBuf>,
    ) -> anyhow::Result<()> {
        if !root.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_declaration_files(&path, paths)?;
            } else if matches!(
                SourceClassifier::classify_path(&path),
                SourceFileKind::Declaration
            ) && path.extension().and_then(|ext| ext.to_str()) == Some("ts")
            {
                paths.push(path);
            }
        }
        Ok(())
    }
}

struct SdkEnumDeclarationCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> SdkEnumDeclarationCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        for statement in &self.program.body {
            match statement {
                Statement::TSEnumDeclaration(declaration) => {
                    names.insert(declaration.id.name.to_string());
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::TSEnumDeclaration(declaration) = &export.declaration {
                        names.insert(declaration.id.name.to_string());
                    }
                }
                _ => {}
            }
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loads_sdk_declarations_with_bundled_arkui_semantics() {
        let metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ets-loader"
        )))
        .expect("test ets-loader metadata");
        assert!(metadata.is_known_enum("FontWeight"));
        assert!(metadata.is_known_enum("ImageInterpolation"));
        assert!(metadata.needs_pop("Blank"));
        assert!(metadata.needs_pop("Search"));
        assert_eq!(metadata.runtime_names().component_for_each, "ForEach");
        assert_eq!(
            metadata.runtime_names().component_gesture_group,
            "GestureGroup"
        );
        assert!(metadata.is_gesture_attribute_name("gesture"));
        assert!(metadata.is_gesture_attribute_name("priorityGesture"));
        assert!(metadata.is_gesture_type_name("TapGesture"));
        assert!(metadata.is_builder_attribute_method("Navigation", "navDestination"));
        assert!(metadata.is_builder_attribute_method("Text", "bindContextMenuWithResponse"));
        assert!(metadata.is_builder_attribute_method("Web", "bindSelectionMenu"));
        assert!(metadata.is_builder_attribute_method("ListItemGroup", "create"));
        assert_eq!(
            metadata.runtime_names().method_create_with_child,
            "createWithChild"
        );
        assert_eq!(metadata.runtime_names().component_common, "__Common__");
        assert_eq!(metadata.runtime_names().component_context, "Context");
        assert_eq!(
            metadata.runtime_names().attribute_modifier,
            "attributeModifier"
        );
        assert_eq!(
            metadata.runtime_names().builder_param_proxy,
            "makeBuilderParameterProxy"
        );
        assert_eq!(metadata.runtime_names().method_call, "call");
        assert!(metadata.is_style_double_dollar_attr("bindSheet"));
        assert!(metadata.is_component_double_dollar_property("TextInput", "text"));
        assert!(metadata.is_component_double_dollar_property("TextTimer", "isCountDown"));
        assert!(metadata.is_bind_popup_attr("bindPopup"));
        assert!(metadata.is_bind_drag_attr("onDragStart"));
        assert!(metadata.is_builder_attribute_without_key("create"));
        assert_eq!(metadata.runtime_names().method_bind, "bind");
        assert_eq!(metadata.runtime_names().module_name, "moduleName");
        assert_eq!(metadata.runtime_names().page_path, "pagePath");
        assert_eq!(
            metadata.runtime_names().is_user_create_stack,
            "isUserCreateStack"
        );
        assert_eq!(metadata.runtime_names().builder_property, "builder");
        assert_eq!(
            metadata.runtime_names().observe_component_creation2,
            "observeComponentCreation2"
        );
        assert_eq!(
            metadata.runtime_names().foreach_update_function,
            "forEachUpdateFunction"
        );
        assert_eq!(
            metadata.runtime_names().observe_recycle_component_creation,
            "observeRecycleComponentCreation"
        );
        assert_eq!(metadata.runtime_names().recycle_node, "recycleNode");
        assert_eq!(metadata.runtime_names().observed_object, "ObservedObject");
        assert_eq!(metadata.runtime_names().get_raw_object, "GetRawObject");
        assert_eq!(metadata.runtime_names().reuse_id, "reuseId");
        let kit_component = metadata
            .sdk_component("@kit.AudioKit", "AVVolumePanel")
            .expect("kit component declaration");
        assert!(kit_component.update_props.contains("volumeLevel"));
        assert!(kit_component.update_props.contains("volumeParameter"));
        assert!(metadata
            .sdk_component("@ohos.multimedia.avVolumePanel", "AVVolumePanel")
            .is_some());
        assert!(metadata
            .sdk_component("@ohos:multimedia.avVolumePanel", "AVVolumePanel")
            .is_some());
        assert!(metadata
            .sdk_namespace_enum_exports("@ohos.file.photoAccessHelper", "default")
            .is_some_and(|enums| enums.contains("PhotoViewMIMETypes")));
        assert!(metadata
            .sdk_namespace_enum_exports("@kit.MediaLibraryKit", "photoAccessHelper")
            .is_some_and(|enums| enums.contains("PhotoViewMIMETypes")));
    }

    #[test]
    fn maps_external_sdk_builder_functions_through_kit_configs() {
        let temp = tempfile::tempdir().expect("temporary external SDK");
        let external_ets = temp.path().join("hms").join("ets");
        let api_root = external_ets.join("api");
        let kit_root = external_ets
            .join("build-tools")
            .join("ets-loader")
            .join("kit_configs");
        std::fs::create_dir_all(&api_root).expect("external API directory");
        std::fs::create_dir_all(&kit_root).expect("external kit-config directory");
        std::fs::write(
            external_ets.join("sdkConfig.json"),
            r#"{"apiPath":"./api"}"#,
        )
        .expect("external SDK config");
        std::fs::write(
            api_root.join("@hms.collaboration.service.d.ets"),
            r#"@Builder
declare function createCollaborationServiceMenuItems(): void;
export { createCollaborationServiceMenuItems };"#,
        )
        .expect("external Builder declaration");
        std::fs::write(
            kit_root.join("@kit.ServiceCollaborationKit.json"),
            r#"{
  "symbols": {
    "createCollaborationServiceMenuItems": {
      "source": "@hms.collaboration.service",
      "bindings": "createCollaborationServiceMenuItems"
    }
  }
}"#,
        )
        .expect("external kit config");

        let metadata = ComponentMetadata::from_ets_loader_path_with_external_declaration_roots(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/ets-loader"
            )),
            &[api_root],
        )
        .expect("metadata with HarmonyOS external API path");

        assert!(metadata.is_sdk_builder_function(
            "@hms.collaboration.service",
            "createCollaborationServiceMenuItems"
        ));
        assert!(metadata.is_sdk_builder_function(
            "@kit.ServiceCollaborationKit",
            "createCollaborationServiceMenuItems"
        ));
    }
}
