/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/sdk-util.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import fs_1 from "fs";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as project_build_profile_js_1 from "../options/build/project-build-profile";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
var ApiValType = project_build_profile_js_1.ProjectBuildProfile.ApiValType;
const LOGGER = ohos_logger_js_1.OhosLogger.getLogger("Sdk"),
  ohosPredict = (o, e, r) => {
    const t = new Map(),
      n = path_1.resolve(o, e);
    if (r.filter(o => !fs_1.existsSync(path_1.resolve(n, o, common_const_js_1.CommonConst.OHOS_SDK_COMPONENT_DEFINITION))).length > 0) return t;
    const s = new OhosParser(o),
      i = r.map(o => path_1.resolve(n, o, "oh-uni-package.json"));
    return s.parse(i).map(o => [new sdkmanager_common_1.PathAndApiVersion(o.getPath(), o.getFullApiVersion()), o]).forEach(o => t.set(o[0], o[1])), t;
  };
export class OhosParser extends sdkmanager_common_1.OhLocalComponentLoader {
  constructor(o) {
    super(o);
  }
  parse(o) {
    return this.parseSdks(o);
  }
}
function getApiVersion(o, e) {
  const r = e ? "HO_API_VERSION_VALIDATE_FAILED" : "OH_API_VERSION_VALIDATE_FAILED";
  try {
    return new sdkmanager_common_1.ApiVersion(o);
  } catch (o) {
    LOGGER.printErrorExit(r, undefined, [[path_1.resolve(process.cwd(), "build-profile.json5")]]);
  }
}
export const parseApiVersion = (o, e) => {
  var r;
  if ("number" == typeof o) return {
    api: `${o}`,
    version: o,
    type: ApiValType.NUM,
    fullVersion: o.toString(),
    major: o,
    minor: 0,
    patch: 0
  };
  const t = /\s*([1-9].[0-9].[0-9])\s*\(\s*(\d+)\s*\)\s*$/g.exec(o);
  if (3 === (null == t ? undefined : t.length)) return {
    api: t[1],
    version: parseInt(t[2]),
    type: ApiValType.STRING,
    fullVersion: t[2],
    major: parseInt(t[2]),
    minor: 0,
    patch: 0
  };
  if (e) {
    const e = /\s*([1-9]\.[0-9]\.[0-9])\s*$/g,
      t = o.split("(")[0],
      n = null === (r = o.split("(")[1]) || undefined === r ? undefined : r.split(")")[0],
      s = e.exec(t);
    2 !== (null == s ? undefined : s.length) && LOGGER.printErrorExit("HO_API_VERSION_VALIDATE_FAILED", undefined, [[path_1.resolve(process.cwd(), "build-profile.json5")]]);
    const i = getApiVersion(n, true);
    return {
      api: t,
      version: null == i ? undefined : i.getMajor(),
      type: ApiValType.STRING,
      fullVersion: n,
      major: null == i ? undefined : i.getMajor(),
      minor: null == i ? undefined : i.getMinor(),
      patch: null == i ? undefined : i.getPatch()
    };
  }
  {
    const e = getApiVersion(o, false);
    return {
      api: o,
      version: null == e ? undefined : e.getMajor(),
      type: ApiValType.STRING,
      fullVersion: o,
      major: null == e ? undefined : e.getMajor(),
      minor: null == e ? undefined : e.getMinor(),
      patch: null == e ? undefined : e.getPatch()
    };
  }
};
export const proxyFun = () => {
  var o, e, r;
  const t = hvigor_1.GlobalParam.getInstance();
  return {
    proxy: t.getConfig("proxy"),
    httpsProxy: t.getConfig("https-proxy"),
    noProxy: null !== (r = null !== (e = null !== (o = t.getConfig("noproxy")) && undefined !== o ? o : t.getConfig("no-proxy")) && undefined !== e ? e : process.env.NO_PROXY) && undefined !== r ? r : process.env.npm_config_noproxy
  };
};
export const contains = (o, e) => {
  for (const [r, t] of e) if (r === o) return t;
};
export const handleSdkException = o => {
  if (!(o instanceof sdkmanager_common_1.SdkException)) throw o;
  {
    const e = o.getErrorTip();
    LOGGER._buildError(e.getReason())._solution(`${e.getFixSuggestion()}`)._printErrorAndExit();
  }
};
export { ohosPredict };
