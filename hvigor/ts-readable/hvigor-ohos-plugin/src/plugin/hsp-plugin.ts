/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hsp-plugin.js
 */
import * as build_hot_reload_resource_js_1 from "../tasks/debugger/build-hot-reload-resource";
import * as hot_reload_build_js_1 from "../tasks/debugger/hot-reload-build";
import * as assemble_hsp_js_1 from "../tasks/hook/assemble/assemble-hsp";
import * as build_previewer_res_js_1 from "../tasks/hook/previewer/build-previewer-res";
import * as preview_build_js_1 from "../tasks/hook/previewer/preview-build";
import * as unit_test_build_js_1 from "../tasks/unitTest/unit-test-build";
import * as abstract_module_plugin_js_1 from "./common/abstract-module-plugin";
import * as ohos_plugin_id_js_1 from "./common/ohos-plugin-id";
export class HspPlugin extends abstract_module_plugin_js_1.AbstractModulePlugin {
  constructor(e) {
    super(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN, e);
  }
  initHspTasks() {
    this.assembleHsp = new assemble_hsp_js_1.AssembleHsp(this._moduleService, false), this.buildPreviewerRes = new build_previewer_res_js_1.BuildPreviewerRes(this._moduleService, false), this.buildHotReloadRes = new build_hot_reload_resource_js_1.BuildHotReloadResource(this._moduleService), this.previewBuild = new preview_build_js_1.PreviewBuild(this._moduleService, false), this.hotReloadBuild = new hot_reload_build_js_1.HotReloadBuild(this._moduleService), this.unitTestBuild = new unit_test_build_js_1.UnitTestBuild(this._moduleService, false), this._module.registry(this.assembleHsp), this._module.registry(this.buildPreviewerRes), this._module.registry(this.buildHotReloadRes), this._module.registry(this.previewBuild), this._module.registry(this.unitTestBuild), this._module.registry(this.hotReloadBuild);
  }
}
