// Fixture: single script source
// Upstream oracle: es2abc --output single-script.abc main.js
const x = 1;
const y = x + 2;
globalThis["result"] = y;
