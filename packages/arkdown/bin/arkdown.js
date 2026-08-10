#!/usr/bin/env node

const { HVIGOR_PARITY_VERSION } = require('../runtime.js')
const { parseArkdownBuildCliArgs, parseHvigorCliArgs } = require('../index.js')

function decodeCliPlan(response) {
  const plan = JSON.parse(response)
  if (plan.error) {
    const error = new Error(plan.error.message)
    error.code = plan.error.code
    throw error
  }
  return plan.config
}

function createPackageRuntime() {
  return require('..')
}

function writeStdout(message) {
  process.stdout.write(`${message}\n`)
}

function writeStderr(message) {
  process.stderr.write(`${message}\n`)
}

class ArkdownPackageCli {
  constructor(argv, build, runHvigorCommand) {
    this.argv = argv
    this.build = build
    this.runHvigorCommand = runHvigorCommand
  }

  async run() {
    const [command, ...args] = this.argv
    if (command === 'build') {
      if (args.includes('--help') || args.includes('-h')) {
        this.printBuildHelp()
        return 0
      }
      const config = this.parseBuildArgs(args)
      const result = await (this.build || createPackageRuntime().build)(config)
      for (const artifact of result.artifacts ?? []) writeStdout(artifact)
      return 0
    }
    if (this.argv.includes('--help') || this.argv.includes('-h')) {
      this.printHelp()
      return 0
    }
    if (this.argv.includes('--version') || this.argv.includes('-v')) {
      writeStdout(HVIGOR_PARITY_VERSION)
      return 0
    }
    if (command === 'version') {
      writeStdout(`CLI version: ${HVIGOR_PARITY_VERSION}`)
      writeStdout(`Local version: ${HVIGOR_PARITY_VERSION}`)
      return 0
    }

    const config = this.parseHvigorArgs(this.argv)
    const daemonOutput = this.daemonCommandOutput(config)
    if (daemonOutput) {
      for (const message of daemonOutput) writeStdout(message)
      return 0
    }
    const result = await (this.runHvigorCommand || createPackageRuntime().runHvigorCommand)(config)
    for (const message of result.output ?? []) writeStdout(message)
    return 0
  }

  parseHvigorArgs(args) {
    return decodeCliPlan(parseHvigorCliArgs(args))
  }

  daemonCommandOutput(config) {
    // Latest hvigor handles these flags before bootstrapping the project and
    // exits after the first matching command. Arkdown deliberately has no
    // daemon registry because every build is process-local, so its registry
    // view is always empty while preserving the command contract.
    if (config.stopDaemon || config.stopDaemonAll) return ['Stopped 0 daemon in total.']
    if (config.statusDaemon) return ['PID\tSTATUS\tPORT\tROOT_PATH']
    return null
  }

  parseBuildArgs(args) {
    return decodeCliPlan(parseArkdownBuildCliArgs(args))
  }

  printHelp() {
    writeStdout(`Usage: hvigorw [taskNames...] <options...>

Options:
  -v, --version                         Shows the version of Hvigor.
  -e, --error                           Sets the log level to error.
  -w, --warn                            Sets the log level to warn.
  -i, --info                            Sets the log level to info.
  -d, --debug                           Sets the log level to debug.
  -c, --config <config>                 Overrides hvigor-config.json5 properties.
  -p, --prop <value>                    Defines extra properties.
  -m, --mode <module|project>           Sets the command execution domain.
  -s, --sync                            Syncs plugin information for other platforms.
  --watch                               Enables watch mode.
  --[no-]type-check                     Controls hvigorfile.ts type checking.
  --[no-]parallel                       Controls parallel task execution.
  --[no-]incremental                    Controls incremental task execution.
  --[no-]daemon                         Controls the daemon process.
  -h, --help                            Displays help information.

Commands:
  version                               Shows the version of Hvigor.
  tasks                                 Shows all available tasks of selected nodes.
  taskTree                              Shows all available task trees of selected nodes.
  prune                                 Cleans old Hvigor caches and package-store entries.
  collectCoverage                       Generates instrumentation coverage reports.

Examples:
  hvigorw assembleApp
  hvigorw --mode module -p module=entry assembleHap`)
  }

  printBuildHelp() {
    writeStdout(`arkdown build

Options:
  --project <dir>     Project root. Defaults to current directory.
  --target <kind>     Output artifact: hap, hsp, har, app, or test. Defaults to hap.
  --mode <mode>       Build mode: debug or release. Defaults to debug.
  --module, -m <name> Scope build to one module.
  --product <name>    Product name from build-profile.json5.
  --build-target-name <name>
                      Named build-profile target.
  --build-version <value>
                      Override buildVersion for this invocation.
  --watch             Rebuild on changes.
  --preview           Preview resource rebuild mode.
  --coverage          Instrument generated JavaScript for coverage.
  --ohos-debug-asan   Enable AddressSanitizer for the generated package.
  --ohos-enable-hwasan
                      Enable HWAddressSanitizer for the generated package.
  --disable-abc-cache Disable CompileArkTS cache.
  --cache-file <path> Override es2abc cache file.
  --checker <mode>    Checker mode: oxc or off. Defaults to oxc.
  --module-analyzer <mode>
                      Module analyzer: oxc or yuku. Defaults to oxc.
  --hvigor-plugin-file <path>
                      Load a hvigor plugin file before compile plugins.
  --compile-plugin-path <path>
                      Load a hvigor compile plugin path. May be repeated.
  --no-cache          Disable coarse build cache and ABC cache.`)
  }
}

if (require.main === module) {
  new ArkdownPackageCli(process.argv.slice(2))
    .run()
    .then((code) => {
      process.exitCode = code
    })
    .catch((error) => {
      writeStderr(`error: ${error.message}`)
      process.exitCode = 1
    })
}

module.exports = {
  ArkdownPackageCli,
}
