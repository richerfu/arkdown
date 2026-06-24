/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/har-plugin.js
 */
import * as assemble_har_js_1 from "../tasks/hook/assemble/assemble-har";
import * as build_har_previewer_res_js_1 from "../tasks/hook/previewer/build-har-previewer-res";
import * as preview_build_js_1 from "../tasks/hook/previewer/preview-build";
import * as unit_test_build_js_1 from "../tasks/unitTest/unit-test-build";
import * as abstract_har_module_plugin_js_1 from "./common/abstract-har-module-plugin";
export class HarPlugin extends abstract_har_module_plugin_js_1.AbstractHarModulePlugin {
  constructor(e) {
    super(e);
  }
  initHarTasks() {
    this.assembleHar = new assemble_har_js_1.AssembleHar(this._moduleService, false), this.buildHarPreviewerRes = new build_har_previewer_res_js_1.BuildHarPreviewerRes(this._moduleService, false), this.previewBuild = new preview_build_js_1.PreviewBuild(this._moduleService, false), this.unitTestBuild = new unit_test_build_js_1.UnitTestBuild(this._moduleService, false), this._module.registry(this.assembleHar), this._module.registry(this.buildHarPreviewerRes), this._module.registry(this.previewBuild), this._module.registry(this.unitTestBuild);
  }
}
