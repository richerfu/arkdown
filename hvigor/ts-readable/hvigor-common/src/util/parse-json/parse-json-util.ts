/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/parse-json/parse-json-util.js
 */
import * as judge_util_js_1 from "./judge-util";
var ErrorType;
!function (e) {
  e[e.Char = 0] = "Char", e[e.EOF = 1] = "EOF", e[e.Identifier = 2] = "Identifier";
}(ErrorType || (ErrorType = {}));
let token,
  source,
  key,
  root,
  buffer,
  c,
  parseState = "start",
  stack = [],
  pos = 0,
  line = 1,
  column = 0,
  needLocation = false,
  lexState = "default",
  quotationMark = "'",
  sign = 1;
export function parseJsonText(e, r = false) {
  source = String(e), parseState = "start", stack = [], pos = 0, line = 1, column = 0, root = undefined, needLocation = r;
  do {
    token = lex(), parseStates[parseState]();
  } while ("eof" !== token.type);
  return root;
}
function lex() {
  for (lexState = "default", buffer = "", quotationMark = "'", sign = 1;;) {
    c = peek();
    const e = lexStates[lexState]();
    if (e) return e;
  }
}
function peek() {
  if (source[pos]) return String.fromCodePoint(source.codePointAt(pos));
}
function read() {
  const e = peek();
  return "\n" === e ? (line++, column = 0) : e ? column += e.length : column++, e && (pos += e.length), e;
}
const lexStates = {
  default() {
    switch (c) {
      case "/":
        return read(), void (lexState = "comment");
      case undefined:
        return read(), newToken("eof");
    }
    if (!judge_util_js_1.JudgeUtil.isIgnoreChar(c) && !judge_util_js_1.JudgeUtil.isSpaceSeparator(c)) return lexStates[parseState]();
    read();
  },
  start() {
    lexState = "value";
  },
  beforePropertyName() {
    switch (c) {
      case "$":
      case "_":
        return buffer = read(), void (lexState = "identifierName");
      case "\\":
        return read(), void (lexState = "identifierNameStartEscape");
      case "}":
        return newToken("punctuator", read());
      case '"':
      case "'":
        return quotationMark = c, read(), void (lexState = "string");
    }
    if (judge_util_js_1.JudgeUtil.isIdStartChar(c)) return buffer += read(), void (lexState = "identifierName");
    throw error(ErrorType.Char, read());
  },
  afterPropertyName() {
    if (":" === c) return newToken("punctuator", read());
    throw error(ErrorType.Char, read());
  },
  beforePropertyValue() {
    lexState = "value";
  },
  afterPropertyValue() {
    switch (c) {
      case ",":
      case "}":
        return newToken("punctuator", read());
    }
    throw error(ErrorType.Char, read());
  },
  beforeArrayValue() {
    if ("]" === c) return newToken("punctuator", read());
    lexState = "value";
  },
  afterArrayValue() {
    switch (c) {
      case ",":
      case "]":
        return newToken("punctuator", read());
    }
    throw error(ErrorType.Char, read());
  },
  end() {
    throw error(ErrorType.Char, read());
  },
  comment() {
    switch (c) {
      case "*":
        return read(), void (lexState = "multiLineComment");
      case "/":
        return read(), void (lexState = "singleLineComment");
    }
    throw error(ErrorType.Char, read());
  },
  multiLineComment() {
    switch (c) {
      case "*":
        return read(), void (lexState = "multiLineCommentAsterisk");
      case undefined:
        throw error(ErrorType.Char, read());
    }
    read();
  },
  multiLineCommentAsterisk() {
    switch (c) {
      case "*":
        return void read();
      case "/":
        return read(), void (lexState = "default");
      case undefined:
        throw error(ErrorType.Char, read());
    }
    read(), lexState = "multiLineComment";
  },
  singleLineComment() {
    switch (c) {
      case "\n":
      case "\r":
      case "\u2028":
      case "\u2029":
        return read(), void (lexState = "default");
      case undefined:
        return read(), newToken("eof");
    }
    read();
  },
  value() {
    switch (c) {
      case "{":
      case "[":
        return newToken("punctuator", read());
      case "n":
        return read(), literal("ull"), newToken("null", null);
      case "t":
        return read(), literal("rue"), newToken("boolean", true);
      case "f":
        return read(), literal("alse"), newToken("boolean", false);
      case "-":
      case "+":
        return "-" === read() && (sign = -1), void (lexState = "numerical");
      case ".":
      case "0":
      case "I":
      case "N":
        return void (lexState = "numerical");
      case '"':
      case "'":
        return quotationMark = c, read(), buffer = "", void (lexState = "string");
    }
    if (undefined === c || !judge_util_js_1.JudgeUtil.isDigitWithoutZero(c)) throw error(ErrorType.Char, read());
    lexState = "numerical";
  },
  numerical() {
    switch (c) {
      case ".":
        return buffer = read(), void (lexState = "decimalPointLeading");
      case "0":
        return buffer = read(), void (lexState = "zero");
      case "I":
        return read(), literal("nfinity"), newToken("numeric", sign * (1 / 0));
      case "N":
        return read(), literal("aN"), newToken("numeric", NaN);
    }
    if (undefined !== c && judge_util_js_1.JudgeUtil.isDigitWithoutZero(c)) return buffer = read(), void (lexState = "decimalInteger");
    throw error(ErrorType.Char, read());
  },
  zero() {
    switch (c) {
      case ".":
      case "e":
      case "E":
        return void (lexState = "decimal");
      case "x":
      case "X":
        return buffer += read(), void (lexState = "hexadecimal");
    }
    return newToken("numeric", 0);
  },
  decimalInteger() {
    switch (c) {
      case ".":
      case "e":
      case "E":
        return void (lexState = "decimal");
    }
    if (!judge_util_js_1.JudgeUtil.isDigit(c)) return newToken("numeric", sign * Number(buffer));
    buffer += read();
  },
  decimal() {
    switch (c) {
      case ".":
        buffer += read(), lexState = "decimalFraction";
        break;
      case "e":
      case "E":
        buffer += read(), lexState = "decimalExponent";
    }
  },
  decimalPointLeading() {
    if (judge_util_js_1.JudgeUtil.isDigit(c)) return buffer += read(), void (lexState = "decimalFraction");
    throw error(ErrorType.Char, read());
  },
  decimalFraction() {
    switch (c) {
      case "e":
      case "E":
        return buffer += read(), void (lexState = "decimalExponent");
    }
    if (!judge_util_js_1.JudgeUtil.isDigit(c)) return newToken("numeric", sign * Number(buffer));
    buffer += read();
  },
  decimalExponent() {
    switch (c) {
      case "+":
      case "-":
        return buffer += read(), void (lexState = "decimalExponentSign");
    }
    if (judge_util_js_1.JudgeUtil.isDigit(c)) return buffer += read(), void (lexState = "decimalExponentInteger");
    throw error(ErrorType.Char, read());
  },
  decimalExponentSign() {
    if (judge_util_js_1.JudgeUtil.isDigit(c)) return buffer += read(), void (lexState = "decimalExponentInteger");
    throw error(ErrorType.Char, read());
  },
  decimalExponentInteger() {
    if (!judge_util_js_1.JudgeUtil.isDigit(c)) return newToken("numeric", sign * Number(buffer));
    buffer += read();
  },
  hexadecimal() {
    if (judge_util_js_1.JudgeUtil.isHexDigit(c)) return buffer += read(), void (lexState = "hexadecimalInteger");
    throw error(ErrorType.Char, read());
  },
  hexadecimalInteger() {
    if (!judge_util_js_1.JudgeUtil.isHexDigit(c)) return newToken("numeric", sign * Number(buffer));
    buffer += read();
  },
  identifierNameStartEscape() {
    if ("u" !== c) throw error(ErrorType.Char, read());
    read();
    const e = unicodeEscape();
    switch (e) {
      case "$":
      case "_":
        break;
      default:
        if (!judge_util_js_1.JudgeUtil.isIdStartChar(e)) throw error(ErrorType.Identifier);
    }
    buffer += e, lexState = "identifierName";
  },
  identifierName() {
    switch (c) {
      case "$":
      case "_":
      case "‌":
      case "‍":
        return void (buffer += read());
      case "\\":
        return read(), void (lexState = "identifierNameEscape");
    }
    if (!judge_util_js_1.JudgeUtil.isIdContinueChar(c)) return newToken("identifier", buffer);
    buffer += read();
  },
  identifierNameEscape() {
    if ("u" !== c) throw error(ErrorType.Char, read());
    read();
    const e = unicodeEscape();
    switch (e) {
      case "$":
      case "_":
      case "‌":
      case "‍":
        break;
      default:
        if (!judge_util_js_1.JudgeUtil.isIdContinueChar(e)) throw error(ErrorType.Identifier);
    }
    buffer += e, lexState = "identifierName";
  },
  string() {
    switch (c) {
      case "\\":
        return read(), void (buffer += escape());
      case '"':
      case "'":
        if (c === quotationMark) {
          const e = newToken("string", buffer);
          return read(), e;
        }
        return void (buffer += read());
      case "\n":
      case "\r":
      case undefined:
        throw error(ErrorType.Char, read());
      case "\u2028":
      case "\u2029":
        separatorChar(c);
    }
    buffer += read();
  }
};
function newToken(e, r) {
  return {
    type: e,
    value: r,
    line: line,
    column: column
  };
}
function literal(e) {
  for (const r of e) {
    if (peek() !== r) throw error(ErrorType.Char, read());
    read();
  }
}
function escape() {
  const e = peek(),
    r = controlEscape();
  if (r) return r;
  switch (e) {
    case "0":
      if (read(), judge_util_js_1.JudgeUtil.isDigit(peek())) throw error(ErrorType.Char, read());
      return "\0";
    case "x":
      return read(), hexEscape();
    case "u":
      return read(), unicodeEscape();
    case "\n":
    case "\u2028":
    case "\u2029":
      return read(), "";
    case "\r":
      return read(), "\n" === peek() && read(), "";
  }
  if (undefined === e || judge_util_js_1.JudgeUtil.isDigitWithoutZero(e)) throw error(ErrorType.Char, read());
  return read();
}
function controlEscape() {
  switch (peek()) {
    case "b":
      return read(), "\b";
    case "f":
      return read(), "\f";
    case "n":
      return read(), "\n";
    case "r":
      return read(), "\r";
    case "t":
      return read(), "\t";
    case "v":
      return read(), "\v";
  }
}
function hexEscape() {
  let e = "",
    r = peek();
  if (!judge_util_js_1.JudgeUtil.isHexDigit(r)) throw error(ErrorType.Char, read());
  if (e += read(), r = peek(), !judge_util_js_1.JudgeUtil.isHexDigit(r)) throw error(ErrorType.Char, read());
  return e += read(), String.fromCodePoint(parseInt(e, 16));
}
function unicodeEscape() {
  let e = "",
    r = 4;
  for (; r-- > 0;) {
    const r = peek();
    if (!judge_util_js_1.JudgeUtil.isHexDigit(r)) throw error(ErrorType.Char, read());
    e += read();
  }
  return String.fromCodePoint(parseInt(e, 16));
}
const parseStates = {
  start() {
    if ("eof" === token.type) throw error(ErrorType.EOF);
    push();
  },
  beforePropertyName() {
    switch (token.type) {
      case "identifier":
      case "string":
        return key = token.value, void (parseState = "afterPropertyName");
      case "punctuator":
        return void pop();
      case "eof":
        throw error(ErrorType.EOF);
    }
  },
  afterPropertyName() {
    if ("eof" === token.type) throw error(ErrorType.EOF);
    parseState = "beforePropertyValue";
  },
  beforePropertyValue() {
    if ("eof" === token.type) throw error(ErrorType.EOF);
    push();
  },
  afterPropertyValue() {
    if ("eof" === token.type) throw error(ErrorType.EOF);
    switch (token.value) {
      case ",":
        return void (parseState = "beforePropertyName");
      case "}":
        pop();
    }
  },
  beforeArrayValue() {
    if ("eof" === token.type) throw error(ErrorType.EOF);
    "punctuator" !== token.type || "]" !== token.value ? push() : pop();
  },
  afterArrayValue() {
    if ("eof" === token.type) throw error(ErrorType.EOF);
    switch (token.value) {
      case ",":
        return void (parseState = "beforeArrayValue");
      case "]":
        pop();
    }
  },
  end() {}
};
function push() {
  const e = createValue();
  if (needLocation && "object" == typeof e && (e._line = line, e._column = column), undefined === root) root = e;else {
    const r = stack[stack.length - 1];
    Array.isArray(r) ? needLocation && "object" != typeof e ? r.push({
      value: e,
      _line: line,
      _column: column
    }) : r.push(e) : r[key] = needLocation && "object" != typeof e ? {
      value: e,
      _line: line,
      _column: column
    } : e;
  }
  changeParseState(e);
}
function createValue() {
  let e;
  switch (token.type) {
    case "punctuator":
      switch (token.value) {
        case "{":
          e = {};
          break;
        case "[":
          e = [];
      }
      break;
    case "null":
    case "boolean":
    case "numeric":
    case "string":
      e = token.value;
  }
  return e;
}
function changeParseState(e) {
  if (e && "object" == typeof e) stack.push(e), parseState = Array.isArray(e) ? "beforeArrayValue" : "beforePropertyName";else {
    const e = stack[stack.length - 1];
    parseState = e ? Array.isArray(e) ? "afterArrayValue" : "afterPropertyValue" : "end";
  }
}
function pop() {
  stack.pop();
  const e = stack[stack.length - 1];
  parseState = e ? Array.isArray(e) ? "afterArrayValue" : "afterPropertyValue" : "end";
}
function separatorChar(e) {
  console.warn(`JSON5: '${formatChar(e)}' in strings is not valid ECMAScript; consider escaping.`);
}
function formatChar(e) {
  const r = {
    "'": "\\'",
    '"': '\\"',
    "\\": "\\\\",
    "\b": "\\b",
    "\f": "\\f",
    "\n": "\\n",
    "\r": "\\r",
    "\t": "\\t",
    "\v": "\\v",
    "\0": "\\0",
    "\u2028": "\\u2028",
    "\u2029": "\\u2029"
  };
  if (r[e]) return r[e];
  if (e < " ") {
    const r = e.charCodeAt(0).toString(16);
    return `\\x${`00${r}`.substring(r.length)}`;
  }
  return e;
}
function error(e, r) {
  let t = "";
  switch (e) {
    case ErrorType.Char:
      t = undefined === r ? `JSON5: invalid end of input at ${line}:${column}` : `JSON5: invalid character '${formatChar(r)}' at ${line}:${column}`;
      break;
    case ErrorType.EOF:
      t = `JSON5: invalid end of input at ${line}:${column}`;
      break;
    case ErrorType.Identifier:
      column -= 5, t = `JSON5: invalid identifier character at ${line}:${column}`;
  }
  const a = new MySyntaxError(t);
  return a.lineNumber = line, a.columnNumber = column, a;
}
class MySyntaxError extends SyntaxError {}
