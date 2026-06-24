/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/generate-native-library.js
 */
import path_1 from "path";
import * as native_library_model_js_1 from "./native-library-model";
export class GenerateNativeLibrary {
  static parseLibrary(t, i, e, r) {
    var s, a;
    const o = new native_library_model_js_1.NativeLibraryModel(i.name, i.id, i.type);
    return o.setAbi(t), o.setBuildType(e), o.setFolders(this.findFolders(i)), o.setFiles(this.findFiles(i)), o.setOutputs(this.findOutputs(i)), o.setNameOnDisk(i.nameOnDisk), o.setRuntimeFiles(this.findRuntimeFiles(i, r)), o.setBuild(null === (s = i.paths) || undefined === s ? undefined : s.build), o.setSource(null === (a = i.paths) || undefined === a ? undefined : a.source), o;
  }
  static findFolders(t) {
    var i;
    const e = [];
    return null === (i = t.compileGroups) || undefined === i || i.forEach(t => {
      var i;
      null === (i = t.includes) || undefined === i || i.forEach(t => {
        t.path && e.push(t.path);
      });
    }), e;
  }
  static findFiles(t) {
    var i;
    const e = [];
    return null === (i = t.sources) || undefined === i || i.forEach(i => {
      var r;
      const s = null !== (r = t.sourceGroups) && undefined !== r ? r : [],
        a = undefined === i.sourceGroupIndex ? undefined : s[i.sourceGroupIndex];
      a && "Source Files" !== a.name && i.path && e.push(i.path);
    }), e;
  }
  static findOutputs(t) {
    var i;
    const e = [];
    return null === (i = t.artifacts) || undefined === i || i.forEach(t => {
      t.path && e.push(t.path);
    }), e;
  }
  static findRuntimeFiles(t, i) {
    var e, r, s, a;
    const o = [];
    let l = null === (r = null === (e = t.link) || undefined === e ? undefined : e.sysroot) || undefined === r ? undefined : r.path;
    return l && (l = path_1.resolve(l)), null === (a = null === (s = t.link) || undefined === s ? undefined : s.commandFragments) || undefined === a || a.forEach(e => {
      if ("libraries" !== e.role) return;
      let r = e.fragment;
      r && (r.startsWith("-") || r.endsWith(".a") || (!path_1.isAbsolute(r) || l && r.startsWith(l) ? this.handleBuildDir(t, i, r, l, o) : o.push(r)));
    }), o;
  }
  static handleBuildDir(t, i, e, r, s) {
    var a, o;
    for (const l of [null === (a = t.paths) || undefined === a ? undefined : a.build, null === (o = null == i ? undefined : i.paths) || undefined === o ? undefined : o.build]) if (l) {
      const t = path_1.join(l, e);
      if (r && t.startsWith(r)) continue;
      s.push(t);
    }
  }
}
