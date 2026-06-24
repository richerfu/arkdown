/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/opt-compression-builder.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as glob_1 from "glob";
import * as minimatch_1 from "minimatch";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as copy_resources_util_js_1 from "../utils/copy-resources-util";
const _log = hvigor_1.HvigorLogger.getLogger("opt-compression-builder"),
  UNIT_TO_SIZE_NUMBER = Object.freeze({
    k: 1024,
    K: 1024,
    m: 1048576,
    M: 1048576,
    g: 1073741824,
    G: 1073741824,
    t: 1099511627776,
    T: 1099511627776
  });
export class OptCompressionBuilder {
  constructor() {
    this.optCompressionObj = {
      context: {
        extensionPath: ""
      },
      compression: {
        media: {
          enable: false
        },
        sizeLimit: undefined,
        filters: []
      }
    }, this.allResourceFilesAbsolutePath = [];
  }
  setResourceDirArr(e) {
    const o = e.map(e => path_1.resolve(e, common_const_js_1.CommonConst.GLOB_MATCH_ALL_FILES));
    return this.allResourceFilesAbsolutePath = glob_1.glob.sync(o, {
      nodir: true,
      absolute: true,
      windowsPathsNoEscape: true
    }), this;
  }
  setExtensionPath(e) {
    return this.optCompressionObj.context.extensionPath = null != e ? e : "", this;
  }
  setCompressionConfig(e) {
    var o, i, t;
    return this.optCompressionObj.compression.filters = this.convertFilters(null !== (o = null == e ? undefined : e.filters) && undefined !== o ? o : []), undefined !== (null === (i = null == e ? undefined : e.media) || undefined === i ? undefined : i.enable) && (this.optCompressionObj.compression.media = e.media), undefined !== (null === (t = null == e ? undefined : e.sizeLimit) || undefined === t ? undefined : t.ratio) && (this.optCompressionObj.compression.sizeLimit = e.sizeLimit), this.checkMediaFilter(), this;
  }
  getOptCompressionObj() {
    return this.optCompressionObj;
  }
  processSize(e) {
    return null == e ? undefined : e.map(e => e.map(e => {
      if ("number" == typeof e) return e;
      const o = e.length - 1,
        i = e.charAt(o);
      if (Object.prototype.hasOwnProperty.call(UNIT_TO_SIZE_NUMBER, i)) {
        const t = UNIT_TO_SIZE_NUMBER[i];
        return parseInt(e.slice(0, o)) * t;
      }
      return parseInt(e);
    })).filter(([e, o = Number.POSITIVE_INFINITY]) => !(e > o) || (_log.warn("Invalid compression filter size will be ignored: ", [e, o]), false)).sort((e, o) => e[0] - o[0]).reduce((e, o) => {
      const i = e.length > 0 ? e[e.length - 1] : undefined;
      return i && 1 === i.length ? e : !i || i[1] < o[0] - 1 ? (e.push(o), e) : 1 === o.length ? (e[e.length - 1] = [i[0]], e) : (i[1] = Math.max(i[1], o[1]), e);
    }, []);
  }
  processResolution(e) {
    return null == e ? undefined : e.filter(e => {
      const [o, i = {
        width: Number.POSITIVE_INFINITY,
        height: Number.POSITIVE_INFINITY
      }] = e;
      return !(o.width > i.width || o.height > i.height) || (_log.warn("Invalid compression filter resolution will be ignored: ", [...e]), false);
    });
  }
  getPathFromResourceFiles(e) {
    if (e) return this.allResourceFilesAbsolutePath.filter(o => e.some(e => {
      const i = (0, copy_resources_util_js_1.toUnixPath)(e);
      return (0, minimatch_1.minimatch)(o, i);
    }));
  }
  convertRules(e) {
    if (e && (e.size || e.resolution)) return {
      size: this.processSize(e.size),
      resolution: this.processResolution(e.resolution)
    };
  }
  convertFilters(e) {
    return e.map(e => {
      var o, i, t, s, r;
      const l = {
        method: (0, wdk_1.cloneDeep)(e.method),
        path: this.getPathFromResourceFiles(null === (o = e.files) || undefined === o ? undefined : o.path),
        exclude_path: this.getPathFromResourceFiles(null === (i = e.exclude) || undefined === i ? undefined : i.path),
        rules_origin: this.convertRules(e.files),
        rules_exclude: this.convertRules(e.exclude)
      };
      return (null === (t = e.exclude) || undefined === t ? undefined : t.path) || (null === (s = e.exclude) || undefined === s ? undefined : s.size) || (null === (r = e.exclude) || undefined === r ? undefined : r.resolution) || (l.exclude_path = []), l;
    });
  }
  checkMediaFilter() {
    var e, o, i;
    const t = null !== (o = null === (e = this.optCompressionObj.compression.media) || undefined === e ? undefined : e.enable) && undefined !== o && o,
      s = !(null === (i = this.optCompressionObj.compression.filters) || undefined === i ? undefined : i.length);
    t && s && this.optCompressionObj.compression.filters.push({
      method: {
        type: "sut",
        blocks: "4x4"
      }
    });
  }
}
