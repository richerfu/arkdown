'use strict'

const { AsyncLocalStorage } = require('node:async_hooks')

const buildLogs = new AsyncLocalStorage()
const LOG_LEVELS = Object.freeze({ silent: 0, error: 1, warn: 2, info: 3, debug: 4 })

class BuildLogDispatcher {
  constructor(config) {
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
        const stream = ['warn', 'error'].includes(log.level) ? process.stderr : process.stdout
        stream.write(`[arkdown] ${log.message}${details ? ` ${details}` : ''}\n`)
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

module.exports = { BuildLogDispatcher }
