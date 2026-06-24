const JS_BIND_COMPONENTS = new Set(["ForEach"]);
const NEEDPOP_COMPONENT = (
  exports.JS_BIND_COMPONENTS = JS_BIND_COMPONENTS,
  new Set(["Blank", "Search"])
);
const GESTURE_ATTRS = new Set([
  "gesture",
  "priorityGesture",
  "parallelGesture",
]);
const GESTURE_TYPE_NAMES = (
  exports.GESTURE_ATTRS = GESTURE_ATTRS,
  new Set([
    "TapGesture",
    "LongPressGesture",
    "PanGesture",
    "PinchGesture",
    "RotationGesture",
    "GestureGroup",
    "SwipeGesture",
  ])
);
const CUSTOM_BUILDER_PROPERTIES = new Set([
  "background",
  "bindPopup",
  "bindMenu",
  "bindContextMenu",
  "title",
  "menus",
  "toolBar",
  "tabBar",
  "onDragStart",
  "onItemDragStart",
  "swipeAction",
  "bindContentCover",
  "bindSheet",
  "navDestination",
  "overlay",
  "toolbarConfiguration",
  "customKeyboard",
  "bindSelectionMenu",
  "description",
  "showUnit",
  "create",
  "addBuilderSpan",
  "dragPreview",
  "accessibilityVirtualNode",
]);
const CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY = new Set([
  "showUnit",
  "create",
]);

exports.NEEDPOP_COMPONENT = NEEDPOP_COMPONENT;
exports.GESTURE_TYPE_NAMES = GESTURE_TYPE_NAMES;
exports.CUSTOM_BUILDER_PROPERTIES = CUSTOM_BUILDER_PROPERTIES;
exports.CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY = CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY;
