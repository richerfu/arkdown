use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    fs,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Declaration,
    ExportDefaultDeclarationKind, Expression, Program, Statement, StructElement, StructStatement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;

use crate::source::{SourceClassifier, SourceFileKind};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};

struct ComponentMapRuntimeName;

impl ComponentMapRuntimeName {
    const MAP: &'static str = "Map";
    const SET: &'static str = "Set";

    fn is_map(name: &str) -> bool {
        name == Self::MAP
    }

    fn is_set(name: &str) -> bool {
        name == Self::SET
    }
}

#[derive(Debug, Clone, Default)]
pub struct ComponentMetadata {
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
    runtime_names: ArkuiRuntimeNames,
}

impl ComponentMetadata {
    pub fn from_ets_loader_path(ets_loader_path: &Path) -> anyhow::Result<Self> {
        ComponentMetadataLoader::new(ets_loader_path).load()
    }

    pub fn needs_pop(&self, name: &str) -> bool {
        !self.atomic_components.contains(name) || self.need_pop_components.contains(name)
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
                .get(SdkPreDefineBindingExtractor::ALL_COMPONENTS)
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
    pub component_button: String,
    pub component_navigation: String,
    pub component_nav_destination: String,
    pub component_gesture_group: String,
    pub component_gesture: String,
    pub component_if: String,
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
    Button,
    Navigation,
    NavDestination,
    GestureGroup,
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

    pub fn is_nav_router(self) -> bool {
        matches!(self, Self::NavDestination | Self::Navigation)
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
            component_button: "Button".to_string(),
            component_navigation: "Navigation".to_string(),
            component_nav_destination: "NavDestination".to_string(),
            component_gesture_group: "GestureGroup".to_string(),
            component_gesture: "Gesture".to_string(),
            component_if: "If".to_string(),
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
        if name == self.component_button {
            return ArkuiRuntimeComponentKind::Button;
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

    fn set_from_pre_define_binding(&mut self, name: &str, value: &str) {
        match name {
            "COMPONENT_FOREACH" => self.component_for_each = value.to_string(),
            "COMPONENT_LAZYFOREACH" => self.component_lazy_for_each = value.to_string(),
            "COMPONENT_REPEAT" => self.component_repeat = value.to_string(),
            "REPEAT_EACH" => self.repeat_each = value.to_string(),
            "REPEAT_TEMPLATE" => self.repeat_template = value.to_string(),
            "LIST_ITEM" => self.component_list_item = value.to_string(),
            "COMPONENT_BUTTON" => self.component_button = value.to_string(),
            "NAVIGATION" => self.component_navigation = value.to_string(),
            "NAV_DESTINATION" => self.component_nav_destination = value.to_string(),
            "COMPONENT_GESTURE_GROUP" => self.component_gesture_group = value.to_string(),
            "COMPONENT_GESTURE" => self.component_gesture = value.to_string(),
            "COMPONENT_IF" => self.component_if = value.to_string(),
            "COMPONENT_CREATE_FUNCTION" => self.method_create = value.to_string(),
            "COMPONENT_CREATE_CHILD_FUNCTION" => self.method_create_with_child = value.to_string(),
            "COMPONENT_CREATE_LABEL_FUNCTION" => self.method_create_with_label = value.to_string(),
            "COMPONENT_CREATE_RECYCLE" => self.method_create_recycle = value.to_string(),
            "COMPONENT_RENDER_FUNCTION" => self.method_render = value.to_string(),
            "COMPONENT_POP_FUNCTION" => self.method_pop = value.to_string(),
            "BUILDER_ATTR_BIND" => self.method_bind = value.to_string(),
            "GLOBAL_CONTEXT" => self.component_context = value.to_string(),
            "COMPONENT_COMMON" => self.component_common = value.to_string(),
            "COMPONENT_RECYCLE" => self.component_recycle = value.to_string(),
            "ATTRIBUTE_ATTRIBUTE_MODIFIER" => self.attribute_modifier = value.to_string(),
            "ATTRIBUTE_CONTENT_MODIFIER" => self.content_modifier = value.to_string(),
            "ATTRIBUTE_MENUITEM_CONTENT_MODIFIER" => {
                self.menu_item_content_modifier = value.to_string()
            }
            "BUILDER_PARAM_PROXY" => self.builder_param_proxy = value.to_string(),
            "CALL" => self.method_call = value.to_string(),
            "$$_THIS" => self.double_dollar_this = value.to_string(),
            "$$_NEW_VALUE" => self.double_dollar_new_value = value.to_string(),
            "$$_VALUE" => self.double_dollar_value = value.to_string(),
            "$$_CHANGE_EVENT" => self.double_dollar_change_event = value.to_string(),
            "CUSTOM_DIALOG_CONTROLLER_BUILDER" => {
                self.custom_dialog_controller_builder = value.to_string()
            }
            "HEADER" => self.header = value.to_string(),
            "INDICATORBUILDER" => self.indicator_builder = value.to_string(),
            "FOOTER" => self.footer = value.to_string(),
            "START" => self.start = value.to_string(),
            "END" => self.end = value.to_string(),
            "PREVIEW" => self.preview = value.to_string(),
            "TITLE" => self.title = value.to_string(),
            "GESTURE_ENUM_KEY" => self.gesture_priority = value.to_string(),
            "GESTURE_ENUM_VALUE_LOW" => self.gesture_priority_low = value.to_string(),
            "ISINITIALRENDER" => self.is_initial_render = value.to_string(),
            "ELMTID" => self.elmt_id = value.to_string(),
            "COMPONENT_CONSTRUCTOR_UNDEFINED" => self.undefined = value.to_string(),
            "VIEWSTACKPROCESSOR" => self.view_stack_processor = value.to_string(),
            "STARTGETACCESSRECORDINGFOR" => self.start_get_access_recording_for = value.to_string(),
            "STOPGETACCESSRECORDING" => self.stop_get_access_recording = value.to_string(),
            "OBSERVECOMPONENTCREATION2" => self.observe_component_creation2 = value.to_string(),
            "ITEMCREATION" => self.item_creation = value.to_string(),
            "ITEMCREATION2" => self.item_creation2 = value.to_string(),
            "_ITEM" => self.item_binding = value.to_string(),
            "FOREACHITEMGENFUNCTION" => self.foreach_item_gen_function = value.to_string(),
            "FOREACHUPDATEFUNCTION" => self.foreach_update_function = value.to_string(),
            "__LAZYFOREACHITEMGENFUNCTION" => {
                self.lazy_foreach_item_gen_function = value.to_string()
            }
            "__LAZYFOREACHITEMIDFUNC" => self.lazy_foreach_item_id_func = value.to_string(),
            "DEEPRENDERFUNCTION" => self.deep_render_function = value.to_string(),
            "IFELSEBRANCHUPDATEFUNCTION" => self.if_else_branch_update_function = value.to_string(),
            "COMPONENT_CALL" => self.component_call = value.to_string(),
            "COMPONENT_PARAMS_LAMBDA_FUNCTION" => self.params_lambda = value.to_string(),
            "COMPONENT_PARAMS_FUNCTION" => self.params_generator_prefix = value.to_string(),
            "UPDATE_STATE_VARS_OF_CHIND_BY_ELMTID" => {
                self.update_state_vars_of_child_by_elmt_id = value.to_string()
            }
            "OBSERVE_RECYCLE_COMPONENT_CREATION" => {
                self.observe_recycle_component_creation = value.to_string()
            }
            "RECYCLE_NODE" => self.recycle_node = value.to_string(),
            "COMPONENT_ABOUTTOREUSEINTERNAL_FUNCTION" => {
                self.about_to_reuse_internal = value.to_string()
            }
            "ABOUT_TO_REUSE" => self.about_to_reuse = value.to_string(),
            "COMPONENT_RERENDER_FUNCTION" => self.rerender = value.to_string(),
            "FUNCTION" => self.function_type = value.to_string(),
            "NAME" => self.name = value.to_string(),
            "RESOURCE_NAME_MODULE" => self.module_name = value.to_string(),
            "PAGE_PATH" => self.page_path = value.to_string(),
            "IS_USER_CREATE_STACK" => self.is_user_create_stack = value.to_string(),
            "BUILDER_ATTR_NAME" => self.builder_property = value.to_string(),
            _ => {}
        }
    }
}

struct ComponentMetadataLoader<'a> {
    ets_loader_path: &'a Path,
}

impl<'a> ComponentMetadataLoader<'a> {
    fn new(ets_loader_path: &'a Path) -> Self {
        Self { ets_loader_path }
    }

    fn load(&self) -> anyhow::Result<ComponentMetadata> {
        let atomic_components = self.load_atomic_components()?;
        let component_map_metadata = ComponentMapMetadataLoader::new(
            self.ets_loader_path.join("lib").join("component_map.js"),
        )
        .load()?;
        let bind_rules =
            SdkPreDefineBindingLoader::new(self.ets_loader_path.join("lib").join("pre_define.js"))
                .load()?;
        let enum_names = SdkEnumNameLoader::new(self.ets_loader_path).load()?;
        let sdk_components = SdkComponentLoader::new(self.ets_loader_path).load()?;

        let mut bind_object_properties = bind_rules.bind_object_properties;
        if !component_map_metadata.custom_builder_properties.is_empty() {
            bind_object_properties
                .entry(SdkPreDefineBindingExtractor::ALL_COMPONENTS.to_string())
                .or_default()
                .extend(component_map_metadata.custom_builder_properties);
        }

        Ok(ComponentMetadata {
            atomic_components,
            need_pop_components: component_map_metadata.need_pop_components,
            bind_object_properties,
            create_bind_components: bind_rules.create_bind_components,
            custom_builder_properties_without_key: component_map_metadata
                .custom_builder_properties_without_key,
            style_double_dollar_attrs: bind_rules.style_double_dollar_attrs,
            component_double_dollar_properties: bind_rules.component_double_dollar_properties,
            bind_popup_attrs: bind_rules.bind_popup_attrs,
            bind_drag_attrs: bind_rules.bind_drag_attrs,
            gesture_attr_names: component_map_metadata.gesture_attr_names,
            gesture_type_names: component_map_metadata.gesture_type_names,
            enum_names,
            sdk_components,
            runtime_names: bind_rules.runtime_names,
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

struct SdkComponentLoader<'a> {
    ets_loader_path: &'a Path,
}

impl<'a> SdkComponentLoader<'a> {
    fn new(ets_loader_path: &'a Path) -> Self {
        Self { ets_loader_path }
    }

    fn load(&self) -> anyhow::Result<FxHashMap<String, FxHashMap<String, SdkComponentInfo>>> {
        let api_components = self.load_api_components()?;
        let mut imports = FxHashMap::default();
        for (specifier, components) in &api_components {
            imports.insert(specifier.clone(), components.clone());
        }
        self.load_kit_components(&api_components, &mut imports)?;
        Ok(imports)
    }

    fn load_api_components(
        &self,
    ) -> anyhow::Result<FxHashMap<String, FxHashMap<String, SdkComponentInfo>>> {
        let api_root = self.ets_loader_path.join("..").join("..").join("api");
        let mut paths = Vec::new();
        self.collect_declaration_files(&api_root, &mut paths)?;
        paths.sort();

        let mut components_by_specifier = FxHashMap::default();
        for path in paths {
            let source = fs::read_to_string(&path)?;
            let allocator = Allocator::default();
            let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
            let parsed = Parser::new(&allocator, &source, source_type).parse();
            if !parsed.diagnostics.is_empty() {
                anyhow::bail!(
                    "failed to parse SDK component declaration at {}",
                    path.display()
                );
            }

            let components = SdkComponentDeclarationCollector::new(&parsed.program).collect();
            if components.is_empty() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            for specifier in SdkDeclarationSpecifier::new(file_name).forms() {
                components_by_specifier.insert(specifier, components.clone());
            }
        }
        Ok(components_by_specifier)
    }

    fn load_kit_components(
        &self,
        api_components: &FxHashMap<String, FxHashMap<String, SdkComponentInfo>>,
        imports: &mut FxHashMap<String, FxHashMap<String, SdkComponentInfo>>,
    ) -> anyhow::Result<()> {
        let kit_root = self.ets_loader_path.join("kit_configs");
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
                let Some(info) = SdkDeclarationSpecifier::new(source)
                    .forms()
                    .into_iter()
                    .find_map(|specifier| {
                        api_components
                            .get(&specifier)
                            .and_then(|components| components.get(binding))
                    })
                else {
                    continue;
                };
                imports
                    .entry(kit_specifier.to_string())
                    .or_default()
                    .insert(local_name.clone(), info.clone());
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
            ) && path.extension().and_then(|ext| ext.to_str()) == Some("ets")
            {
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
        let Some(dot_form) = self.without_declaration_suffix() else {
            return Vec::new();
        };
        let mut forms = vec![dot_form.clone()];
        if let Some(colon_form) = Self::colon_form(&dot_form) {
            forms.push(colon_form);
        }
        forms
    }

    fn without_declaration_suffix(&self) -> Option<String> {
        self.raw
            .strip_suffix(".d.ets")
            .or_else(|| self.raw.strip_suffix(".d.ts"))
            .map(str::to_string)
    }

    fn colon_form(dot_form: &str) -> Option<String> {
        let (prefix, module) = dot_form.split_once('.')?;
        if !prefix.starts_with('@') || module.is_empty() {
            return None;
        }
        Some(format!("{prefix}:{module}"))
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
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::StructStatement(statement)) =
                        export.declaration.as_ref()
                    {
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

#[derive(Debug, Default)]
struct PreDefineBindingRules {
    bind_object_properties: FxHashMap<String, FxHashSet<String>>,
    create_bind_components: FxHashSet<String>,
    style_double_dollar_attrs: FxHashSet<String>,
    component_double_dollar_properties: FxHashMap<String, FxHashSet<String>>,
    bind_popup_attrs: FxHashSet<String>,
    bind_drag_attrs: FxHashSet<String>,
    runtime_names: ArkuiRuntimeNames,
}

struct SdkPreDefineBindingLoader {
    pre_define_path: PathBuf,
}

impl SdkPreDefineBindingLoader {
    fn new(pre_define_path: PathBuf) -> Self {
        Self { pre_define_path }
    }

    fn load(&self) -> anyhow::Result<PreDefineBindingRules> {
        let source = fs::read_to_string(&self.pre_define_path)?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, SourceType::cjs()).parse();
        if !parsed.diagnostics.is_empty() {
            anyhow::bail!(
                "failed to parse SDK pre_define.js at {}",
                self.pre_define_path.display()
            );
        }
        Ok(SdkPreDefineBindingExtractor::new(&parsed.program).extract())
    }
}

struct SdkPreDefineBindingExtractor<'a> {
    program: &'a Program<'a>,
    string_bindings: FxHashMap<String, String>,
}

impl<'a> SdkPreDefineBindingExtractor<'a> {
    const ALL_COMPONENTS: &'static str = "AllComponents";
    const BIND_OBJECT_PROPERTY: &'static str = "BIND_OBJECT_PROPERTY";
    const CREATE_BIND_COMPONENT: &'static str = "CREATE_BIND_COMPONENT";
    const STYLE_ADD_DOUBLE_DOLLAR: &'static str = "STYLE_ADD_DOUBLE_DOLLAR";
    const PROPERTIES_ADD_DOUBLE_DOLLAR: &'static str = "PROPERTIES_ADD_DOUBLE_DOLLAR";
    const BIND_POPUP_SET: &'static str = "BIND_POPUP_SET";
    const BIND_DRAG_SET: &'static str = "BIND_DRAG_SET";

    fn new(program: &'a Program<'a>) -> Self {
        let string_bindings = Self::collect_string_bindings(program);
        Self {
            program,
            string_bindings,
        }
    }

    fn extract(&self) -> PreDefineBindingRules {
        let mut rules = PreDefineBindingRules::default();
        for statement in &self.program.body {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            for declarator in &declaration.declarations {
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                match Self::binding_name(&declarator.id) {
                    Some(Self::BIND_OBJECT_PROPERTY) => {
                        if let Some(map) = self.map_literal_string_sets(init) {
                            rules.bind_object_properties = map;
                        }
                    }
                    Some(Self::CREATE_BIND_COMPONENT) => {
                        if let Some(values) = self.set_literal_strings(init) {
                            rules.create_bind_components = values;
                        }
                    }
                    Some(Self::STYLE_ADD_DOUBLE_DOLLAR) => {
                        if let Some(values) = self.set_literal_strings(init) {
                            rules.style_double_dollar_attrs = values;
                        }
                    }
                    Some(Self::PROPERTIES_ADD_DOUBLE_DOLLAR) => {
                        if let Some(map) = self.map_literal_string_sets(init) {
                            rules.component_double_dollar_properties = map;
                        }
                    }
                    Some(Self::BIND_POPUP_SET) => {
                        if let Some(values) = self.set_literal_strings(init) {
                            rules.bind_popup_attrs = values;
                        }
                    }
                    Some(Self::BIND_DRAG_SET) => {
                        if let Some(values) = self.set_literal_strings(init) {
                            rules.bind_drag_attrs = values;
                        }
                    }
                    _ => {}
                }
                self.extract_from_expression(init, &mut rules);
                if let Some(name) = Self::binding_name(&declarator.id) {
                    if let Some(value) = Self::string_literal(init) {
                        rules
                            .runtime_names
                            .set_from_pre_define_binding(name, &value);
                    }
                }
            }
        }
        rules
    }

    fn collect_string_bindings(program: &Program<'_>) -> FxHashMap<String, String> {
        let mut bindings = FxHashMap::default();
        for statement in &program.body {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            for declarator in &declaration.declarations {
                let Some(name) = Self::binding_name(&declarator.id) else {
                    continue;
                };
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                if let Some(value) = Self::string_literal(init) {
                    bindings.insert(name.to_string(), value);
                }
            }
        }
        bindings
    }

    fn extract_from_expression(&self, expr: &Expression<'_>, rules: &mut PreDefineBindingRules) {
        match expr {
            Expression::AssignmentExpression(assignment) => {
                if Self::assignment_target_name(&assignment.left)
                    == Some(Self::BIND_OBJECT_PROPERTY)
                {
                    if let Some(map) = self.map_literal_string_sets(&assignment.right) {
                        rules.bind_object_properties = map;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::CREATE_BIND_COMPONENT)
                {
                    if let Some(values) = self.set_literal_strings(&assignment.right) {
                        rules.create_bind_components = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::STYLE_ADD_DOUBLE_DOLLAR)
                {
                    if let Some(values) = self.set_literal_strings(&assignment.right) {
                        rules.style_double_dollar_attrs = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::PROPERTIES_ADD_DOUBLE_DOLLAR)
                {
                    if let Some(map) = self.map_literal_string_sets(&assignment.right) {
                        rules.component_double_dollar_properties = map;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::BIND_POPUP_SET)
                {
                    if let Some(values) = self.set_literal_strings(&assignment.right) {
                        rules.bind_popup_attrs = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::BIND_DRAG_SET)
                {
                    if let Some(values) = self.set_literal_strings(&assignment.right) {
                        rules.bind_drag_attrs = values;
                    }
                }
                self.extract_from_expression(&assignment.right, rules);
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.extract_from_expression(&parenthesized.expression, rules);
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.extract_from_expression(expression, rules);
                }
            }
            _ => {}
        }
    }

    fn binding_name<'b>(pattern: &'b BindingPattern<'b>) -> Option<&'b str> {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn assignment_target_name<'b>(target: &'b AssignmentTarget<'b>) -> Option<&'b str> {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                Some(identifier.name.as_str())
            }
            AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.as_str()),
            _ => None,
        }
    }

    fn initializer_value<'b>(expr: &'b Expression<'b>) -> Option<&'b Expression<'b>> {
        match expr {
            Expression::AssignmentExpression(assignment) => {
                Self::initializer_value(&assignment.right)
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::initializer_value(&parenthesized.expression)
            }
            Expression::SequenceExpression(sequence) => sequence
                .expressions
                .last()
                .and_then(Self::initializer_value),
            _ => Some(expr),
        }
    }

    fn map_literal_string_sets(
        &self,
        expr: &Expression<'_>,
    ) -> Option<FxHashMap<String, FxHashSet<String>>> {
        let Expression::NewExpression(new_expr) = Self::initializer_value(expr)? else {
            return None;
        };
        let Expression::Identifier(callee) = &new_expr.callee else {
            return None;
        };
        if !ComponentMapRuntimeName::is_map(callee.name.as_str()) {
            return None;
        }
        let Some(Argument::ArrayExpression(entries)) = new_expr.arguments.first() else {
            return None;
        };
        let mut map = FxHashMap::default();
        for entry in &entries.elements {
            let Expression::ArrayExpression(pair) = entry.as_expression()? else {
                return None;
            };
            let key = pair
                .elements
                .first()
                .and_then(ArrayExpressionElement::as_expression)
                .and_then(|expression| self.string_value(expression))?;
            let values = pair
                .elements
                .get(1)
                .and_then(ArrayExpressionElement::as_expression)
                .and_then(|expression| self.set_literal_strings(expression))?;
            map.insert(key, values);
        }
        Some(map)
    }

    fn set_literal_strings(&self, expr: &Expression<'_>) -> Option<FxHashSet<String>> {
        let Expression::NewExpression(new_expr) = Self::initializer_value(expr)? else {
            return None;
        };
        let Expression::Identifier(callee) = &new_expr.callee else {
            return None;
        };
        if !ComponentMapRuntimeName::is_set(callee.name.as_str()) {
            return None;
        }
        let Some(Argument::ArrayExpression(array)) = new_expr.arguments.first() else {
            return None;
        };
        let mut out = FxHashSet::default();
        for element in &array.elements {
            let value = element
                .as_expression()
                .and_then(|expression| self.string_value(expression))?;
            out.insert(value);
        }
        Some(out)
    }

    fn string_value(&self, expr: &Expression<'_>) -> Option<String> {
        match Self::initializer_value(expr)? {
            Expression::Identifier(identifier) => {
                self.string_bindings.get(identifier.name.as_str()).cloned()
            }
            expression => Self::string_literal(expression),
        }
    }

    fn string_literal(expr: &Expression<'_>) -> Option<String> {
        match expr {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            Expression::AssignmentExpression(assignment) => Self::string_literal(&assignment.right),
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::string_literal(&parenthesized.expression)
            }
            Expression::SequenceExpression(sequence) => {
                sequence.expressions.last().and_then(Self::string_literal)
            }
            _ => None,
        }
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
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                        names.insert(declaration.id.name.to_string());
                    }
                }
                _ => {}
            }
        }
        names
    }
}

#[derive(Debug, Default)]
struct ComponentMapMetadata {
    need_pop_components: FxHashSet<String>,
    gesture_attr_names: FxHashSet<String>,
    gesture_type_names: FxHashSet<String>,
    custom_builder_properties: FxHashSet<String>,
    custom_builder_properties_without_key: FxHashSet<String>,
}

struct ComponentMapMetadataLoader {
    component_map_path: PathBuf,
}

impl ComponentMapMetadataLoader {
    fn new(component_map_path: PathBuf) -> Self {
        Self { component_map_path }
    }

    fn load(&self) -> anyhow::Result<ComponentMapMetadata> {
        let source = fs::read_to_string(&self.component_map_path)?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, SourceType::cjs()).parse();
        if !parsed.diagnostics.is_empty() {
            anyhow::bail!(
                "failed to parse SDK component_map.js at {}",
                self.component_map_path.display()
            );
        }
        ComponentMapMetadataExtractor::new(&parsed.program)
            .extract()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to find component metadata in SDK component_map.js at {}",
                    self.component_map_path.display()
                )
            })
    }
}

struct ComponentMapMetadataExtractor<'a> {
    program: &'a Program<'a>,
}

impl<'a> ComponentMapMetadataExtractor<'a> {
    const CUSTOM_BUILDER_PROPERTIES: &'static str = "CUSTOM_BUILDER_PROPERTIES";
    const CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY: &'static str =
        "CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY";
    const GESTURE_ATTRS: &'static str = "GESTURE_ATTRS";
    const GESTURE_TYPE_NAMES: &'static str = "GESTURE_TYPE_NAMES";
    const NEEDPOP_COMPONENT: &'static str = "NEEDPOP_COMPONENT";

    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn extract(&self) -> Option<ComponentMapMetadata> {
        let mut metadata = ComponentMapMetadata::default();
        for statement in &self.program.body {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            for declarator in &declaration.declarations {
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                match Self::binding_name(&declarator.id) {
                    Some(Self::NEEDPOP_COMPONENT) => {
                        if let Some(values) = Self::set_literal_strings(init) {
                            metadata.need_pop_components = values;
                        }
                    }
                    Some(Self::GESTURE_ATTRS) => {
                        if let Some(values) = Self::set_literal_strings(init) {
                            metadata.gesture_attr_names = values;
                        }
                    }
                    Some(Self::GESTURE_TYPE_NAMES) => {
                        if let Some(values) = Self::set_literal_strings(init) {
                            metadata.gesture_type_names = values;
                        }
                    }
                    Some(Self::CUSTOM_BUILDER_PROPERTIES) => {
                        if let Some(values) = Self::set_literal_strings(init) {
                            metadata.custom_builder_properties = values;
                        }
                    }
                    Some(Self::CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY) => {
                        if let Some(values) = Self::set_literal_strings(init) {
                            metadata.custom_builder_properties_without_key = values;
                        }
                    }
                    _ => {}
                }
                Self::extract_from_expression(init, &mut metadata);
            }
        }
        (!metadata.need_pop_components.is_empty()
            || !metadata.gesture_type_names.is_empty()
            || !metadata.custom_builder_properties.is_empty()
            || !metadata.custom_builder_properties_without_key.is_empty())
        .then_some(metadata)
    }

    fn extract_from_expression(expr: &Expression<'_>, metadata: &mut ComponentMapMetadata) {
        match expr {
            Expression::AssignmentExpression(assignment) => {
                if Self::assignment_target_name(&assignment.left) == Some(Self::NEEDPOP_COMPONENT) {
                    if let Some(values) = Self::set_literal_strings(&assignment.right) {
                        metadata.need_pop_components = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::GESTURE_ATTRS)
                {
                    if let Some(values) = Self::set_literal_strings(&assignment.right) {
                        metadata.gesture_attr_names = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::GESTURE_TYPE_NAMES)
                {
                    if let Some(values) = Self::set_literal_strings(&assignment.right) {
                        metadata.gesture_type_names = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::CUSTOM_BUILDER_PROPERTIES)
                {
                    if let Some(values) = Self::set_literal_strings(&assignment.right) {
                        metadata.custom_builder_properties = values;
                    }
                } else if Self::assignment_target_name(&assignment.left)
                    == Some(Self::CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY)
                {
                    if let Some(values) = Self::set_literal_strings(&assignment.right) {
                        metadata.custom_builder_properties_without_key = values;
                    }
                }
                Self::extract_from_expression(&assignment.right, metadata);
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::extract_from_expression(&parenthesized.expression, metadata);
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    Self::extract_from_expression(expression, metadata);
                }
            }
            _ => {}
        }
    }

    fn binding_name<'b>(pattern: &'b BindingPattern<'b>) -> Option<&'b str> {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn assignment_target_name<'b>(target: &'b AssignmentTarget<'b>) -> Option<&'b str> {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                Some(identifier.name.as_str())
            }
            _ => None,
        }
    }

    fn set_literal_strings(expr: &Expression<'_>) -> Option<FxHashSet<String>> {
        if let Expression::ParenthesizedExpression(parenthesized) = expr {
            return Self::set_literal_strings(&parenthesized.expression);
        }
        let Expression::SequenceExpression(sequence) = expr else {
            return Self::set_literal_strings_from_new_set(expr);
        };
        sequence
            .expressions
            .last()
            .and_then(Self::set_literal_strings_from_new_set)
    }

    fn set_literal_strings_from_new_set(expr: &Expression<'_>) -> Option<FxHashSet<String>> {
        let Expression::NewExpression(new_expr) = expr else {
            return None;
        };
        let Expression::Identifier(callee) = &new_expr.callee else {
            return None;
        };
        if !ComponentMapRuntimeName::is_set(callee.name.as_str()) {
            return None;
        }
        let Some(Argument::ArrayExpression(array)) = new_expr.arguments.first() else {
            return None;
        };
        let mut out = FxHashSet::default();
        for element in &array.elements {
            let ArrayExpressionElement::StringLiteral(literal) = element else {
                return None;
            };
            out.insert(literal.value.to_string());
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;

    fn parse_component_map_metadata(source: &str) -> ComponentMapMetadata {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        ComponentMapMetadataExtractor::new(&parsed.program)
            .extract()
            .expect("component metadata")
    }

    #[test]
    fn extracts_need_pop_from_upstream_minified_declarator_shape() {
        let values = parse_component_map_metadata(
            r#"const JS_BIND_COMPONENTS=new Set(["ForEach"]),NEEDPOP_COMPONENT=(exports.JS_BIND_COMPONENTS=JS_BIND_COMPONENTS,new Set(["Blank","Search"]));"#,
        )
        .need_pop_components;
        assert!(values.contains("Blank"));
        assert!(values.contains("Search"));
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn extracts_need_pop_from_assignment_initializer_shape() {
        let values = parse_component_map_metadata(
            r#"const componentMap=(NEEDPOP_COMPONENT=new Set(["Blank","Search"]));"#,
        )
        .need_pop_components;
        assert!(values.contains("Blank"));
        assert!(values.contains("Search"));
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn extracts_gesture_type_names_from_upstream_minified_declarator_shape() {
        let values = parse_component_map_metadata(
            r#"const GESTURE_ATTRS=new Set(["onAction"]),GESTURE_TYPE_NAMES=(exports.GESTURE_ATTRS=GESTURE_ATTRS,new Set(["TapGesture","GestureGroup"]));"#,
        );
        assert!(values.gesture_attr_names.contains("onAction"));
        assert!(values.gesture_type_names.contains("TapGesture"));
        assert!(values.gesture_type_names.contains("GestureGroup"));
        assert_eq!(values.gesture_type_names.len(), 2);
    }

    #[test]
    fn extracts_custom_builder_properties_from_upstream_minified_declarator_shape() {
        let values = parse_component_map_metadata(
            r#"const NEEDPOP_COMPONENT=new Set(["Blank"]),CUSTOM_BUILDER_PROPERTIES=(exports.NEEDPOP_COMPONENT=NEEDPOP_COMPONENT,new Set(["title","navDestination"])),CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY=(exports.CUSTOM_BUILDER_PROPERTIES=CUSTOM_BUILDER_PROPERTIES,new Set(["create"]));"#,
        );
        assert!(values.custom_builder_properties.contains("title"));
        assert!(values.custom_builder_properties.contains("navDestination"));
        assert_eq!(values.custom_builder_properties.len(), 2);
        assert!(values
            .custom_builder_properties_without_key
            .contains("create"));
    }

    #[test]
    fn extracts_double_dollar_and_builder_sets_from_pre_define() {
        let allocator = Allocator::default();
        let source = r#"const STYLE_ADD_DOUBLE_DOLLAR=new Set(["bindSheet"]);
const PROPERTIES_ADD_DOUBLE_DOLLAR=new Map([["TextInput",new Set(["text"])]]);
const BIND_POPUP_SET=new Set(["bindPopup"]);
const BIND_DRAG_SET=new Set(["onDragStart"]);
const BUILDER_PARAM_PROXY="makeBuilderParameterProxy";
const CALL="call";
const $$_THIS="$$this";
const $$_NEW_VALUE="newValue";
const $$_VALUE="value";
const $$_CHANGE_EVENT="changeEvent";"#;
        let parsed = Parser::new(&allocator, source, SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let rules = SdkPreDefineBindingExtractor::new(&parsed.program).extract();
        assert!(rules.style_double_dollar_attrs.contains("bindSheet"));
        assert!(rules
            .component_double_dollar_properties
            .get("TextInput")
            .is_some_and(|properties| properties.contains("text")));
        assert!(rules.bind_popup_attrs.contains("bindPopup"));
        assert!(rules.bind_drag_attrs.contains("onDragStart"));
        assert_eq!(
            rules.runtime_names.builder_param_proxy,
            "makeBuilderParameterProxy"
        );
        assert_eq!(rules.runtime_names.double_dollar_this, "$$this");
        assert_eq!(
            rules.runtime_names.double_dollar_change_event,
            "changeEvent"
        );
    }

    #[test]
    fn extracts_double_dollar_sets_from_sdk_exported_identifier_shape() {
        let allocator = Allocator::default();
        let source = r#"const BIND_POPUP=exports.BIND_POPUP="bindPopup",
BIND_SHEET=exports.BIND_SHEET="bindSheet",
TEXT_INPUT=exports.TEXT_INPUT="TextInput",
TEXT=exports.TEXT="text",
ALL_COMPONENTS=exports.ALL_COMPONENTS="AllComponents",
BIND_OBJECT_PROPERTY=exports.BIND_OBJECT_PROPERTY=new Map([[ALL_COMPONENTS,new Set([BIND_SHEET])]]),
STYLE_ADD_DOUBLE_DOLLAR=exports.STYLE_ADD_DOUBLE_DOLLAR=new Set([BIND_POPUP,BIND_SHEET]),
PROPERTIES_ADD_DOUBLE_DOLLAR=exports.PROPERTIES_ADD_DOUBLE_DOLLAR=new Map([[TEXT_INPUT,new Set([TEXT])]]),
BIND_POPUP_SET=exports.BIND_POPUP_SET=new Set([BIND_POPUP]);"#;
        let parsed = Parser::new(&allocator, source, SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let rules = SdkPreDefineBindingExtractor::new(&parsed.program).extract();
        assert!(rules.style_double_dollar_attrs.contains("bindPopup"));
        assert!(rules.style_double_dollar_attrs.contains("bindSheet"));
        assert!(
            rules
                .component_double_dollar_properties
                .get("TextInput")
                .is_some_and(|properties| properties.contains("text")),
            "SDK exported identifier constants must feed PROPERTIES_ADD_DOUBLE_DOLLAR"
        );
        assert!(
            rules
                .bind_object_properties
                .get("AllComponents")
                .is_some_and(|properties| properties.contains("bindSheet")),
            "SDK exported identifier constants must feed BIND_OBJECT_PROPERTY"
        );
        assert!(rules.bind_popup_attrs.contains("bindPopup"));
    }

    #[test]
    fn loads_sdk_enum_names_from_declarations() {
        let metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ets-loader"
        )))
        .expect("test ets-loader metadata");
        assert!(metadata.is_known_enum("FontWeight"));
        assert!(metadata.is_known_enum("ImageInterpolation"));
        assert_eq!(metadata.runtime_names().component_for_each, "ForEach");
        assert_eq!(
            metadata.runtime_names().component_gesture_group,
            "GestureGroup"
        );
        assert!(metadata.is_gesture_attribute_name("gesture"));
        assert!(metadata.is_gesture_attribute_name("priorityGesture"));
        assert!(metadata.is_builder_attribute_method("Navigation", "navDestination"));
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
    }
}
