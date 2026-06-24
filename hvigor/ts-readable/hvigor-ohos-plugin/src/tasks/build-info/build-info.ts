/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-info/build-info.js
 */
import * as os from "os";
import * as hvigor_1 from "@ohos/hvigor";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as task_names_js_1 from "../common/task-names";
const log = hvigor_1.HvigorLogger.getLogger("no-pattern-info");
export class BuildInfo extends hvigor_1.DefaultTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.BUILD_INFO);
  }
  isProjectNode() {
    const e = this.node.getName();
    return ":" === e || e === this.node.getProject().getName();
  }
  registryAction() {
    return () => {
      const e = hvigor_1.hvigorCore.getParameter().getExtParamsInternal();
      if (e.module && this.isProjectNode()) return;
      let t = {},
        o = this.node;
      const i = "" === e.buildOption,
        r = "" === e.json;
      if (e.module) {
        const r = e.module.split("@")[0],
          n = this.node.findModuleByName(r);
        n && (t = this.getModuleBuildInfo(n, i), o = n);
      } else t = this.getProjectBuildInfo(o, i);
      if (r) log.info(JSON.stringify(t, null, 2));else {
        const e = ["--------------------------------------------------------------------------", `All build information from hvigor node '${o.getName()}'`, "--------------------------------------------------------------------------"],
          i = [];
        this.createFormatInfoRecursively(t, i, 0), e.push(...i), log.info(e.join(os.EOL));
      }
    };
  }
  getModuleBuildInfo(e, t) {
    const o = {},
      i = hvigor_1.ParseJsonFile.parseJsonFile(e.getBuildProfilePath()),
      r = hvigor_1.ParseJsonFile.parseJsonFile(e.getProject().getBuildProfilePath());
    if (i) {
      const n = this.getBuildModes(r),
        l = this.getModuleBuildOption(i, n, e.getName(), t);
      o.buildMode = n, o.buildInfo = l;
    }
    return o;
  }
  getProjectBuildInfo(e, t) {
    const o = {},
      i = hvigor_1.ParseJsonFile.parseJsonFile(e.getBuildProfilePath());
    if (i) {
      const e = this.getProducts(i),
        r = this.getModules(i),
        n = this.getBuildModes(i),
        l = this.getProductBuildOption(i, e, n, t);
      o.product = e, o.module = r, o.buildMode = n, o.buildInfo = l;
    }
    return o;
  }
  createFormatInfoRecursively(e, t, o) {
    if ("string" != typeof e && "boolean" != typeof e) {
      if (e instanceof Array) e.forEach(e => {
        this.createFormatInfoRecursively(e, t, o);
      });else if (e instanceof Object) for (const [i, r] of Object.entries(e)) t.push(this.getPrefix(o) + i), this.createFormatInfoRecursively(r, t, o + 1);
    } else t.push(this.getPrefix(o) + e);
  }
  getPrefix(e) {
    let t = "";
    for (let o = 0; o < e; o++) t += "    ";
    return t;
  }
  getProducts(e) {
    var t;
    const o = [];
    return (null === (t = null == e ? undefined : e.app) || undefined === t ? undefined : t.products) && e.app.products.forEach(e => {
      o.push(null == e ? undefined : e.name);
    }), o;
  }
  getModules(e) {
    const t = [];
    return (null == e ? undefined : e.modules) && e.modules.forEach(e => {
      t.push(null == e ? undefined : e.name);
    }), t;
  }
  getBuildModes(e) {
    var t, o;
    const i = [];
    return null === (o = null === (t = null == e ? undefined : e.app) || undefined === t ? undefined : t.buildModeSet) || undefined === o || o.forEach(e => {
      i.push(null == e ? undefined : e.name);
    }), i;
  }
  getModuleBuildOption(e, t, o, i) {
    const r = {};
    return null == e || e.targets.forEach(e => {
      t.forEach(t => {
        const n = null == e ? undefined : e.name;
        n in r || (r[n] = {}), r[n][t] = i ? build_mode_manager_js_1.buildOptionManager.getTargetBuildOptionWithProductBuildMode(o, n, t) : {};
      });
    }), r;
  }
  getProductBuildOption(e, t, o, i) {
    const r = {};
    t.forEach(e => {
      r[e] = {};
    });
    const n = e.modules;
    return null == n || n.forEach(e => {
      var t;
      null === (t = null == e ? undefined : e.targets) || undefined === t || t.forEach(t => {
        var n;
        null === (n = null == t ? undefined : t.applyToProducts) || undefined === n || n.forEach(n => {
          const l = {
            module: e,
            target: t,
            applyProduct: n
          };
          this.setBuildMode(o, l, r, i);
        });
      });
    }), r;
  }
  setBuildMode(e, t, o, i) {
    e.forEach(e => {
      var r, n;
      const l = t.applyProduct,
        u = null === (r = t.module) || undefined === r ? undefined : r.name,
        s = null === (n = t.target) || undefined === n ? undefined : n.name;
      u in o[l] || (o[l][u] = {}), s in o[l][u] || (o[l][u][s] = {}), o[l][u][s][e] = i ? build_mode_manager_js_1.buildOptionManager.getTargetBuildOptionWithProductBuildMode(u, s, e, l) : {};
    });
  }
}
