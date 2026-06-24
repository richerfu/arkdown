/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/core-project-profile.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as array_util_js_1 from "../../utils/array-util";
import * as project_build_profile_loader_js_1 from "../../utils/loader/file/project-build-profile-loader";
export class CoreProjectProfile {
  constructor(r) {
    this.projectPath = r, this.loadProfile();
  }
  static getInstance(r) {
    return this.coreProjectProfile = new CoreProjectProfile(r), this.coreProjectProfile;
  }
  loadProfile() {
    this.profileOpt = project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile();
  }
  getProjectProfileOpt() {
    return this.loadProfile(), this.profileOpt;
  }
  getCurrentProductOpt() {
    const r = hvigor_1.hvigorCore.getExtraConfig().get("product"),
      e = null != r ? r : "default";
    return (0, array_util_js_1.getElementFromArr)(this.getProjectProfileOpt().app.products, e);
  }
}
