/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/log/clang-log-diagnostic.js
 */
import os_1 from "os";
var Level;
!function (i) {
  i[i.INFO = 0] = "INFO", i[i.WARNING = 1] = "WARNING", i[i.ERROR = 2] = "ERROR";
}(Level || (Level = {}));
export class ClangLogHandler {
  constructor() {
    this.state = 0, this.lines = [], this._text = "";
  }
  log(i) {
    const t = doClassify(i);
    if (t.complete && (this.lines.forEach(([i, t]) => {
      var e;
      return this.logging(null !== (e = this.currentDiagnostic) && undefined !== e ? e : t, i);
    }), this.lines = []), (1 === this.state && t.main && !t.after || 0 === this.state && t.main) && (this.currentDiagnostic = t), t.precede || t.main || 1 === this.state && t.name === LogDiagnostic.NONE.name) return this.lines.push([i, t]), void this.updateState(t);
    this.logging(t, i), this.updateState(t);
  }
  updateState(i) {
    i.main ? this.state = 1 : (i.complete || i.precede) && (this.state = 0);
  }
  get text() {
    return this._text;
  }
  logging(i, t) {
    switch (i.level) {
      case Level.INFO:
        this._text += `[32m${t}[32m[39m${os_1.EOL}`;
        break;
      case Level.WARNING:
        this._text += `[33m${t}[33m[39m${os_1.EOL}`;
        break;
      case Level.ERROR:
        this._text += `[31m${t}[31m[39m${os_1.EOL}`;
        break;
      default:
        this._text += `[32m${t}[32m[39m${os_1.EOL}`;
    }
  }
}
const doClassify = i => {
  if (!i.includes(":") && !i.includes("error") && !i.includes("warning")) return LogDiagnostic.NONE;
  for (let t of LogDiagnostic.classifications) {
    const e = t.match(i);
    if (e) return e;
  }
  return LogDiagnostic.NONE;
};
export class LogDiagnostic {
  constructor(i, t, e, o, n, s, L) {
    this.name = i, this.pattern = t, this.level = e, this.main = o, this.after = n, this.precede = s, this.complete = L;
  }
  match(i) {
    return this.pattern.test(i) ? this : undefined;
  }
}
LogDiagnostic.IN_FILES_INCLUDED_FROM = new LogDiagnostic("IN_FILES_INCLUDED_FROM", /In file included from (?<file>.+):(?<line>\d+):/, Level.INFO, false, false, true, false), LogDiagnostic.CLANG_COMPILER_ERROR = new LogDiagnostic("CLANG_COMPILER_ERROR", /(?<file>(?:[A-Z]:)?[^\s][^:]+):(?<line>\d+):(?<column>\d+): (error|fatal error): (?<body>.*)/, Level.ERROR, true, false, false, true), LogDiagnostic.CLANG_LINKER_ERROR = new LogDiagnostic("CLANG_LINKER_ERROR", /((?:[A-Z]:)?[^\s][^:]+)(?::(\d+))?: (error|fatal error)?: (?<body>.+)/, Level.ERROR, true, false, false, true), LogDiagnostic.CLANG_COMPILER_WARNING = new LogDiagnostic("CLANG_COMPILER_WARNING", /(?<file>(?:[A-Z]:)?[^\s][^:]+):(?<line>\d+):(?<column>\d+): (warn|warning): (?<body>.*)/, Level.WARNING, true, false, false, true), LogDiagnostic.CLANG_LINKER_WARNING = new LogDiagnostic("CLANG_LINKER_WARNING", /((?:[A-Z]:)?[^\s][^:]+)(?::(\d+))?: (warn|warning)?: (?<body>.+)/, Level.WARNING, false, false, false, false), LogDiagnostic.LINK_FATAL = new LogDiagnostic("CLANG_COMPILER_ERROR", /.*ld: fatal error: (?<file>.*): (?<body>open: Invalid argument)/, Level.ERROR, true, false, false, true), LogDiagnostic.CLANG_COMPILER_INFO = new LogDiagnostic("CLANG_COMPILER_INFO", /(?<file>(?:[A-Z]:)?[^\s][^:]+):(\d+):(\d+): (ignored|note|remark): (?<body>.*)/, Level.INFO, true, true, false, false), LogDiagnostic.CLANG_LINKER_INFO = new LogDiagnostic("CLANG_LINKER_INFO", /((?:[A-Z]:)?[^\s][^:]+)(?::(\d+))?: (ignored|note|remark)?: (?<body>.+)/, Level.INFO, true, true, false, false), LogDiagnostic.CLANG_ERRORS_GENERATED = new LogDiagnostic("CLANG_ERRORS_GENERATED", /([0-9]* warnings?)?( and )?([0-9]* errors?)? generated./, Level.WARNING, false, false, false, true), LogDiagnostic.NINJA_ENTERING_DIRECTORY = new LogDiagnostic("NINJA_ENTERING_DIRECTORY", /ninja: Entering directory [`'](?<dir>[^']+)'/, Level.INFO, false, false, false, true), LogDiagnostic.NINJA_BUILD_STOPPED = new LogDiagnostic("NINJA_BUILD_STOPPED", /ninja: build stopped.*failed/, Level.ERROR, false, false, false, true), LogDiagnostic.FAILED_TARGET = new LogDiagnostic("FAILED_TARGET", /FAILED: .*/, Level.ERROR, false, false, false, true), LogDiagnostic.CLANG_COMMAND_LINE = new LogDiagnostic("CLANG_COMMAND_LINE", /.*clang(\+\+)?(\.exe)? .*--target.*ohos.*/, Level.ERROR, false, false, false, true), LogDiagnostic.NONE = new LogDiagnostic("NONE", /.*/, Level.INFO, false, false, false, false), LogDiagnostic.classifications = [LogDiagnostic.IN_FILES_INCLUDED_FROM, LogDiagnostic.CLANG_COMPILER_ERROR, LogDiagnostic.CLANG_LINKER_ERROR, LogDiagnostic.CLANG_COMPILER_WARNING, LogDiagnostic.CLANG_LINKER_WARNING, LogDiagnostic.LINK_FATAL, LogDiagnostic.CLANG_COMPILER_INFO, LogDiagnostic.CLANG_LINKER_INFO, LogDiagnostic.CLANG_ERRORS_GENERATED, LogDiagnostic.NINJA_ENTERING_DIRECTORY, LogDiagnostic.NINJA_BUILD_STOPPED, LogDiagnostic.FAILED_TARGET, LogDiagnostic.CLANG_COMMAND_LINE, LogDiagnostic.NONE];
