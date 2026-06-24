/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/legacy/legacy-hap-plugin.js
 */
import * as assemble_hap_js_1 from "../../tasks/hook/assemble/assemble-hap";
import * as build_previewer_res_js_1 from "../../tasks/hook/previewer/build-previewer-res";
import * as preview_build_js_1 from "../../tasks/hook/previewer/preview-build";
import * as unit_test_build_js_1 from "../../tasks/unitTest/unit-test-build";
import * as abstract_hap_module_plugin_js_1 from "../common/abstract-hap-module-plugin";
export class LegacyHapPlugin extends abstract_hap_module_plugin_js_1.AbstractHapModulePlugin {
  constructor(e) {
    super(e);
  }
  initHapTasks() {
    this.assembleHap = new assemble_hap_js_1.AssembleHap(this._moduleService, true), this.buildPreviewerRes = new build_previewer_res_js_1.BuildPreviewerRes(this._moduleService, true), this.previewBuild = new preview_build_js_1.PreviewBuild(this._moduleService, true), this.unitTestBuild = new unit_test_build_js_1.UnitTestBuild(this._moduleService, true), this._module.registry(this.assembleHap), this._module.registry(this.buildPreviewerRes), this._module.registry(this.previewBuild), this._module.registry(this.unitTestBuild);
  }
}
