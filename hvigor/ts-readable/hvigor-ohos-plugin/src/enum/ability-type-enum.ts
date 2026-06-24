/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/ability-type-enum.js
 */
export var AbilityTypeEnum;
(function (e) {
  e.PAGE = "page", e.DATA = "data", e.SERVICE = "service", e.FORM = "form", e.TEST_RUNNER = "testrunner", e.WORKER = "worker";
})(AbilityTypeEnum = AbilityTypeEnum || (AbilityTypeEnum = {})), function (e) {
  e.valueOf = function (t) {
    return "page" === t.type ? e.PAGE : "data" === t.type ? e.DATA : t.formsEnabled ? e.FORM : e.SERVICE;
  };
}(AbilityTypeEnum = AbilityTypeEnum || (AbilityTypeEnum = {}));
