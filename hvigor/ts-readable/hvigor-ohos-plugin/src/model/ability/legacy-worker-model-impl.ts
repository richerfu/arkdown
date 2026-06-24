/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/ability/legacy-worker-model-impl.js
 */
import path_1 from "path";
import * as ability_type_enum_1 from "../../enum/ability-type-enum";
import * as code_type_enum_1 from "../../enum/code-type-enum";
export class LegacyWorkerModelImpl {
  constructor(e) {
    e.length > 0 && (this.workerFilePath = e[0]);
  }
  getConfigJsonJsObj() {}
  getName() {
    return this.workerFilePath ? path_1.basename(this.workerFilePath, path_1.extname(this.workerFilePath)) : "";
  }
  getRelateSrcPath() {
    return this.workerFilePath ? path_1.basename(path_1.dirname(this.workerFilePath)) : "";
  }
  getSrcLanguage() {
    return code_type_enum_1.CodeType.ETS;
  }
  getType() {
    return ability_type_enum_1.AbilityTypeEnum.WORKER;
  }
}
