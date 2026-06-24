/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/main/cli.js
 */
import process_1 from "process";
import * as commander_1 from "commander";
import * as log4js from "log4js";
import * as exit_js_1 from "../../base/boot/hooks/exit";
import * as hvigor_daemon_client_log_js_1 from "../../base/daemon/log/hvigor-daemon-client-log";
import * as process_info_util_js_1 from "../../base/daemon/util/process-info-util";
import * as global_core_parameters_js_1 from "../../base/internal/data/global-core-parameters";
import * as global_data_js_1 from "../../base/internal/data/global-data";
import base_log_js_1 from "../../base/log/base-log";
import * as default_configuration_js_1 from "../../base/log/default-configuration";
import * as hvigor_log_js_1 from "../../base/log/hvigor-log";
import * as check_hvigor_config_before_program_js_1 from "../../base/util/check-hvigor-config-before-program";
import * as const_js_1 from "../../common/const/const";
import * as std_hook_js_1 from "../util/std-hook";
const cliVersion = const_js_1.CUR_HVIGOR_VERSION,
  _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor"),
  hvigorCommandMixin = {
    flagPair(e, o) {
      return this.option(`--${e}`, `Enables ${o}`).option(`--no-${e}`, `Disables ${o}`);
    },
    deprecatedOption(e, o, t) {
      return this.option(e, `['${e}' deprecated: use '${o}' instead] ${t}`);
    }
  };
function addCommands(e) {
  commander_1.program.command("version").action(() => {
    _log.info("CLI version:", cliVersion), _log.info("Local version:", cliVersion || "Unknown"), (0, exit_js_1.exit)(0);
  }).description("Shows the version of Hvigor."), commander_1.program.command("tasks").action(() => {
    e.unshift("tasks");
  }).description("Shows all available tasks of specific modules."), commander_1.program.command("taskTree").action(() => {
    e.unshift("taskTree");
  }).description("Shows all available task trees of specific modules."), commander_1.program.command("prune").action(() => {
    e.unshift("prune");
  }).description("Cleans up Hvigor cache files and removes unreferenced packages from store."), commander_1.program.command("collectCoverage").action(() => {
    e.unshift("collectCoverage");
  }).description("Generates coverage statistics reports based on instrumentation test data.");
}
function addBasicOptions() {
  commander_1.program.option("-e, --error", "Sets the log level to error.").option("-w, --warn", "Sets the log level to warn.").option("-i, --info", "Sets the log level to info.").option("-d, --debug", "Sets the log level to debug.").option("-c, --config <config>", "Sets properties in the hvigor-config.json5 file. The settings will overwrite those in the file.", (e, o = []) => o.concat(e)).option("-p, --prop <value>", "Defines extra properties.", (e, o) => {
    const [t, r] = e.split("=");
    return "buildVersion" === t && validateBuildVersion(r), o.concat([e]);
  }, []).option("-m, --mode <string>", "Sets the mode in which the command is executed.").option("-s, --sync", "Syncs the information in plugin for other platforms.").option("--node-home, <string>", "Sets the Node.js location.").option("--stop-daemon", "Stops the current project's daemon process.").option("--stop-daemon-all", "Stops all projects' daemon process.").option("--status-daemon", "Shows the daemon process status of the current project.").option("--verbose-analyze", "Enables detailed mode for build analysis.").option("--watch", "Enables watch mode.").option("--hot-compile", "HotReload watch mode to compile.").option("--hot-reload-build", "HotReload build").option("--max-old-space-size <integer>", "Sets the maximum memory size of V8's old memory section.", e => {
    const o = Number(e);
    return (!Number.isInteger(o) || o <= 0) && _log.printErrorExit("INVALID_MAX_OLD_SPACE_SIZE", [e]), o;
  }).option("--max-semi-space-size <integer>", "Sets the maximum memory size of V8's new space memory section.", e => {
    const o = Number(e);
    return (!Number.isInteger(o) || o <= 0) && _log.warn(`MaxSemiSpaceSize must be positive integer. --max-semi-space-size=${e} is invalid. Default value will be used by Node.`), o;
  });
}
function validateBuildVersion(e) {
  (null == e ? undefined : e.length) > 18 && _log.printErrorExit("INVALID_BUILDVERSION_PARAMETER", [e]);
  /^(?=.{1,18}$)(?:0|[1-9]\d*)(?:\.(?:0|[1-9]\d*)){0,2}$/.test(e) || _log.printErrorExit("INVALID_BUILDVERSION_PARAMETER", [e]);
}
function addOtherOptions() {
  commander_1.program.option("--Xmx <integer>", "Sets the maximum JVM heap size, in MB.", e => {
    const o = Number(e);
    if (!Number.isInteger(o) || o <= 0) throw new Error("--Xmx should be positive integer.");
    return o;
  }).option("--optimization-strategy <string>", "Sets the optimization strategy: memory, performance.").deprecatedOption("--enable-build-script-type-check", "type-check", "Enables the build script hvigorfile.ts type check. This option is deprecated. Use 'type-check' instead.").flagPair("stacktrace", "the printing of stack traces for all exceptions.").flagPair("type-check", "the build script hvigorfile.ts type check.").flagPair("parallel", "parallel building mode.").flagPair("incremental", "incremental building mode.").flagPair("daemon", "building with daemon process.").flagPair("generate-build-profile", "the generation of BuildProfile.ets files.").flagPair("analyze", "build analysis.").option("--analyze=<analysisMode>", "Sets the build analysis mode: normal (default), advanced, false and ultrafine.").addHelpText("after", "\nExamples:\n  hvigorw assembleApp  Do assembleApp task\n").helpOption("-h, --help", "Displays help information.").allowUnknownOption();
}
export function parseCommand() {
  (0, std_hook_js_1.initHook)();
  let e = [];
  commander_1.program.version(cliVersion, "-v, --version", "Shows the version of Hvigor.").name("hvigorw").usage("[taskNames...] <options...>"), addBasicOptions(), addOtherOptions(), commander_1.program.arguments("[taskNames...]").action(o => e = o), addCommands(e), commander_1.program.parse(process_1.argv);
  const o = {
    ...commander_1.program.opts(),
    _: e,
    env: process_1.env
  };
  return (0, check_hvigor_config_before_program_js_1.checkHvigorConfigBeforeProgram)(o, base_log_js_1).result || (0, exit_js_1.exit)(-1), (0, global_data_js_1.initStartData)(o), initLogger(), (0, hvigor_log_js_1.evaluateLogLevel)(global_core_parameters_js_1.coreParameter.startParams.logLevel), o;
}
function initDaemonClientLogger() {
  (0, hvigor_daemon_client_log_js_1.configureDaemonClient)(), (0, hvigor_log_js_1.evaluateLogLevel)(global_core_parameters_js_1.coreParameter.startParams.logLevel);
}
function initLogger() {
  log4js.shutdown(), log4js.configure((0, default_configuration_js_1.getConfiguration)());
}
export async function startHvigorBuild(e) {
  if (e.stopDaemon && (0, process_info_util_js_1.stopDaemons)(false), e.stopDaemonAll && (0, process_info_util_js_1.stopDaemons)(true), e.statusDaemon && (0, process_info_util_js_1.logDaemonInfo)(), _log.debug(`env: daemon=${global_core_parameters_js_1.coreParameter.startParams.daemon}`), global_core_parameters_js_1.coreParameter.startParams.daemon) {
    initDaemonClientLogger();
    const {
      daemonBoot: o
    } = require("./daemon-client.js");
    await o(e);
  } else {
    process_1.execArgv.length > 0 ? _log.debug(`no-daemon, use the parent process.execArgv ${process_1.execArgv}`) : _log.debug("no-daemon, use the default node options");
    const {
      boot: o
    } = require("../../base/boot/index.js");
    await o(e);
  }
}
Object.assign(commander_1.Command.prototype, hvigorCommandMixin);
