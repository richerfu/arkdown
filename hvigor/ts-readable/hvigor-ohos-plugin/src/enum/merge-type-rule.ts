/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/merge-type-rule.js
 */
export var KeepHapOptions, KeepModuleAttr, KeepAbilityMergeList, KeepOneOfThem, NeedDisposePlaceholder, HasPlaceholderOptions, IsMapFields, UniqueKeyEqualsName, IsNeedMergeRule;
(function (e) {
  e.APP_OPTIONS = "app", e.ARK_OPTIONS = "ark", e.NETWORK_OPTIONS = "network", e.DISTRO_OPTIONS = "distro";
})(KeepHapOptions = KeepHapOptions || (KeepHapOptions = {})), function (e) {
  e.PACKAGE = "package", e.NAME = "name", e.DESCRIPTION = "description", e.DEVICE_TYPE = "deviceType", e.SUPPORTED_MODES = "supportModes", e.SHORTCUTS = "shortcuts", e.DISTRO = "distro";
}(KeepModuleAttr = KeepModuleAttr || (KeepModuleAttr = {})), function (e) {
  e.PERMISSION = "permissions", e.BACKGROUND_MODES = "backgroundModes", e.CONFIG_CHANGES = "configChanges", e.META_DATA = "metaData";
}(KeepAbilityMergeList = KeepAbilityMergeList || (KeepAbilityMergeList = {})), function (e) {
  e.ALLOW_COMPONENTS_PROXY = "allowComponentsProxy", e.ALLOW_CLASS_MAP = "allowClassMap";
}(KeepOneOfThem = KeepOneOfThem || (KeepOneOfThem = {})), function (e) {
  e.PERMISSIONS = "permissions", e.READ_PERMISSIONS = "readPermission", e.WRITE_PERMISSIONS = "writePermission", e.NAME = "name", e.URI = "uri";
}(NeedDisposePlaceholder = NeedDisposePlaceholder || (NeedDisposePlaceholder = {})), function (e) {
  e.ABILITIES = "abilities", e.DEF_PERMISSIONS = "defPermissions", e.DEF_PERMISSION_GROUPS = "defPermissionGroups";
}(HasPlaceholderOptions = HasPlaceholderOptions || (HasPlaceholderOptions = {})), function (e) {
  e.MERGE_RULE = "mergeRule", e.DEVICE_CONFIG = "deviceConfig";
}(IsMapFields = IsMapFields || (IsMapFields = {})), function (e) {
  e.DOMAINS = "domains", e.PARAMETERS = "parameters", e.RESULTS = "results", e.CUSTOMIZE_DATA = "customizeData", e.COMMON_EVENTS = "commonEvents", e.JS = "js", e.REQ_PERMISSIONS = "reqPermissions", e.DEF_PERMISSIONS = "defPermissions", e.DEF_PERMISSION_GROUPS = "defPermissionGroups";
}(UniqueKeyEqualsName = UniqueKeyEqualsName || (UniqueKeyEqualsName = {})), function (e) {
  e.ABILITIES = "abilities", e.COMMON_EVENTS = "commonEvents", e.DEF_PERMISSIONS = "defPermissions", e.DEF_PERMISSION_GROUPS = "defPermissionGroups", e.JS = "js", e.REQ_PERMISSIONS = "reqPermissions", e.CUSTOMIZE_DATA = "customizeData", e.PARAMETERS = "parameters", e.RESULTS = "results";
}(IsNeedMergeRule = IsNeedMergeRule || (IsNeedMergeRule = {}));
