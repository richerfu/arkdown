#!/usr/bin/env node

const packageManifest = require('../package.json')
const { parseArkdownBuildCliArgs } = require('../index.js')

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
  constructor(argv, build) {
    this.argv = argv
    this.build = build
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
    if (this.argv.includes('--help') || this.argv.includes('-h') || command === undefined) {
      this.printHelp()
      return 0
    }
    if (this.argv.includes('--version') || this.argv.includes('-v') || command === 'version') {
      writeStdout(packageManifest.version)
      return 0
    }
    const error = new Error(`Unknown arkdown command "${command}". Only "build" is supported.`)
    error.code = 'ARKDOWN_CLI_COMMAND_UNSUPPORTED'
    throw error
  }

  parseBuildArgs(args) {
    return decodeCliPlan(parseArkdownBuildCliArgs(args))
  }

  printHelp() {
    writeStdout(`Usage: arkdown <command> [options]

Options:
  -v, --version      Show the arkdown package version.
  -h, --help         Display help.

Commands:
  build              Build an OpenHarmony artifact.
  version            Show the arkdown package version.

Examples:
  arkdown build --project . --target hap
  arkdown build --project . --arkts-version 1.2`)
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
  --arkts-version <version>
                      Compile as ArkTS 1.1 or 1.2. 1.2 must be explicit;
                      the default is 1.1.
  --ohpm-mode <mode>  Dependency acquisition: offline, prefer-offline, or online.
  --watch             Rebuild on changes.
  --preview           Preview resource rebuild mode.
  --check-entry       Require exactly one @Entry on the previewed page.
  --coverage          Instrument generated JavaScript for coverage.
  --ohos-debug-asan   Enable AddressSanitizer for the generated package.
  --ohos-enable-hwasan
                      Enable HWAddressSanitizer for the generated package.
  --enable-zig        Compile native C/C++ with zig cc and zig c++.
  --disable-abc-cache Disable CompileArkTS cache.
  --cache-file <path> Override es2abc cache file.
  --checker <mode>    Checker mode: oxc or off. Defaults to oxc.
  --hvigor-plugin-file <path>
                      Load a compatible TS/JS compile-plugin module.
  --compile-plugin-path <path>
                      Load a TS/JS compile-plugin module. May be repeated.
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
