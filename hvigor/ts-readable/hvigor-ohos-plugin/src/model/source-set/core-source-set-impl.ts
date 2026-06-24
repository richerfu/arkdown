/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/source-set/core-source-set-impl.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as code_model_impl_js_1 from "../code-source/code-model-impl";
export class CoreSourceSetImpl {
  constructor(e, t) {
    this.sourceSetRoot = e, this.codeMap = this.initCodeMap(), this.targetResModel = t;
  }
  getSourceSetRoot() {
    return this.sourceSetRoot;
  }
  getCodeMap() {
    return this.codeMap;
  }
  getTargetResPath() {
    return this.targetResModel.getResourcePath();
  }
  initCodeMap() {
    const e = new Map();
    for (const t of Object.values(code_type_enum_js_1.CodeType)) {
      if ("string" != typeof t) continue;
      const o = t.toString().toLowerCase(),
        r = path_1.resolve(this.sourceSetRoot, o);
      fs_1.existsSync(r) && e.set(t, new code_model_impl_js_1.CodeModelImpl(r, t));
    }
    return e;
  }
}
