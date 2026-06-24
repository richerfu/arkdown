/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/report/report-service-impl.js
 */
import fs_1 from "fs";
import path_1 from "path";
import process_1 from "process";
import fs_extra_1 from "fs-extra";
import * as local_file_writer_js_1 from "../../../common/util/local-file-writer";
import * as path_util_js_1 from "../../../common/util/path-util";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
export class ReportServiceImpl {
  constructor() {
    this.reportListeners = [];
  }
  report() {
    const e = this.getReport(),
      t = path_util_js_1.PathUtil.getReportDirPath();
    fs_1.existsSync(t) || fs_1.mkdirSync(t, {
      recursive: true
    }), this.deleteUnusableFiles(t), this.storage(e, t);
  }
  getReport() {
    const e = {
      version: "2.0",
      ppid: process_1.ppid
    };
    for (const t of this.reportListeners) {
      const r = t.queryReport();
      e[r.getName()] = r.getValue();
    }
    return e;
  }
  storage(e, t) {
    const r = fs_1.readdirSync(t).filter(e => e.startsWith("report-") && e.endsWith("json")).sort((e, r) => {
      const s = path_1.resolve(t, e),
        o = path_1.resolve(t, r),
        i = fs_1.statSync(s);
      return fs_1.statSync(o).birthtimeMs - i.birthtimeMs;
    });
    for (let e = 0; e < r.length; e++) if (e >= 9) {
      const s = path_1.resolve(t, r[e]);
      fs_1.existsSync(s) && fs_1.unlinkSync(s);
    }
    const s = require("../../internal/data/global-data.js");
    if (undefined === s.globalData.buildId) return;
    const o = s.globalData.buildId;
    ReportServiceImpl.buildId = o;
    const i = path_1.resolve(t, `report-${o}.json`);
    local_file_writer_js_1.LocalFileWriter.getInstance().write(i, e), enableHtmlGenerate() && this.generateHtmlResource(t, `report-${o}`, e);
  }
  deleteUnusableFiles(e) {
    fs_1.readdirSync(e).forEach(t => {
      if (!ReportServiceImpl.REPORT_REG.test(t) && (!ReportServiceImpl.HTML_REG.test(t) || enableHtmlGenerate()) && t !== ReportServiceImpl.HTML_RESOURCE_NAME && t !== ReportServiceImpl.UPLOAD_NAME) {
        const r = path_1.resolve(e, t);
        fs_1.existsSync(r) && fs_1.unlinkSync(r);
      }
    });
  }
  addListener(e) {
    this.reportListeners.push(e);
  }
  removeListener(e) {
    const t = this.reportListeners.indexOf(e);
    -1 !== t && this.reportListeners.splice(t, 1);
  }
  generateHtmlResource(e, t, r) {
    const s = path_1.resolve(e, "htmlResource"),
      o = path_1.resolve(__filename, "../../../../../res/staticHtmlResource/htmlResource");
    if (fs_1.existsSync(s)) {
      const e = fs_1.readdirSync(o),
        t = fs_1.readdirSync(s);
      e.every(e => {
        if (!t.includes(e)) return false;
        return fs_1.statSync(path_1.resolve(o, e)).size === fs_1.statSync(path_1.resolve(s, e)).size;
      }) || fs_extra_1.copySync(o, s);
    } else fs_extra_1.copySync(o, s);
    const i = path_1.resolve(__filename, "../../../../../res/staticHtmlResource/index.html"),
      l = fs_1.readFileSync(i, "utf8"),
      a = `<script>window.__HVIGOR_REPORT__ = ${JSON.stringify(JSON.stringify(r))};<\/script>`,
      p = l.indexOf("</body>"),
      n = l.slice(0, p) + a + l.slice(p),
      c = path_1.resolve(e, `${t}.html`);
    fs_1.writeFileSync(c, n);
  }
  static getInstance() {
    return ReportServiceImpl.instance || (ReportServiceImpl.instance = new ReportServiceImpl()), ReportServiceImpl.instance;
  }
  startProcessMonitor() {
    this.monitorTimeId && clearInterval(this.monitorTimeId), ReportServiceImpl.data = [], this.monitorTimeId = setInterval(() => {
      const e = process_1.memoryUsage(),
        t = {
          time: Date.now(),
          rss: this.convertToMb(e.rss),
          heapTotal: this.convertToMb(e.heapTotal),
          heapUsed: this.convertToMb(e.heapUsed),
          external: this.convertToMb(e.external),
          arrayBuffers: this.convertToMb(e.arrayBuffers)
        };
      ReportServiceImpl.data.push(t);
    }, ReportServiceImpl.REPORT_INTERVAL_MS);
  }
  stopProcessMonitor() {
    if (this.monitorTimeId) {
      clearInterval(this.monitorTimeId), this.monitorTimeId = undefined;
      const e = {
        version: ReportServiceImpl.VERSION,
        pid: process_1.pid,
        data: ReportServiceImpl.data
      };
      if (undefined === ReportServiceImpl.buildId) return;
      const t = path_util_js_1.PathUtil.getReportDirPath(),
        r = path_1.join(t, `report-monitor-${ReportServiceImpl.buildId}.json`);
      fs_1.writeFileSync(r, JSON.stringify(e, null, 2), "utf-8");
    }
  }
  convertToMb(e) {
    return e / ReportServiceImpl.MB_CONVERTER / ReportServiceImpl.MB_CONVERTER;
  }
}
function enableHtmlGenerate() {
  return "boolean" == typeof global_core_parameters_js_1.coreParameter.properties["hvigor.analyzeHtml"] && global_core_parameters_js_1.coreParameter.properties["hvigor.analyzeHtml"];
}
ReportServiceImpl.MAX_REPEAT_TIMES = 10, ReportServiceImpl.REPORT_REG = /^report(-monitor)?-[0-9]+\.json$/, ReportServiceImpl.HTML_REG = /^report-?[0-9]*.html$/, ReportServiceImpl.HTML_RESOURCE_NAME = "htmlResource", ReportServiceImpl.UPLOAD_NAME = "upload", ReportServiceImpl.VERSION = "1.0", ReportServiceImpl.MB_CONVERTER = 1024, ReportServiceImpl.REPORT_INTERVAL_MS = 1e3, ReportServiceImpl.data = [];
