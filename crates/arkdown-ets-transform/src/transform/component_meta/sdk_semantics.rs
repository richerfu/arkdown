use rustc_hash::{FxHashMap, FxHashSet};

/// ArkUI lowering semantics bundled with arkdown.
///
/// Generated from DevEco Studio 6.1.1.280 / OpenHarmony ETS 6.1.1.125
/// (API 24, Release):
///
/// - `component_map.js`
///   SHA-256 `8a0f96d842218b30a42c705369f8e3502d6ee3edf10c7ff87dc56cec1569c4ab`
/// - `pre_define.js`
///   SHA-256 `49ffdca1752addb800f74dffc44f04a966da618168bb3de4c9daf91a97800763`
///
/// The source modules were evaluated once while generating this table. They
/// are not read or executed by production builds.
pub(super) struct SdkArkUiSemanticSnapshot {
    pub(super) need_pop_components: FxHashSet<String>,
    pub(super) gesture_attr_names: FxHashSet<String>,
    pub(super) gesture_type_names: FxHashSet<String>,
    pub(super) custom_builder_properties: FxHashSet<String>,
    pub(super) custom_builder_properties_without_key: FxHashSet<String>,
    pub(super) bind_object_properties: FxHashMap<String, FxHashSet<String>>,
    pub(super) create_bind_components: FxHashSet<String>,
    pub(super) style_double_dollar_attrs: FxHashSet<String>,
    pub(super) component_double_dollar_properties: FxHashMap<String, FxHashSet<String>>,
    pub(super) bind_popup_attrs: FxHashSet<String>,
    pub(super) bind_drag_attrs: FxHashSet<String>,
}

impl SdkArkUiSemanticSnapshot {
    pub(super) const ALL_COMPONENTS: &'static str = "AllComponents";

    pub(super) fn bundled() -> Self {
        Self {
            need_pop_components: Self::set(NEED_POP_COMPONENTS),
            gesture_attr_names: Self::set(GESTURE_ATTR_NAMES),
            gesture_type_names: Self::set(GESTURE_TYPE_NAMES),
            custom_builder_properties: Self::set(CUSTOM_BUILDER_PROPERTIES),
            custom_builder_properties_without_key: Self::set(CUSTOM_BUILDER_PROPERTIES_WITHOUT_KEY),
            bind_object_properties: Self::set_map(BIND_OBJECT_PROPERTIES),
            create_bind_components: Self::set(CREATE_BIND_COMPONENTS),
            style_double_dollar_attrs: Self::set(STYLE_DOUBLE_DOLLAR_ATTRS),
            component_double_dollar_properties: Self::set_map(COMPONENT_DOUBLE_DOLLAR_PROPERTIES),
            bind_popup_attrs: Self::set(BIND_POPUP_ATTRS),
            bind_drag_attrs: Self::set(BIND_DRAG_ATTRS),
        }
    }

    fn set(values: &[&str]) -> FxHashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn set_map(entries: &[(&str, &[&str])]) -> FxHashMap<String, FxHashSet<String>> {
        entries
            .iter()
            .map(|(key, values)| ((*key).to_string(), Self::set(values)))
            .collect()
    }
}

const NEED_POP_COMPONENTS: &[&str] = &["Blank", "Search"];

const GESTURE_ATTR_NAMES: &[&str] = &["gesture", "parallelGesture", "priorityGesture"];

const GESTURE_TYPE_NAMES: &[&str] = &[
    "GestureGroup",
    "LongPressGesture",
    "PanGesture",
    "PinchGesture",
    "RotationGesture",
    "SwipeGesture",
    "TapGesture",
];

const CUSTOM_BUILDER_PROPERTIES: &[&str] = &[
    "accessibilityVirtualNode",
    "addBuilderSpan",
    "background",
    "bindContentCover",
    "bindContextMenu",
    "bindContextMenuWithResponse",
    "bindMenu",
    "bindPopup",
    "bindSelectionMenu",
    "bindSheet",
    "create",
    "customKeyboard",
    "description",
    "dragPreview",
    "menus",
    "navDestination",
    "onDragStart",
    "onItemDragStart",
    "overlay",
    "showUnit",
    "swipeAction",
    "tabBar",
    "title",
    "toolBar",
    "toolbar",
    "toolbarConfiguration",
];

const CUSTOM_BUILDER_PROPERTIES_WITHOUT_KEY: &[&str] = &["create", "showUnit"];

const BIND_OBJECT_PROPERTIES: &[(&str, &[&str])] = &[
    (
        "AllComponents",
        &[
            "bindContextMenu",
            "bindContextMenuWithResponse",
            "bindMenu",
            "bindSheet",
            "dragPreview",
        ],
    ),
    ("ArcListItem", &["swipeAction"]),
    ("Checkbox", &["create"]),
    ("ListItem", &["swipeAction"]),
    ("MenuItem", &["create"]),
    ("MenuItemGroup", &["create"]),
    ("NavDestination", &["title"]),
    ("Navigation", &["title"]),
    ("Radio", &["create"]),
    ("Refresh", &["create"]),
    ("WaterFlow", &["create"]),
    ("Web", &["bindSelectionMenu"]),
];

const CREATE_BIND_COMPONENTS: &[&str] = &["ListItemGroup", "Refresh"];

const STYLE_DOUBLE_DOLLAR_ATTRS: &[&str] =
    &["bindContentCover", "bindPopup", "bindSheet", "visibility"];

const COMPONENT_DOUBLE_DOLLAR_PROPERTIES: &[(&str, &[&str])] = &[
    ("AlphabetIndexer", &["selected"]),
    ("Calendar", &["date"]),
    ("Checkbox", &["select"]),
    ("CheckboxGroup", &["selectAll"]),
    ("DatePicker", &["selected"]),
    ("GridItem", &["selected"]),
    ("ListItem", &["selected"]),
    ("MenuItem", &["selected"]),
    ("Panel", &["mode"]),
    ("Radio", &["checked"]),
    ("Rating", &["rating"]),
    ("Refresh", &["refreshing"]),
    ("Search", &["value"]),
    ("Select", &["selected", "value"]),
    ("SideBarContainer", &["showSideBar"]),
    ("Slider", &["value"]),
    ("Stepper", &["index"]),
    ("Swiper", &["index"]),
    ("Tabs", &["index"]),
    ("TextArea", &["text"]),
    ("TextInput", &["text"]),
    ("TextPicker", &["selected", "value"]),
    ("TextTimer", &["count", "format", "isCountDown"]),
    ("TimePicker", &["selected"]),
    ("Toggle", &["isOn"]),
];

const BIND_POPUP_ATTRS: &[&str] = &["bindPopup"];

const BIND_DRAG_ATTRS: &[&str] = &["onDragStart", "onItemDragStart"];
