'use strict'

const util = require('node:util')

// This module is the deliberate V8 boundary for the values declared by
// @ohos/hvigor/index.d.ts. Filesystem and logging behavior stays native; only
// JavaScript identity, Buffer conversion, and JSON.stringify callbacks remain.
function createHvigorPublicFacade(nativeRuntimeBinding) {
  const constants = new nativeRuntimeBinding.RuntimeHvigorConstantApi().snapshot()
  const fileApi = new nativeRuntimeBinding.RuntimeHvigorFileApi()

  const filePath = (value) => (typeof value === 'string' ? value : value?.filePath)
  const requirePath = (value) => {
    const resolved = filePath(value)
    if (typeof resolved === 'string') return resolved
    throw new TypeError('The parameter type is invalid. The function requires the string type.')
  }
  const requireFile = (value) => {
    const resolved = requirePath(value)
    if (!fileApi.isFile(resolved)) {
      const error = new Error(`the Path: ${resolved} is not a file`)
      error.code = 'IS_NOT_A_FILE_PATH'
      throw error
    }
    return resolved
  }

  class FileUtil {
    static exist(value) {
      return fileApi.exists(requirePath(value))
    }

    static isDictionary(value) {
      return fileApi.isDirectory(requirePath(value))
    }

    static isFile(value) {
      return fileApi.isFile(requirePath(value))
    }

    static ensureDirSync(value) {
      fileApi.ensureDirectory(requirePath(value))
    }

    static ensureFileSync(value) {
      fileApi.ensureFile(requirePath(value))
    }

    static readFileSync(value) {
      return fileApi.readFileSync(requireFile(value))
    }

    static readFile(value) {
      return fileApi.readFile(requireFile(value))
    }

    static readJson5(value) {
      return fileApi.readJson5(requireFile(value))
    }

    static writeFile(value, content) {
      const bytes = Buffer.isBuffer(content) ? content : Buffer.from(String(content))
      return fileApi.writeFile(requireFile(value), bytes)
    }

    static writeFileSync(value, content) {
      const bytes = Buffer.isBuffer(content) ? content : Buffer.from(String(content))
      fileApi.writeFileSync(requireFile(value), bytes)
    }

    static copyFile(value, target) {
      return fileApi.copyFile(requireFile(value), requirePath(target))
    }

    static copyFileSync(value, target) {
      fileApi.copyFileSync(requireFile(value), requirePath(target))
    }

    static pathResolve(...segments) {
      return fileApi.pathResolve(segments.map(String))
    }
  }

  for (const name of [
    'exist',
    'isDictionary',
    'isFile',
    'ensureDirSync',
    'ensureFileSync',
    'readFileSync',
    'readFile',
    'readJson5',
    'writeFile',
    'writeFileSync',
    'copyFile',
    'copyFileSync',
    'pathResolve',
  ]) {
    FileUtil[`${name}Internal`] = FileUtil[name]
  }

  class BuildResult {
    constructor(error, report = null) {
      this.error = error
      this.report = report
    }

    getError() {
      return this.error
    }

    getErrorInternal() {
      return this.error
    }

    getReportJson() {
      return this.report
    }

    getReportJsonInternal() {
      return this.report
    }
  }

  function replacer(_key, value) {
    if (value instanceof Map) return Object.fromEntries(value)
    if (value instanceof Set) return [...value]
    return value
  }

  return {
    BuildResult,
    FileUtil,
    Priority: Object.freeze(constants.priority),
    TaskState: Object.freeze(constants.taskState),
    workerConstants: Object.freeze(constants.worker),
    replacer,
  }
}

function createRuntimeLoggerFactory(nativeRuntimeBinding) {
  const instances = new Map()

  class RuntimeLogger {
    constructor(category, durationId) {
      this.category = category
      this.durationId = durationId
    }

    log(level, ...args) {
      nativeRuntimeBinding.emitRuntimeLog({
        level,
        category: this.category,
        message: util.format(...args),
        durationId: this.durationId,
        source: 'js_plugin',
      })
    }

    debug(...args) {
      this.log('debug', ...args)
    }

    info(...args) {
      this.log('info', ...args)
    }

    warn(...args) {
      this.log('warn', ...args)
    }

    error(...args) {
      this.log('error', ...args)
    }

    fatal(...args) {
      this.log('error', ...args)
    }

    getMessage(...args) {
      return util.format(...args)
    }

    printErrorExit(code, args = []) {
      const error = new Error([code, ...args].flat(2).map(String).join(' '))
      error.code = code
      throw error
    }

    errorMessageExit(...args) {
      throw new Error(util.format(...args))
    }
  }

  return {
    getLogger(category = 'default') {
      if (!instances.has(category)) instances.set(category, new RuntimeLogger(category))
      return instances.get(category)
    },
    getLoggerWithDurationId(category, durationId) {
      return new RuntimeLogger(category, durationId)
    },
    clean() {
      instances.clear()
    },
  }
}

function createToolingModelRegistry(logger) {
  const models = new Map()
  return {
    registry(model) {
      const modelId = String(model?.modelId)
      if (models.has(modelId)) logger.warn(`Tooling model ${modelId} was registered more than once`)
      models.set(modelId, model)
    },
    getModelMap: () => models,
    clear: () => models.clear(),
  }
}

module.exports = { createHvigorPublicFacade, createRuntimeLoggerFactory, createToolingModelRegistry }
