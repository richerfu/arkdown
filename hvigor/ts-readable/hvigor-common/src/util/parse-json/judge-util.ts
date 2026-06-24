/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/parse-json/judge-util.js
 */
import * as unicode_js_1 from "./unicode";
export class JudgeUtil {
  static isIgnoreChar(t) {
    return "string" == typeof t && ("\t" === t || "\v" === t || "\f" === t || " " === t || " " === t || "\ufeff" === t || "\n" === t || "\r" === t || "\u2028" === t || "\u2029" === t);
  }
  static isSpaceSeparator(t) {
    return "string" == typeof t && unicode_js_1.Unicode.SPACE_SEPARATOR.test(t);
  }
  static isIdStartChar(t) {
    return "string" == typeof t && (t >= "a" && t <= "z" || t >= "A" && t <= "Z" || "$" === t || "_" === t || unicode_js_1.Unicode.ID_START.test(t));
  }
  static isIdContinueChar(t) {
    return "string" == typeof t && (t >= "a" && t <= "z" || t >= "A" && t <= "Z" || t >= "0" && t <= "9" || "$" === t || "_" === t || "‌" === t || "‍" === t || unicode_js_1.Unicode.ID_CONTINUE.test(t));
  }
  static isDigitWithoutZero(t) {
    return /[1-9]/.test(t);
  }
  static isDigit(t) {
    return "string" == typeof t && /[0-9]/.test(t);
  }
  static isHexDigit(t) {
    return "string" == typeof t && /[0-9A-Fa-f]/.test(t);
  }
}
