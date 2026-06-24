#!/usr/bin/env node
/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/bin/hvigor-simple.js
 */
import * as cli_js_1 from "../src/cli/main/cli";
const cliOptions = (0, cli_js_1.parseCommand)();
(0, cli_js_1.startHvigorBuild)(cliOptions);
