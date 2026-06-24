#!/usr/bin/env node
/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/bin/hvigor.js
 */
import process_1 from "process";
import * as semver_1 from "semver";
import * as exit_js_1 from "../src/base/boot/hooks/exit";
import * as global_core_parameters_js_1 from "../src/base/internal/data/global-core-parameters";
import base_log_js_1 from "../src/base/log/base-log";
import * as cli_js_1 from "../src/cli/main/cli";
import * as prepare_node_path_js_1 from "../src/cli/wrapper/prepare-node-path";
import * as util_js_1 from "../src/cli/wrapper/util";
const cliOptions = (0, cli_js_1.parseCommand)();
const LOWEST_NODE_VERSION = 'v14.19.1';
if ((0, semver_1.lt)(process_1.version, LOWEST_NODE_VERSION)) {
  base_log_js_1.error(`current node version: ${process_1.version}, node version cannot be lower than ${LOWEST_NODE_VERSION}`);
  (0, exit_js_1.exit)(-1);
}
(0, prepare_node_path_js_1.initNodePath)((0, util_js_1.getHvigorProjectHome)());
startDaemonIfNeeded().then(() => (0, cli_js_1.startHvigorBuild)(cliOptions));
function checkDaemonCli(cliArgv) {
  return !cliArgv.stopDaemon && !cliArgv.stopDaemonAll && !cliArgv.statusDaemon && global_core_parameters_js_1.coreParameter.startParams.daemon;
}
async function startDaemonIfNeeded() {
  if (checkDaemonCli(cliOptions)) {
    await Promise.resolve().then(() => __importStar(require('../src/base/daemon/process/daemon-process-factory.js'))).then(module => {
      if (cliOptions.Xmx) {
        process_1.env.Xmx = cliOptions.Xmx.toString();
      }
      module.defaultDaemonServerFactory.createDaemonFork();
    });
  }
}
