/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-more-info.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as hvigor_console_logger_js_1 from "../../logger/hvigor-console-logger";
import * as hvigor_error_info_js_1 from "../../logger/hvigor-error-info";
export class MergeMoreInfo {
  constructor() {
    this.log = new hvigor_console_logger_js_1.HvigorConsoleLogger("hvigor");
  }
  mergeTwoKey(o, r, e) {
    var _, n, s, g, l, i, t, c, v, L;
    let M;
    if (errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_CN && errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_EN) return o || r ? (o || (o = new hvigor_error_info_js_1.HvigorErrorInfo()), r || (r = new hvigor_error_info_js_1.HvigorErrorInfo()), e.moreInfo === errorlog_consts_js_1.MergeType.COLLECT_ALL ? M = {
      cn: (null === (_ = o.getMoreInfo()) || undefined === _ ? undefined : _.cn) + " " + (null === (n = r.getMoreInfo()) || undefined === n ? undefined : n.cn),
      en: (null === (s = o.getMoreInfo()) || undefined === s ? undefined : s.en) + " " + (null === (g = r.getMoreInfo()) || undefined === g ? undefined : g.en)
    } : e.moreInfo === errorlog_consts_js_1.MergeType.COLLECT_FIRST ? M = {
      cn: (null === (l = o.getMoreInfo()) || undefined === l ? undefined : l.cn) || errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_CN,
      en: (null === (i = o.getMoreInfo()) || undefined === i ? undefined : i.en) || errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_EN
    } : e.moreInfo === errorlog_consts_js_1.MergeType.COLLECT_LAST ? M = {
      cn: (null === (t = r.getMoreInfo()) || undefined === t ? undefined : t.cn) || errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_CN,
      en: (null === (c = r.getMoreInfo()) || undefined === c ? undefined : c.en) || errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_EN
    } : (this.log.printDebug(`Unknown MergeType: ${e.code}. Use default strategy [COLLECT_LAST].`), M = {
      cn: (null === (v = r.getMoreInfo()) || undefined === v ? undefined : v.cn) || errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_CN,
      en: (null === (L = r.getMoreInfo()) || undefined === L ? undefined : L.en) || errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_EN
    }), M) : {
      cn: errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_CN,
      en: errorlog_consts_js_1.DEFAULT_MORE_INFO_URL_EN
    };
  }
}
