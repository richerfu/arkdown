/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-extend-info.js
 */
export var ArtifactType;
(function (t) {
  t.ORIGINAL = "original", t.OBFUSCATION = "obfuscation", t.BYTECODE = "bytecode";
})(ArtifactType = ArtifactType || (ArtifactType = {}));
export class HarExtendInfo {
  constructor(t, i) {
    this.moduleModel = t, this.artifactType = i.artifactType, this.nativeOption = i.externalNativeOptions, this.buildOpt = i;
  }
  hasConfiguredNativeOption() {
    return undefined !== this.nativeOption;
  }
  isObfuscatedHar() {
    var t, i, e;
    if (this.buildOpt.debuggable) return false;
    const r = null === (e = null === (i = null === (t = this.buildOpt.arkOptions) || undefined === t ? undefined : t.obfuscation) || undefined === i ? undefined : i.ruleOptions) || undefined === e ? undefined : e.enable;
    return r || undefined === r && this.artifactType === ArtifactType.OBFUSCATION;
  }
  getArtifactType() {
    return undefined === this.artifactType ? ArtifactType.ORIGINAL : this.artifactType;
  }
}
