#!/usr/bin/env node

const packageManifest = require('../package.json')

const BUILD_VALUE_OPTIONS = new Map([
  ['--project', 'project'],
  ['--target', 'target'],
  ['--mode', 'mode'],
  ['--action', 'action'],
  ['--analyze', 'analyze'],
  ['--module', 'module'],
  ['-m', 'module'],
  ['--product', 'product'],
  ['--build-target-name', 'buildTargetName'],
  ['--build-version', 'buildVersion'],
  ['--arkts-version', 'arktsVersion'],
  ['--scope', 'scope'],
  ['--ohos-test-coverage', 'ohosTestCoverage'],
  ['--coverage-path-filter', 'coveragePathFilter'],
  ['--coverage-mode', 'coverageMode'],
  ['--unit-test-replace-page', 'unitTestReplacePage'],
  ['--cache-file', 'cacheFile'],
])
const BUILD_FLAG_OPTIONS = new Map([
  ['--watch', 'watch'],
  ['--preview', 'preview'],
  ['--check-entry', 'checkEntry'],
  ['--coverage', 'coverage'],
  ['--ohos-debug-asan', 'ohosDebugAsan'],
  ['--ohos-enable-hwasan', 'ohosEnableHwasan'],
  ['--ohos-enable-tsan', 'ohosEnableTsan'],
  ['--ohos-enable-ubsan', 'ohosEnableUbsan'],
  ['--ohos-enable-binxo', 'ohosEnableBinxo'],
  ['--enable-zig', 'enableZig'],
  ['--disable-abc-cache', 'disableAbcCache'],
  ['--crossplatform', 'isCrossplatform'],
  ['--ignore-crossplatform-check', 'ignoreCrossplatformCheck'],
  ['--skip-checker', 'skipChecker'],
  ['--cache', 'cache'],
])
const BUILD_APPEND_OPTIONS = new Map([
  ['--hvigor-plugin-file', 'hvigorPluginFile'],
  ['--compile-plugin-path', 'compilePluginPath'],
])

class JsCliArgumentParser {
  constructor(command, args) {
    this.command = command
    this.args = args
  }

  build() {
    const config = {}
    for (let index = 0; index < this.args.length; index += 1) {
      const option = this.args[index]
      if (option === '--help' || option === '-h') continue
      if (BUILD_VALUE_OPTIONS.has(option)) {
        const key = BUILD_VALUE_OPTIONS.get(option)
        config[key] = this.value(++index, option)
        continue
      }
      if (BUILD_FLAG_OPTIONS.has(option)) {
        config[BUILD_FLAG_OPTIONS.get(option)] = true
        continue
      }
      if (BUILD_APPEND_OPTIONS.has(option)) {
        const key = BUILD_APPEND_OPTIONS.get(option)
        const value = this.value(++index, option)
        config[key] = config[key] === undefined ? value : [config[key]].flat().concat(value)
        continue
      }
      if (option === '--no-cache') {
        config.cache = false
        continue
      }
      this.unknown(option)
    }
    this.oneOf(config, 'analyze', ['normal', 'default', 'verbose', 'false', 'trace'])
    this.oneOf(config, 'action', [
      'assemble',
      'hot-reload-compile',
      'hot-reload-build',
      'cold-reload',
      'prepare-quickfix',
      'package-quickfix',
      'sign-quickfix',
      'assemble-quickfix',
      'device-test',
    ])
    this.oneOf(config, 'arktsVersion', ['1.1', '1.2'])
    this.oneOf(config, 'coverageMode', ['black', 'white', 'full'])
    if (config.ohosTestCoverage !== undefined) {
      if (!['true', 'false'].includes(config.ohosTestCoverage)) {
        this.invalid('--ohos-test-coverage', config.ohosTestCoverage, 'true or false')
      }
      config.ohosTestCoverage = config.ohosTestCoverage === 'true'
    }
    return config
  }

  check() {
    const config = {}
    const values = new Map([
      ['--project', 'project'],
      ['--target', 'target'],
      ['--module', 'module'],
      ['-m', 'module'],
    ])
    this.parseValues(
      config,
      values,
      new Map([
        ['--crossplatform', 'isCrossplatform'],
        ['--ignore-crossplatform-check', 'ignoreCrossplatformCheck'],
      ]),
    )
    this.oneOf(config, 'target', ['hap', 'hsp', 'har'])
    return config
  }

  clean() {
    const config = {}
    this.parseValues(config, new Map([['--project', 'project']]))
    return config
  }

  parseValues(config, values, flags = new Map()) {
    for (let index = 0; index < this.args.length; index += 1) {
      const option = this.args[index]
      if (option === '--help' || option === '-h') continue
      if (flags.has(option)) {
        config[flags.get(option)] = true
        continue
      }
      const key = values.get(option)
      if (key === undefined) this.unknown(option)
      config[key] = this.value(++index, option)
    }
  }

  value(index, option) {
    const value = this.args[index]
    if (value === undefined || value.length === 0 || value.startsWith('-')) {
      const error = new Error(`${option} requires a value`)
      error.code = 'MISSING_ARKDOWN_CLI_OPTION_VALUE'
      throw error
    }
    return value
  }

  oneOf(config, key, allowed) {
    if (config[key] !== undefined && !allowed.includes(config[key])) {
      this.invalid(key, config[key], allowed.join(', '))
    }
  }

  invalid(option, value, expected) {
    const error = new Error(`${option} must be ${expected}: ${value}`)
    error.code = 'INVALID_ARKDOWN_CLI_OPTION'
    throw error
  }

  unknown(option) {
    const error = new Error(`unknown ${this.command} option '${option}'`)
    error.code = `UNKNOWN_ARKDOWN_${this.command.toUpperCase()}_OPTION`
    throw error
  }
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
  constructor(argv, operations) {
    this.argv = argv
    this.operations = typeof operations === 'function' ? { build: operations } : (operations ?? {})
  }

  async run() {
    const [command, ...args] = this.argv
    if (command === 'build') {
      if (args.includes('--help') || args.includes('-h')) {
        this.printBuildHelp()
        return 0
      }
      const config = this.parseBuildArgs(args)
      const result = await this.operation('build')(config)
      for (const artifact of result.artifacts ?? []) writeStdout(artifact)
      return 0
    }
    if (command === 'check') {
      if (args.includes('--help') || args.includes('-h')) {
        this.printCheckHelp()
        return 0
      }
      await this.operation('check')(this.parseCheckArgs(args))
      return 0
    }
    if (command === 'clean') {
      if (args.includes('--help') || args.includes('-h')) {
        this.printCleanHelp()
        return 0
      }
      await this.operation('clean')(this.parseCleanArgs(args))
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
    const error = new Error(`Unknown arkdown command "${command}". Supported commands: build, check, clean.`)
    error.code = 'ARKDOWN_CLI_COMMAND_UNSUPPORTED'
    throw error
  }

  parseBuildArgs(args) {
    return new JsCliArgumentParser('build', args).build()
  }

  parseCheckArgs(args) {
    return new JsCliArgumentParser('check', args).check()
  }

  parseCleanArgs(args) {
    return new JsCliArgumentParser('clean', args).clean()
  }

  operation(name) {
    return this.operations[name] || createPackageRuntime()[name]
  }

  printHelp() {
    writeStdout(`Usage: arkdown <command> [options]

Options:
  -v, --version      Show the arkdown package version.
  -h, --help         Display help.

Commands:
  build              Build an OpenHarmony artifact.
  check              Check ArkTS sources without building.
  clean              Remove generated project and module outputs.
  version            Show the arkdown package version.

Examples:
  arkdown build --project . --target hap
  arkdown build --project . --arkts-version 1.2
  arkdown check --project .
  arkdown clean --project .`)
  }

  printBuildHelp() {
    writeStdout(`arkdown build

Options:
  --project <dir>     Project root. Defaults to current directory.
  --target <kind>     Output artifact: hap, hsp, har, app, or test. Defaults to hap.
  --mode <mode>       Build mode: debug or release. Defaults to debug.
  --action <action>   Build action. Defaults to assemble. Reload, QuickFix,
                      and device-test actions select their complete Rust flow.
  --analyze <mode>    Hook metrics: normal, verbose, false, or trace.
  --module, -m <name> Scope build to one module.
  --product <name>    Product name from build-profile.json5.
  --build-target-name <name>
                      Named build-profile target.
  --build-version <value>
                      Override buildVersion for this invocation.
  --arkts-version <version>
                      Override the selected product's arkTSVersion with 1.1 or
                      1.2. Products without arkTSVersion default to 1.1.
  --watch             Rebuild on changes.
  --preview           Preview resource rebuild mode.
  --check-entry       Require exactly one @Entry on the previewed page.
  --coverage          Instrument generated JavaScript for coverage.
  --ohos-debug-asan   Enable AddressSanitizer for the generated package.
  --ohos-enable-hwasan
                      Enable HWAddressSanitizer for the generated package.
  --enable-zig        Compile native C/C++ with zig cc and zig c++.
  --disable-abc-cache Disable CompileArkTS cache.
  --crossplatform     Enable ArkUI-X cross-platform API validation.
  --ignore-crossplatform-check
                      Downgrade cross-platform API errors to warnings.
  --skip-checker      Skip ArkTS linter and TypeScript build diagnostics while
                      retaining type facts required for correct transforms.
  --cache-file <path> Override es2abc cache file.
  --hvigor-plugin-file <path>
                      Load a compatible TS/JS compile-plugin module.
  --compile-plugin-path <path>
                      Load a TS/JS compile-plugin module. May be repeated.
  --no-cache          Disable coarse build cache and ABC cache.`)
  }

  printCheckHelp() {
    writeStdout(`arkdown check

Options:
  --project <dir>     Project root. Defaults to current directory.
  --target <kind>     Check context: hap, hsp, or har. Defaults to hap.
  --module, -m <name> Scope checking to one module.
  --crossplatform     Enable ArkUI-X cross-platform API validation.
  --ignore-crossplatform-check
                      Downgrade cross-platform API errors to warnings.`)
  }

  printCleanHelp() {
    writeStdout(`arkdown clean

Options:
  --project <dir>     Project root. Defaults to current directory.`)
  }
}

if (
  require.main === module &&
  ['build', 'check'].includes(process.argv[2]) &&
  typeof global.gc !== 'function' &&
  !process.execArgv.includes('--no-expose-gc')
) {
  // Current Hvigor calcChildExecArgv exposes GC by default; serviceChecker
  // uses it after releasing compiler state. Process ownership stays in JS.
  const { spawnSync } = require('node:child_process')
  const child = spawnSync(
    process.execPath,
    [...process.execArgv, '--expose-gc', __filename, ...process.argv.slice(2)],
    { stdio: 'inherit' },
  )
  if (child.error) writeStderr(`error: ${child.error.message}`)
  process.exitCode = child.status ?? 1
} else if (require.main === module) {
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
