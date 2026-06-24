/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/project-build-profile-loader.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as class_type_const_js_1 from "@ohos/hvigor/src/base/common/options/class-type-const";
import * as common_const_js_1 from "../../../const/common-const";
import * as abstract_build_profile_loader_js_1 from "./abstract-build-profile-loader";
class ProjectBuildProfileLoader extends abstract_build_profile_loader_js_1.AbstractBuildProfileLoader {
  constructor() {
    super(...arguments), this.logger = hvigor_1.HvigorLogger.getLogger(ProjectBuildProfileLoader.name);
  }
  getBuildProfile() {
    return this.getBuildProfileJson5Obj(hvigor_1.hvigor.getRootNodeInternal().getNodeNameInternal(), true, class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT);
  }
  setBuildProfileJson5Obj(e, o, r = true) {
    super.setBuildProfileJson5ObjCheck(e, o, common_const_js_1.BuildProfileSchemaFileConst.PROJECT_BUILD_PROFILE_SCHEMA_PATH, class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT);
  }
  initializeReplace(e) {
    return this.replaceTargets(e);
  }
  replaceTargets(e) {
    const o = (0, wdk_1.cloneDeep)(e);
    return hvigor_1.hvigor.getRootNodeInternal().subNodesInternal(e => {
      var r;
      if (!o.modules.some(o => o.name === e.getNodeNameInternal())) {
        const t = null === (r = hvigor_1.hvigorConfig.getNodeDescriptorByNameInternal(e.getNodeNameInternal())) || undefined === r ? undefined : r.srcPath;
        o.modules.push({
          name: e.getNodeNameInternal(),
          srcPath: null != t ? t : e.getNodePathInternal()
        });
      }
      const t = e.getExtraOptionInternal("targets");
      if (!t) return;
      o.modules.forEach(o => {
        o.name === e.getNodeNameInternal() && (o.targets = t);
      });
    }), o.modules = o.modules.filter(e => hvigor_1.hvigor.getAllNodesInternal().some(o => o.getNodeNameInternal() === e.name)), o;
  }
  checkBuildProfile(e, o) {
    if (!e.app.products || !o.app.products || e.app.products.length !== o.app.products.length) return void this.logger.printErrorExit("CAN_NOT_ADD_OR_DELETE_PRODUCTS", ["build-profile.json5"]);
    this.checkMaxFlowDepth(o) && this.logger.warn("Setting maxFlowDepth in hvigorfile.ts is not supported and may cause side effects.");
    const r = e.app.products,
      t = o.app.products;
    r.forEach(e => {
      const o = t.find(o => o.name === e.name);
      o ? o.runtimeOS !== e.runtimeOS && this.logger.printErrorExit("CAN_NOT_CHANGE_NAME_OR_RUNTIME_PRODUCTS", [e.runtimeOS]) : this.logger.printErrorExit("CAN_NOT_CHANGE_NAME_OR_RUNTIME_PRODUCTS", [e.name]);
    });
  }
  checkMaxFlowDepth(e) {
    var o, r, t, i, l, n, s, a;
    for (const l of (null === (o = null == e ? undefined : e.app) || undefined === o ? undefined : o.products) || []) if (undefined !== (null === (i = null === (t = null === (r = null == l ? undefined : l.buildOption) || undefined === r ? undefined : r.arkOptions) || undefined === t ? undefined : t.tscConfig) || undefined === i ? undefined : i.maxFlowDepth)) return true;
    for (const o of (null === (l = null == e ? undefined : e.app) || undefined === l ? undefined : l.buildModeSet) || []) if (undefined !== (null === (a = null === (s = null === (n = null == o ? undefined : o.buildOption) || undefined === n ? undefined : n.arkOptions) || undefined === s ? undefined : s.tscConfig) || undefined === a ? undefined : a.maxFlowDepth)) return true;
    return false;
  }
}
export const projectBuildProfileLoader = new ProjectBuildProfileLoader();
