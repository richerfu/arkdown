/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/core/dependency-interface.js
 */
export var DependencyType, DependencyEnum;
export function compareDependencyEnum(e, n) {
  return n === DependencyEnum.DEV_DEPENDENCIES && e !== DependencyEnum.DEV_DEPENDENCIES;
}
(function (e) {
  e.DEPENDENCY_TYPE_HAR = "har", e.DEPENDENCY_TYPE_HSP = "hsp", e.DEPENDENCY_TYPE_SO = "so", e.DEPENDENCY_TYPE_OTHER = "other", e.DEPENDENCY_TYPE_HAP = "hap";
})(DependencyType = DependencyType || (DependencyType = {})), function (e) {
  e.DEPENDENCIES = "dependencies", e.DYNAMIC_DEPENDENCIES = "dynamicDependencies", e.DEV_DEPENDENCIES = "devDependencies";
}(DependencyEnum = DependencyEnum || (DependencyEnum = {}));
