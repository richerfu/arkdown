#!/usr/bin/env node

const packageManifest = require('../package.json')
const { TerminalColors } = require('./build-logger.js')

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
  ['--logLevel', 'logLevel'],
  ['--log-level', 'logLevel'],
  ['-l', 'logLevel'],
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
      if (/^--(?:logLevel|log-level)=/.test(option)) {
        config.logLevel = option.slice(option.indexOf('=') + 1)
        continue
      }
      if (option === '--silent') {
        config.logLevel = 'silent'
        continue
      }
      if (option === '--clearScreen' || option === '--clear-screen') {
        const value = this.value(++index, option)
        if (!['true', 'false'].includes(value)) this.invalid(option, value, 'true or false')
        config.clearScreen = value === 'true'
        continue
      }
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
    this.oneOf(config, 'logLevel', ['debug', 'info', 'warn', 'error', 'silent'])
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
  process.stderr.write(`${new TerminalColors(process.stderr).format('error', message)}\n`)
}

class BuildCliReporter {
  constructor(config, output = {}) {
    this.level = config.logLevel ?? 'info'
    this.watch = config.watch || config.preview || ['hot-reload-compile', 'hot-reload-build'].includes(config.action)
    this.stdout = output.stdout ?? process.stdout
    this.stderr = output.stderr ?? process.stderr
    this.stdoutColors = new TerminalColors(this.stdout, output.env)
    this.stderrColors = new TerminalColors(this.stderr, output.env)
    this.tty = Boolean(
      this.stdout.isTTY &&
      !(output.env ?? process.env).CI &&
      (output.env ?? process.env).TERM !== 'dumb' &&
      config.clearScreen !== false,
    )
    this.started = performance.now()
    this.artifacts = new Set()
  }

  enabled(level) {
    const levels = { silent: 0, error: 1, warn: 2, info: 3, debug: 4 }
    return levels[this.level] >= levels[level]
  }

  duration(ms) {
    return ms < 1000 ? `${Math.round(ms)}ms` : `${(ms / 1000).toFixed(2)}s`
  }

  clear() {
    if (this.pending) this.stdout.write('\r\x1b[2K')
    this.pending = false
  }

  stopProgress() {
    clearInterval(this.progressTimer)
    this.progressTimer = undefined
    this.activeStage = undefined
    this.clear()
  }

  progressText(fields, complete = false) {
    const completed = complete ? fields.step : fields.step - 1
    const columns = this.stdout.columns > 1 ? this.stdout.columns : 80
    const width = Math.max(4, Math.min(20, columns - 44))
    const filled = Math.floor((completed / fields.total) * width)
    const marker = complete ? '✓' : ['|', '/', '-', '\\'][this.progressFrame % 4]
    const active = !complete && filled < width ? '>' : ''
    const bar = `${'='.repeat(filled)}${active}${'-'.repeat(width - filled - active.length)}`
    const elapsed = complete ? fields.elapsed_ms : performance.now() - this.stageStarted
    const prefix = `${marker} [${bar}] ${completed}/${fields.total} stages | `
    const suffix = ` (${this.duration(elapsed)})`
    const available = columns - 1 - prefix.length - suffix.length
    const label =
      fields.label.length > available ? `${fields.label.slice(0, Math.max(0, available - 1))}…` : fields.label
    return `${prefix}${label}${suffix}`.slice(0, Math.max(0, columns - 1))
  }

  renderProgress() {
    if (!this.activeStage) return
    this.clear()
    this.stdout.write(this.stdoutColors.format('info', this.progressText(this.activeStage)))
    this.pending = true
  }

  startProgress(fields) {
    this.stopProgress()
    this.activeStage = fields
    this.stageStarted = performance.now()
    this.progressFrame = 0
    this.renderProgress()
    this.progressTimer = setInterval(() => {
      this.progressFrame++
      this.renderProgress()
    }, 100)
    this.progressTimer.unref()
  }

  write(level, message) {
    if (!this.enabled(level)) return
    this.clear()
    const stream = ['warn', 'error'].includes(level) ? this.stderr : this.stdout
    const colors = stream === this.stderr ? this.stderrColors : this.stdoutColors
    stream.write(`${level === 'info' ? message : colors.format(level, message)}\n`)
    this.renderProgress()
  }

  log(log) {
    if (!this.enabled(log.level)) return
    const fields = log.fields ?? {}
    if (fields.source === 'rust_stage') {
      const stage = `[${fields.step}/${fields.total}] ${fields.label}`
      if (fields.phase === 'start') {
        if (this.tty) this.startProgress(fields)
        else this.write('info', this.stdoutColors.format('info', `${stage}...`))
      } else if (this.tty) {
        this.stopProgress()
        this.write('info', this.stdoutColors.format('success', this.progressText(fields, true)))
      } else {
        this.write(
          'info',
          `${this.stdoutColors.format('success', '✓')} ${stage} ${this.stdoutColors.format('muted', `(${this.duration(fields.elapsed_ms)})`)}`,
        )
      }
      return
    }
    if (log.message === 'build artifact') {
      if (this.watch) this.artifacts.add(fields.artifact)
      return
    }
    if (log.message === 'watch rebuild started' || log.message === 'watch rebuild failed') {
      this.stopProgress()
      this.artifacts.clear()
    }
    // Watch renders a summary after all artifacts for the completed cycle arrive.
    if (log.message === 'build succeeded' || log.message === 'build succeeded with no output files produced') {
      if (this.watch) {
        this.summary([...this.artifacts], fields.elapsed_seconds * 1000)
        this.artifacts.clear()
      }
      return
    }
    const details = Object.entries(fields)
      .filter(([key]) => this.level === 'debug' || ['module', 'plugin', 'code', 'diagnostic', 'error'].includes(key))
      .map(([key, value]) => `${key}=${typeof value === 'string' ? value : JSON.stringify(value)}`)
      .join(' ')
    const prefix = log.level === 'info' ? '[arkdown]' : `[arkdown:${log.level}]`
    this.write(
      log.level,
      `${log.level === 'info' ? this.stdoutColors.format('info', prefix) : prefix} ${log.message}${details ? ` ${details}` : ''}`,
    )
  }

  finish(result) {
    this.summary(result.artifacts ?? [], result.elapsedMs ?? performance.now() - this.started)
  }

  highlight(style, value) {
    return this.stdoutColors.format(style, this.stdoutColors.format('bold', value))
  }

  summary(artifacts, elapsedMs) {
    this.stopProgress()
    const paths = [...new Set(artifacts)].sort()
    if (paths.length > 0) {
      this.write('info', `\n${this.highlight('info', `Build outputs (${paths.length}):`)}`)
      for (const artifact of paths) this.write('info', `  ${this.highlight('artifact', artifact)}`)
    }
    this.write('info', this.highlight('success', `✓ built in ${this.duration(elapsedMs)}`))
    this.clear()
  }

  fail(error) {
    this.stopProgress()
    this.write('error', `✗ Build failed in ${this.duration(performance.now() - this.started)}\nerror: ${error.message}`)
    this.clear()
  }
}

class ArkdownPackageCli {
  constructor(argv, operations, output) {
    this.argv = argv
    this.operations = typeof operations === 'function' ? { build: operations } : (operations ?? {})
    this.output = output
  }

  async run() {
    const [command, ...args] = this.argv
    if (command === 'build') {
      if (args.includes('--help') || args.includes('-h')) {
        this.printBuildHelp()
        return 0
      }
      const config = this.parseBuildArgs(args)
      const reporter = new BuildCliReporter(config, this.output)
      const { clearScreen: _, ...request } = config
      reporter.write(
        'info',
        `${reporter.stdoutColors.format('info', `arkdown v${packageManifest.version}`)} building ${config.mode ?? 'debug'} ${config.target ?? 'hap'}...`,
      )
      try {
        const result = await this.operation('build')({ ...request, onLog: (log) => reporter.log(log) })
        reporter.finish(result)
      } catch (error) {
        reporter.fail(error)
        return 1
      }
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
  -l, --logLevel <level>
                      Console logging: debug, info (default), warn, error, silent.
                      Also accepts --log-level. Silent keeps failure exit codes.
  --clearScreen <boolean>
                      Update progress in place on a terminal (default true).
                      Use false for persistent progress lines. No animation in CI.
  --silent            Suppress all build output.
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
