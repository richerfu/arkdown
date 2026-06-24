/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/har-dedepulicate/collect_duplicate_har_dependencies.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_plugin_id_js_1 from "../plugin/common/ohos-plugin-id";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("HarDeduplicate");
let _hspNeedDeduplicateHarMap, _mergedHap, _mergedHapAllHspLocalDependencies;
export function processHarDeduplicate(e) {
  var n, t, o, a;
  (null === (o = null === (t = null === (n = e.getTaskService()) || undefined === n ? undefined : n.getBuildOption()) || undefined === t ? undefined : t.packOptions) || undefined === o ? undefined : o.deduplicateHar) && !(null === (a = e.getTaskService()) || undefined === a ? undefined : a.isFaMode()) && (checkHasOnlyOneHap(e), collectDuplicateHarInfos(e._project));
}
export function getMergedHap() {
  return _mergedHap;
}
export function getMergedHapAllHspDependencies() {
  return _mergedHapAllHspLocalDependencies;
}
export function getHspNeedDeduplicateHarInfoMap(e) {
  if (_hspNeedDeduplicateHarMap) return _hspNeedDeduplicateHarMap[e];
}
export function checkHasOnlyOneHap(e) {
  var n, t;
  const o = null === (n = e.getTaskService()) || undefined === n ? undefined : n.getTargetProduct().name,
    a = null === (t = e.getTaskService()) || undefined === t ? undefined : t.getProductDataMap();
  if (!o || !a) return;
  const s = [];
  for (const e of a.values()) if (e.length > 0) {
    e[0].getModuleModel().isHapModule() && s.push(e[0].getModuleModel().getName());
  }
  s.length > 1 && _log.printErrorExit("HAR_DEDUPLICATE_SUPPORT_ONLY_ONE_HAP", [s.length, s.join(","), o]);
}
export function collectDuplicateHarInfos(e) {
  const n = {},
    t = {},
    o = {};
  let a, s;
  if (e.getAllSubModules().forEach(e => {
    const d = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN),
      c = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN);
    if (d) {
      const e = d.getModuleModel();
      e && (a = e.getName(), s = e.getOhPackageJson5Path()), processHap(d, t, o, n);
    } else c && processHsp(c, t, o, n);
  }), _log.debug(`entryHap ${a} oh-package.json5 ${s}`), !a || !s) return;
  _mergedHap = a;
  const d = hvigor_1.MetricFactory.createDurationEvent("collect duplicate hars", "Collect duplicate hars", hvigor_1.HvigorTaskGroupType.SYNC_TASK_GROUP);
  d.start(), markNeedDeduplicateHar(a, s, _mergedHapAllHspLocalDependencies, t, o, n), d.stop();
}
function processHap(e, n, t, o) {
  var a, s, d, c, i, l, p;
  const r = null === (a = e.getModuleModel()) || undefined === a ? undefined : a.getName(),
    g = null === (s = e.getTaskService()) || undefined === s ? undefined : s.getAllDependencies().filter(e => !e.isHspDependency()),
    u = null === (d = e.getTaskService()) || undefined === d ? undefined : d.getAllDirectDependencies().filter(e => !e.isHspDependency()),
    H = null === (c = e.getTaskService()) || undefined === c ? undefined : c.getHspModuleDependencyNames();
  g && u && H && r && (n[r] = new Set(g.map(e => {
    const n = e.getDependencyRootPath();
    return o[n] = e, n;
  })), t[r] = new Set(u.map(e => e.getDependencyRootPath())), _mergedHapAllHspLocalDependencies = null !== (p = null === (l = null === (i = e.getTaskService()) || undefined === i ? undefined : i.getAllHspDependencies()) || undefined === l ? undefined : l.filter(e => e.isLocal()).map(e => {
    var n, t;
    return null !== (t = null === (n = e.getModuleJsonObj()) || undefined === n ? undefined : n.module.name) && undefined !== t ? t : "";
  })) && undefined !== p ? p : []);
}
function processHsp(e, n, t, o) {
  var a, s, d, c;
  const i = null === (a = e.getTaskService()) || undefined === a ? undefined : a.getAllDependencies().filter(e => !e.isHspDependency()),
    l = null === (s = e.getTaskService()) || undefined === s ? undefined : s.getAllDirectDependencies().filter(e => !e.isHspDependency()),
    p = null === (d = e.getTaskService()) || undefined === d ? undefined : d.getHspModuleDependencyNames(),
    r = null === (c = e.getModuleModel()) || undefined === c ? undefined : c.getName();
  i && l && p && r && (n[r] = new Set(i.map(e => {
    const n = e.getDependencyRootPath();
    return o[n] = e, n;
  })), t[r] = new Set(l.map(e => e.getDependencyRootPath())));
}
function getIntersection(e, n) {
  const [t, o] = e.size <= n.size ? [e, n] : [n, e];
  return new Set([...t].filter(e => o.has(e)));
}
function markNeedDeduplicateHar(e, n, t, o, a, s) {
  const d = o[e],
    c = a[e],
    i = {},
    l = new Set();
  for (let n = 0; n < t.length; n++) {
    const p = t[n];
    i[p] = new Set();
    const r = o[p],
      g = a[p];
    if (!r || !g) continue;
    getIntersection(d, r).forEach(n => {
      g.has(n) && !c.has(n) && l.add(`${s[n].getPackageName()}: the common dependency of ${e} and ${p} must be declared in dependencies of ${e}`), i[p].add(n);
    });
  }
  for (let n = 0; n < t.length; n++) for (let d = n + 1; d < t.length; d++) {
    const p = t[n],
      r = t[d],
      g = o[p],
      u = o[r],
      H = a[p],
      D = a[r];
    getIntersection(g, u).forEach(n => {
      (H.has(n) || D.has(n)) && !c.has(n) && l.add(`${s[n].getPackageName()}: the common dependency of ${p} and ${r} must be declared in dependencies of ${e}`), i[p].add(n), i[r].add(n);
    });
  }
  l.size > 0 && _log.printErrorExit("HAR_DEDUPLICATE_DEPENDENCY_CHECK_ERROR", [[...l].join("\n")], [[e, n]]);
  const p = {};
  Object.keys(i).forEach(e => {
    p[e] = {};
    i[e].forEach(n => {
      p[e][n] = makeDependencyObject(s[n]);
    });
  }), _log.debug("hspNeedDeduplicateHarMap", p), _hspNeedDeduplicateHarMap = p;
}
function makeDependencyObject(e) {
  return {
    packageName: e.getPackageName(),
    dependencyRootPath: e.getDependencyRootPath(),
    isModuleDependency: e.isModuleDependency(),
    isLocal: e.isLocal(),
    isByteCodeHarDependency: e.isByteCodeHarDependency(),
    isOtherDependency: e.isOtherDependency(),
    isSODependency: e.isSODependency(),
    isHspDependency: e.isHspDependency(),
    isHarDependency: e.isHarDependency()
  };
}
