'use strict'

const { AsyncLocalStorage } = require('node:async_hooks')

const buildLogs = new AsyncLocalStorage()
const LOG_LEVELS = Object.freeze({ silent: 0, error: 1, warn: 2, info: 3, debug: 4 })
const COLOR_CODES = Object.freeze({ info: 36, success: 32, warn: 33, error: 31, debug: 90, muted: 90, artifact: 36 })

class TerminalColors {
  constructor(stream, env = process.env) {
    // Explicit controls precede automatic TTY detection. CI and redirected
    // output stay plain unless FORCE_COLOR requests color; NO_COLOR wins.
    this.enabled =
      !env.NO_COLOR &&
      (env.FORCE_COLOR !== undefined
        ? env.FORCE_COLOR !== '0'
        : Boolean(stream.isTTY && !env.CI && env.TERM !== 'dumb'))
  }

  format(style, value) {
    const text = String(value)
    if (!this.enabled) return text
    const open = `\x1b[${COLOR_CODES[style]}m`
    const close = '\x1b[39m'
    // Restore the outer color after nested styles and reset it on every line.
    return `${open}${text.replaceAll(close, open).replaceAll('\n', `${close}\n${open}`)}${close}`
  }
}

class BuildLogDispatcher {
  constructor(config, output = {}) {
    this.level = config.logLevel ?? 'info'
    if (!Object.hasOwn(LOG_LEVELS, this.level)) {
      throw Object.assign(new Error(`Invalid logLevel '${this.level}'; expected debug, info, warn, error, or silent`), {
        code: 'INVALID_ARKDOWN_LOG_LEVEL',
      })
    }
    if (config.onLog !== undefined && typeof config.onLog !== 'function') {
      throw Object.assign(new Error('onLog must be a function'), { code: 'INVALID_ARKDOWN_LOG_HANDLER' })
    }
    this.onLog = config.onLog
    this.stdout = output.stdout ?? process.stdout
    this.stderr = output.stderr ?? process.stderr
    this.stdoutColors = new TerminalColors(this.stdout, output.env)
    this.stderrColors = new TerminalColors(this.stderr, output.env)
  }

  emit(log) {
    if (log.target === 'arkdown::log_flush' || LOG_LEVELS[log.level] > LOG_LEVELS[this.level]) return
    try {
      if (this.onLog) this.onLog(log)
      else {
        const details = Object.entries(log.fields || {})
          .filter(([, value]) => value !== undefined)
          .map(([key, value]) => `${key}=${typeof value === 'string' ? value : JSON.stringify(value)}`)
          .join(' ')
        const warning = ['warn', 'error'].includes(log.level)
        const stream = warning ? this.stderr : this.stdout
        const colors = warning ? this.stderrColors : this.stdoutColors
        stream.write(`${colors.format(log.level, `[arkdown] ${log.message}${details ? ` ${details}` : ''}`)}\n`)
      }
    } catch (error) {
      // Native diagnostics arrive through a TSFN: reject the build promise after
      // its FIFO barrier instead of letting an exception escape into Node.
      this.error ??= error instanceof Error ? error : new Error(String(error))
    }
  }

  static current() {
    return buildLogs.getStore()
  }

  static run(logger, work) {
    return buildLogs.run(logger, work)
  }

  static emit(level, message, fields = {}) {
    const log = { level, message, fields, target: 'arkdown::plugin' }
    const dispatcher = buildLogs.getStore()
    if (dispatcher) dispatcher.emit(log)
    else new BuildLogDispatcher({}).emit(log)
  }
}

module.exports = { BuildLogDispatcher, TerminalColors }
