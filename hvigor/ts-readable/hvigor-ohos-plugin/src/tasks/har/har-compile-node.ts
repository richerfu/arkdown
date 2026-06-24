/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-compile-node.js
 */
import path_1 from "path";
import * as compile_node_js_1 from "../compile-node";
import * as core_process_package_json_js_1 from "./core-process-package-json";
export var ObfuscateType;
!function (e) {
  e.UGLIFY = "uglify";
}(ObfuscateType = ObfuscateType || (ObfuscateType = {}));
export class HarCompileNode extends compile_node_js_1.CompileNode {
  constructor(e, o, t) {
    super(e, o, t), this.aceModuleBuild = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath());
  }
  declareInputFiles() {
    const e = super.declareInputFiles(),
      o = path_1.resolve(this.moduleModel.getProjectDir(), core_process_package_json_js_1.packageJsonValidMainField);
    return e.addEntry(o), e;
  }
  async doTaskAction() {
    await super.doTaskAction();
  }
  createNodeCommandBuilder() {
    const e = super.createNodeCommandBuilder();
    return e.addObfuscateType(ObfuscateType.UGLIFY), e;
  }
}
