'use strict'

const fs = require('node:fs')
const crypto = require('node:crypto')
const childProcess = require('node:child_process')
const { EventEmitter } = require('node:events')
const Module = require('node:module')
const os = require('node:os')
const path = require('node:path')
const util = require('node:util')
const { Worker } = require('node:worker_threads')
const iconvLite = require('iconv-lite')
const log4js = require('log4js')
const nativeRuntimeBinding = require('./index.js')

// Behavior baseline verified against the hvigor bundled with DevEco Studio
// 6.1.1. Keep this explicit so compatibility changes are evidence-driven.
const HVIGOR_PARITY_VERSION = '6.24.3'
const HVIGOR_TASK_GROUP_TYPE = Object.freeze({
  HELP_TASK_GROUP: 'Help',
  OTHER_TASK_GROUP: 'Other',
  SYNC_TASK_GROUP: 'Sync',
  INIT_TASK_GROUP: 'Init',
  CUSTOMIZE_TASK_GROUP: 'Customize',
})
const HVIGOR_DEFAULT_CONFIG_FILE_NAME = 'hvigor-config.json5'
const HVIGOR_INCREMENTAL_OPTIMIZATION = 'hvigor.task.schedule.optimization'
const OHOS_ARK_COMPILE_SOURCE_MAP_DIR = 'ohos.arkCompile.sourceMapDir'
const HVIGOR_MAIN_THREAD = 'Main Thread'
const HVIGOR_CUSTOM_WORKER_REQUEST = 'CUSTOM_WORKER_REQUEST'
const HVIGOR_ANALYZE_MODE_MAP = new Map([
  ['default', 0],
  ['verbose', 1],
  [false, 2],
  ['false', 2],
  ['normal', 0],
  ['advanced', 1],
  ['ultrafine', 3],
])
const HVIGOR_TASK_STATE = Object.freeze({
  WAITING: 'waiting',
  RUNNING: 'running',
  END: 'end',
  REJECT: 'reject',
  ERROR: 'error',
})
const HVIGOR_WORKER_PRIORITY = Object.freeze({
  0: 'FIRST',
  1: 'HEAVY',
  2: 'MEDIUM',
  3: 'LIGHT',
  4: 'LAST',
  FIRST: 0,
  HEAVY: 1,
  MEDIUM: 2,
  LIGHT: 3,
  LAST: 4,
})
const HVIGOR_POOL_STATE = Object.freeze({
  INIT: 'init',
  RUNNING: 'running',
  STOP: 'stop',
  CLOSE: 'close',
  TERMINATED: 'terminated',
})
const HVIGOR_WORKER_EVENT = Object.freeze({
  CLEAN_HVIGOR_CONFIG_LOADER: 'CLEAN_HVIGOR_CONFIG_LOADER',
  CLEAN_HVIGOR_LOGGER: 'CLEAN_HVIGOR_LOGGER',
  UNMOUNT_TSC_COMMON_CACHE_EVENT: 'UNMOUNT_TSC_COMMON_CACHE_EVENT',
  UPDATE_LOG_LEVEL: 'UPDATE_LOG_LEVEL',
  CUSTOM_WORKER_REQUEST: 'CUSTOM_WORKER_REQUEST',
})
const HVIGOR_WATCH_EVENT = Object.freeze({
  WATCH_START: 'WatchStart',
  WATCH_RESULT: 'WatchResult',
  CLOSE_WATCH: 'CloseWatch',
  WATCH_COMPILE_RESULT: 'WatchCompileResult',
  WATCH_COMPILE_DATA: 'WatchCompileData',
  WATCH_LOG: 'WatchLog',
  TERMINATE_WORKER: 'TerminateWorker',
  NEW_WATCH_WORKER: 'NewWatchWorker',
  SEND_KEEP_ALIVE_SIGNAL: 'SendKeepAliveSignal',
})
const HVIGOR_METRIC_EVENT_TYPE = Object.freeze({
  DURATION: 'duration',
  INSTANT: 'instant',
  COUNTER: 'counter',
  GAUGE: 'gauge',
  OBJECT: 'object',
  METADATA: 'metadata',
  MARK: 'mark',
  LOG: 'log',
  CONTINUAL: 'continual',
})
const HVIGOR_METRIC_LOG_TYPE = Object.freeze({
  DEBUG: 'debug',
  INFO: 'info',
  WARN: 'warn',
  ERROR: 'error',
  DETAIL: 'detail',
})
const HVIGOR_MARK_EVENT_TYPE = Object.freeze({ HISTORY: 'history', OTHER: 'other' })
const HVIGOR_MARK_EVENT_STATE = Object.freeze({
  SUCCESS: 'success',
  FAILED: 'failed',
  RUNNING: 'running',
})
const HVIGOR_DURATION_EVENT_STATE = Object.freeze({
  CREATED: 'created',
  BEGINNING: 'beginning',
  RUNNING: 'running',
  FAILED: 'failed',
  SUCCESS: 'success',
  WARN: 'warn',
})
const OHOS_COMMAND_BUILDER_TYPE = Object.freeze({
  0: 'NONE',
  1: 'CMAKE',
  2: 'NINJA',
  NONE: 0,
  CMAKE: 1,
  NINJA: 2,
})

class HvigorPoolConstant {
  static MAX_POOL_NUM = os.cpus().length - 1
  static MIN_POOL_NUM = 0
  static DEFAULT_MAX_IDLE_TIME = 600000
  static DEFAULT_RECYCLE_INTERVAL = 600000
  static WORKER_ACTION_PATH = './worker-action.js'
  static WORK_DONE = 'work done'
  static WORK_ERROR = 'work error'
  static CALLBACK_ERROR = 'callback error'
  static FILE_SUFFIX = '.js'
  static UNKNOWN_LOG_TIME = 'unknown'
  static ENCODING = 'utf8'
  static TIME_PREFIX_LENGTH = 25
  static CPU_LIMIT = 0.8
  static FAIL_DISPATCH_INTERVAL = 100
  static MAX_FAIL_ATTEMPTS = 5
  static TSC_COMMON_CACHE_KEY = 'common'
}

class HvigorExtendedErrorInfo extends Error {
  constructor(message, extraInfo, stack) {
    super(message)
    this.extraInfo = extraInfo
    if (stack) this.stack = stack
  }
}

class HvigorSystemPlugin {
  constructor(pluginId) {
    this.pluginVersion = HVIGOR_PARITY_VERSION
    this.pluginId = pluginId
  }

  getPluginId() {
    return this.pluginId
  }
}

class HvigorTcbStore {
  static tcbMap = new Map()

  static getTCB(id) {
    return this.tcbMap.get(id)
  }

  static clear() {
    this.tcbMap.clear()
  }

  static add(tcb) {
    this.tcbMap.set(tcb.getId(), tcb)
  }
}

class HvigorTaskDirectedAcyclicGraph {
  constructor() {
    this._out = new Map()
    this._in = new Map()
  }

  addNode(nodeKey) {
    if (!this._out.has(nodeKey)) this._out.set(nodeKey, new Set())
    if (!this._in.has(nodeKey)) this._in.set(nodeKey, new Set())
  }

  addEdge(originKey, targetKey) {
    this.addNode(originKey)
    this.addNode(targetKey)
    this._out.get(originKey).add(targetKey)
    this._in.get(targetKey).add(originKey)
    return true
  }

  _hasEdge(originKey, targetKey, visited = new Set()) {
    if (originKey === targetKey) return true
    if (visited.has(originKey)) return false
    visited.add(originKey)
    for (const child of this.getChildren(originKey)) {
      if (this._hasEdge(child, targetKey, visited)) return true
    }
    return false
  }

  removeNode(nodeKey) {
    for (const child of this._out.get(nodeKey) || []) this._in.get(child)?.delete(nodeKey)
    for (const parent of this._in.get(nodeKey) || []) this._out.get(parent)?.delete(nodeKey)
    this._out.delete(nodeKey)
    this._in.delete(nodeKey)
  }

  clear() {
    this._out.clear()
    this._in.clear()
  }

  getChildren(nodeKey) {
    return this._out.get(nodeKey) || new Set()
  }

  getParent(nodeKey) {
    return this._in.get(nodeKey) || new Set()
  }

  getAllStartNodes() {
    return this.findAllZeroEdgeNodes(this._in)
  }

  getAllEndNodes() {
    return this.findAllZeroEdgeNodes(this._out)
  }

  checkCircle() {
    const visited = new Set()
    const active = new Set()
    const pathStack = []
    for (const nodeKey of this._out.keys()) {
      const circlePath = this.dfsCircleCheck(nodeKey, visited, active, pathStack)
      if (circlePath) return { hasCircle: true, circlePath }
    }
    return { hasCircle: false, circlePath: [] }
  }

  dfsCircleCheck(nodeKey, visited, active, pathStack) {
    if (active.has(nodeKey)) {
      const start = pathStack.indexOf(nodeKey)
      return [...pathStack.slice(start), nodeKey]
    }
    if (visited.has(nodeKey)) return undefined
    visited.add(nodeKey)
    active.add(nodeKey)
    pathStack.push(nodeKey)
    for (const child of this.getChildren(nodeKey)) {
      const circlePath = this.dfsCircleCheck(child, visited, active, pathStack)
      if (circlePath) return circlePath
    }
    pathStack.pop()
    active.delete(nodeKey)
    return undefined
  }

  findAllZeroEdgeNodes(edges) {
    const result = new Set()
    for (const [nodeKey, adjacent] of edges) {
      if (adjacent.size === 0) result.add(nodeKey)
    }
    return result
  }
}

class HvigorBuildConst {
  static MODULE_MODE = 'module'
  static PROJECT_MODE = 'project'
}

class HvigorCommonConst {
  static BUILD_PROFILE_FILE_NAME = 'build-profile'
  static BUILD_PROFILE_FILE = 'build-profile.json5'
  static BUILD_FILE_NAME = 'hvigorfile'
  static PACKAGE_JSON = 'package.json'
  static OH_PACKAGE_JSON = 'oh-package.json5'
  static BUILD_FILE_NAME_SUFFIX = ['.ts', '.js', '.cjs', '.mjs']
  static HVIGOR_CONFIG_FILE_NAME = 'hvigorconfig'
  static HVIGOR_CONFIG_FILE_NAME_SUFFIX = ['.ts', '.js', '.cjs', '.mjs']
}

function hvigorIsWindows() {
  return os.type() === 'Windows_NT'
}

function hvigorIsLinux() {
  return os.type() === 'Linux'
}

function hvigorIsMac() {
  return os.type() === 'Darwin'
}

function hvigorMaxPathLength() {
  return hvigorIsMac() ? 1016 : hvigorIsWindows() ? 259 : 4095
}

function hvigorHash(data, algorithm) {
  return crypto.createHash(algorithm || 'sha256').update(data).digest('hex')
}

function hvigorHashFile(filePath, algorithm) {
  if (fs.existsSync(filePath)) {
    return hvigorHash(fs.readFileSync(filePath, 'utf8'), algorithm)
  }
  return undefined
}

function hvigorReplacer(_key, value) {
  if (value instanceof Map) {
    const object = Object.create(null)
    value.forEach((entry, key) => {
      object[key] = entry
    })
    return object
  }
  if (value instanceof Set) {
    return Array.from(value)
  }
  return value
}

function hvigorReplaceBundleName(value, bundleName, regexp) {
  if (!bundleName?.length) return value
  const matcher = regexp || new RegExp(bundleName, 'ig')
  if (typeof value === 'string' && matcher.test(value)) {
    return value.replace(matcher, (matched) => `${matched[0]}***${matched[matched.length - 1]}`)
  }
  if (Array.isArray(value)) {
    return value.map((entry) => hvigorReplaceBundleName(entry, bundleName, matcher))
  }
  if (typeof value === 'object') {
    return Object.keys(value).reduce(
      (output, key) => ({ ...output, [key]: hvigorReplaceBundleName(value[key], bundleName, matcher) }),
      {},
    )
  }
  return value
}

function hvigorNoop() {}

const HVIGOR_TIME_SHORT_DESCRIPTIONS = ['h', 'min', 's', 'ms', 'μs', 'ns']
const HVIGOR_TIME_LONG_DESCRIPTIONS = [
  'hour',
  'minute',
  'second',
  'millisecond',
  'microsecond',
  'nanosecond',
]
const HVIGOR_TIME_CONVERSIONS = [3600, 60, 1, 1e6, 1e3, 1]
const HVIGOR_TIME_UNIT_INDEX = new Map([
  ['hour', 0],
  ['hours', 0],
  ['minute', 1],
  ['minutes', 1],
  ['second', 2],
  ['seconds', 2],
  ['millisecond', 3],
  ['milliseconds', 3],
  ['microsecond', 4],
  ['microseconds', 4],
  ['nanosecond', 5],
  ['nanoseconds', 5],
])

function hvigorPrettyHrtime(value, options) {
  if (!Array.isArray(value) || value.length !== 2 || value.some((entry) => typeof entry !== 'number')) {
    return ''
  }
  const verbose = options?.verbose || false
  const precise = options?.precise || false
  if (value[1] < 0) {
    const normalized = value[0] + value[1] / 1e9
    value[0] = Number.parseInt(normalized.toString())
    value[1] = 1e9 * Number.parseFloat((normalized % 1).toPrecision(9))
  }
  let output = ''
  for (let index = 0; index < 6; index += 1) {
    let unitValue = value[index < 3 ? 0 : 1]
    if (index !== 3 && index !== 0) unitValue %= HVIGOR_TIME_CONVERSIONS[index - 1]
    if (index === 2) unitValue += value[1] / 1e9
    let converted = unitValue / HVIGOR_TIME_CONVERSIONS[index]
    if (converted < 1) continue
    if (verbose) converted = Math.floor(converted)
    let rendered = precise ? converted.toString() : converted.toFixed(converted >= 10 ? 0 : 2)
    if (rendered.includes('.') && rendered.endsWith('0')) rendered = rendered.replace(/\.?0+$/, '')
    if (output) output += ' '
    output += rendered
    if (verbose) {
      output += ` ${HVIGOR_TIME_LONG_DESCRIPTIONS[index]}`
      if (rendered !== '1') output += 's'
    } else {
      output += ` ${HVIGOR_TIME_SHORT_DESCRIPTIONS[index]}`
      break
    }
  }
  return output
}

function hvigorSplitTime(value) {
  const parts = value.split(' ')
  if (parts.length < 1 || parts.length % 2 !== 0) return value
  let output = ''
  for (let index = 0; index < parts.length; index += 2) {
    let amount = parts[index]
    const unitIndex = HVIGOR_TIME_UNIT_INDEX.get(parts[index + 1])
    const unit = HVIGOR_TIME_SHORT_DESCRIPTIONS[unitIndex]
    if (index === 0 && unitIndex > 3) return '1 ms '
    if (unitIndex > 3) break
    if (unit === 'ms' && index < parts.length - 2) amount = String(Number.parseInt(amount) + 1)
    output += `${amount} ${unit} `
  }
  return output
}

function hvigorFormatTime(value) {
  return hvigorSplitTime(hvigorPrettyHrtime(value, { verbose: true }))
}

function hvigorFormatTimeToNumber(value) {
  return value ? 1e9 * value[0] + value[1] : Number(process.hrtime.bigint())
}

function parseHvigorJsonText(text, location = false) {
  let bridge
  try {
    bridge = JSON.parse(nativeRuntimeBinding.parseHvigorJsonText(String(text), location))
  } catch (cause) {
    throw new SyntaxError(cause instanceof Error ? cause.message : String(cause), { cause })
  }
  return reviveHvigorJsonBridge(bridge)
}

function reviveHvigorJsonBridge(bridge) {
  const location = bridge.location
    ? { _line: bridge.location.line, _column: bridge.location.column }
    : undefined
  switch (bridge.kind) {
    case 'null':
      return location ? { value: null, ...location } : null
    case 'boolean':
    case 'string':
      return location ? { value: bridge.value, ...location } : bridge.value
    case 'number': {
      const value = Number(bridge.value)
      return location ? { value, ...location } : value
    }
    case 'array': {
      const output = bridge.values.map(reviveHvigorJsonBridge)
      if (location) Object.assign(output, location)
      return output
    }
    case 'object': {
      const output = location ? { ...location } : {}
      for (const property of bridge.properties) {
        output[property.key] = reviveHvigorJsonBridge(property.value)
      }
      return output
    }
    default:
      throw new TypeError(`Invalid native hvigor JSON bridge kind '${bridge.kind}'`)
  }
}

class HvigorReplacePlaceHolderUtil {
  static instance

  static getInstance() {
    this.instance ||= new HvigorReplacePlaceHolderUtil()
    return this.instance
  }

  replacePlaceholders(value, params) {
    if (Array.isArray(value)) return value.map((entry) => this.replacePlaceholders(entry, params))
    if (typeof value !== 'object' || value === null) return value
    const output = { ...value }
    if (Array.isArray(output.sourceRoots)) output.sourceRoots = this.replacePathArray(output.sourceRoots, params)
    if (Array.isArray(output.directories)) output.directories = this.replacePathArray(output.directories, params)
    if (typeof output.buildProfileFields === 'object' && output.buildProfileFields !== null) {
      output.buildProfileFields = this.replaceBuildProfileFields(output.buildProfileFields, params)
    }
    for (const [key, entry] of Object.entries(output)) {
      if (!['sourceRoots', 'directories', 'buildProfileFields'].includes(key)) {
        output[key] = this.replacePlaceholders(entry, params)
      }
    }
    return output
  }

  replaceBuildProfileFields(value, params) {
    const output = { ...value }
    for (const [key, entry] of Object.entries(output)) {
      if (typeof entry === 'string') output[key] = this.replaceStringPlaceholders(entry, params)
    }
    return output
  }

  replacePathArray(value, params) {
    return value.map((entry) =>
      typeof entry === 'string' ? this.replaceStringPlaceholders(entry, params) : entry,
    )
  }

  replaceStringPlaceholders(value, params) {
    return value.replace(/@param:([a-zA-Z0-9_.-]+)/g, (placeholder, parameterPath) => {
      const replacement = this.getValueByPath(params, parameterPath)
      return replacement === undefined ? placeholder : replacement
    })
  }

  getValueByPath(value, parameterPath) {
    if (!value || typeof value !== 'object') return undefined
    let current = value
    for (const segment of parameterPath.split('.')) {
      if (current === undefined) return undefined
      current = current[segment]
    }
    return current
  }
}

function createHvigorPublicUtilities(host) {
  const customizedParams = () => host._parameter.getCoreStartParams().customizedParam || {}
  const replaceBuildProfileParameters = (filePath, value) =>
    typeof filePath === 'string' && filePath.endsWith(HvigorCommonConst.BUILD_PROFILE_FILE)
      ? HvigorReplacePlaceHolderUtil.getInstance().replacePlaceholders(value, customizedParams())
      : value

  class ParseJsonFile {
    static parseJsonFile(filePath, location = false, encoding = 'utf-8') {
      const resolved = path.resolve(filePath)
      const value = parseHvigorJsonText(fs.readFileSync(resolved, { encoding }), location)
      return replaceBuildProfileParameters(filePath, value)
    }
  }

  const parseJsonFile = (filePath, location = false, encoding = 'utf-8') =>
    ParseJsonFile.parseJsonFile(filePath, location, encoding)

  class Json5Reader {
    static getJson5Obj(filePath, encoding = 'utf-8') {
      if (!fs.existsSync(filePath)) throw new Error(`FILE_NOT_EXIST: ${filePath}`)
      const value = parseHvigorJsonText(fs.readFileSync(path.resolve(filePath), { encoding }))
      return replaceBuildProfileParameters(filePath, value)
    }

    static async readJson5File(filePath, encoding = 'utf-8') {
      return parseHvigorJsonText(await fs.promises.readFile(filePath, { encoding }))
    }

    static getJson5ObjProp(value, propertyPath) {
      let current = value
      for (const segment of propertyPath.split('.')) {
        if (current?.[segment] === undefined) return undefined
        current = current[segment]
      }
      return current
    }
  }

  const filePath = (file) => (typeof file === 'string' ? file : file?.filePath)
  const requireFile = (file) => {
    const resolved = filePath(file)
    if (typeof resolved !== 'string' || !fs.existsSync(resolved) || !fs.statSync(resolved).isFile()) {
      throw new Error(`The path is not a file: ${String(resolved)}`)
    }
    return resolved
  }

  class FileUtil {
    static exist(candidate) {
      return typeof candidate === 'string' ? fs.existsSync(candidate) : false
    }

    static existInternal(candidate) {
      return this.exist(candidate)
    }

    static isDictionary(file) {
      const candidate = filePath(file)
      return typeof candidate === 'string' && fs.existsSync(candidate) && fs.statSync(candidate).isDirectory()
    }

    static isDictionaryInternal(file) {
      return this.isDictionary(file)
    }

    static isFile(file) {
      const candidate = filePath(file)
      return typeof candidate === 'string' && fs.existsSync(candidate) && fs.statSync(candidate).isFile()
    }

    static isFileInternal(file) {
      return this.isFile(file)
    }

    static ensureDirSync(directory) {
      if (!fs.existsSync(directory)) fs.mkdirSync(directory, { recursive: true })
    }

    static ensureDirSyncInternal(directory) {
      return this.ensureDirSync(directory)
    }

    static ensureFileSync(output) {
      if (!fs.existsSync(output)) {
        fs.mkdirSync(path.dirname(output), { recursive: true })
        fs.closeSync(fs.openSync(output, 'a'))
      }
    }

    static ensureFileSyncInternal(output) {
      return this.ensureFileSync(output)
    }

    static readFileSync(file) {
      return fs.readFileSync(requireFile(file))
    }

    static readFileSyncInternal(file) {
      return this.readFileSync(file)
    }

    static readFile(file) {
      return fs.promises.readFile(requireFile(file))
    }

    static readFileInternal(file) {
      return this.readFile(file)
    }

    static readJson5(file) {
      return Json5Reader.getJson5Obj(requireFile(file))
    }

    static readJson5Internal(file) {
      return this.readJson5(file)
    }

    static writeFile(file, content) {
      return fs.promises.writeFile(requireFile(file), content, 'utf-8')
    }

    static writeFileInternal(file, content) {
      return this.writeFile(file, content)
    }

    static writeFileSync(file, content) {
      fs.writeFileSync(requireFile(file), content, 'utf-8')
    }

    static writeFileSyncInternal(file, content) {
      return this.writeFileSync(file, content)
    }

    static copyFile(file, destination) {
      return fs.promises.copyFile(requireFile(file), destination)
    }

    static copyFileInternal(file, destination) {
      return this.copyFile(file, destination)
    }

    static copyFileSync(file, destination) {
      fs.copyFileSync(requireFile(file), destination)
    }

    static copyFileSyncInternal(file, destination) {
      return this.copyFileSync(file, destination)
    }

    static pathResolve(...paths) {
      return path.resolve(...paths)
    }

    static pathResolveInternal(...paths) {
      return this.pathResolve(...paths)
    }
  }

  class PathUtil {
    static getHvigorCacheDir(logger) {
      return host.hvigorCacheDir(logger)
    }

    static checkCopyPathIsSame(source, destination) {
      const sourceStats = this.getStatsSync(source)
      const destinationStats = this.getStatsSync(destination)
      return Boolean(
        sourceStats &&
          destinationStats &&
          destinationStats.ino &&
          destinationStats.dev &&
          destinationStats.ino === sourceStats.ino &&
          destinationStats.dev === sourceStats.dev,
      )
    }

    static getStatsSync(candidate) {
      try {
        return fs.statSync(candidate)
      } catch {
        return null
      }
    }

    static getReportDirPath() {
      return path.resolve(this.getHvigorCacheDir(), 'report')
    }
  }

  const logFilePath = () => path.resolve(PathUtil.getHvigorCacheDir(), 'outputs', 'build-logs')
  let loggingConfiguration = {
    appenders: {
      debug: {
        type: 'stdout',
        layout: { type: 'pattern', pattern: '[%d] > hvigor %p %c %[%m%]' },
      },
      'debug-log-file': {
        type: 'file',
        filename: path.resolve(logFilePath(), 'build.log'),
        maxLogSize: 2097152,
        backups: 9,
        encoding: 'utf-8',
        level: 'debug',
      },
      info: {
        type: 'stdout',
        layout: { type: 'pattern', pattern: '[%d] > hvigor %[%m%]' },
      },
      'no-pattern-info': {
        type: 'stdout',
        layout: { type: 'pattern', pattern: '%m' },
      },
      wrong: {
        type: 'stderr',
        layout: { type: 'pattern', pattern: '[%d] > hvigor %[%p: %m%]' },
      },
      'just-debug': { type: 'logLevelFilter', appender: 'debug', level: 'debug', maxLevel: 'debug' },
      'just-info': { type: 'logLevelFilter', appender: 'info', level: 'info', maxLevel: 'info' },
      'just-wrong': { type: 'logLevelFilter', appender: 'wrong', level: 'warn', maxLevel: 'error' },
    },
    categories: {
      default: { appenders: ['just-debug', 'just-info', 'just-wrong'], level: 'debug' },
      'no-pattern-info': { appenders: ['no-pattern-info'], level: 'info' },
      'debug-file': { appenders: ['debug-log-file'], level: 'debug' },
    },
  }
  let contextLogLevel = log4js.levels.DEBUG
  const updateLoggingConfiguration = () => {
    const fileAppender = loggingConfiguration.appenders['debug-log-file']
    if (fileAppender && Object.prototype.hasOwnProperty.call(fileAppender, 'filename')) {
      fileAppender.filename = path.resolve(logFilePath(), 'build.log')
    }
    return loggingConfiguration
  }
  const getConfiguration = () => loggingConfiguration
  const setCategoriesLevel = (level, ignoredCategories) => {
    const resolved = log4js.levels.getLevel(level, contextLogLevel)
    contextLogLevel = resolved
    for (const [category, configuration] of Object.entries(loggingConfiguration.categories)) {
      if (ignoredCategories?.includes(category) || category.includes('file')) continue
      configuration.level = resolved.levelStr
    }
  }
  const levelMap = new Map()
  for (const name of ['ALL', 'MARK', 'TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR', 'FATAL', 'OFF']) {
    levelMap.set(name, log4js.levels[name])
    levelMap.set(name.toLowerCase(), log4js.levels[name])
  }
  const LogLevelMap = new Map([
    ['info', log4js.levels.INFO],
    ['debug"', log4js.levels.DEBUG],
    ['warn"', log4js.levels.WARN],
    ['error"', log4js.levels.ERROR],
  ])

  class HvigorFileLogger {
    constructor(logger) {
      this.fileLogger = logger
    }

    anonymize(value) {
      if (Array.isArray(value)) return value.map((entry) => this.anonymize(entry))
      if (!value || typeof value !== 'object') return value
      const output = {}
      for (const [key, entry] of Object.entries(value)) {
        if (key === 'bundleName' && typeof entry === 'string') {
          output[key] = entry ? `${entry[0]}***${entry[entry.length - 1]}` : '*****'
        } else {
          output[key] = this.anonymize(entry)
        }
      }
      return output
    }

    debug(message, ...args) {
      const values = [message, ...args].map((entry) => this.anonymize(entry))
      this.fileLogger.debug(...values)
      return values
    }

    log(level, ...args) {
      this.fileLogger.log(level, ...args.map((entry) => this.anonymize(entry)))
    }

    warn(message, ...args) {
      this.fileLogger.warn(...[message, ...args].map((entry) => this.anonymize(entry)))
    }

    info(message, ...args) {
      this.fileLogger.info(...[message, ...args].map((entry) => this.anonymize(entry)))
    }

    error(message, ...args) {
      this.fileLogger.error(...[message, ...args].map((entry) => this.anonymize(entry)))
    }
  }

  class HvigorErrorAdaptorCompatibility {
    constructor(errorId) {
      this.errorId = errorId
      this.messages = []
      this.solutions = []
    }

    formatMessage(...messages) {
      this.messages = messages
      return this
    }

    formatSolutions(index, ...values) {
      this.solutions[index] = values
      return this
    }

    getErrorMessage() {
      const details = this.messages.length ? `: ${this.messages.map(String).join(', ')}` : ''
      return [
        {
          code: this.errorId,
          message: `${this.errorId}${details}`,
          solutions: this.solutions.filter(Boolean).map((values) => values.map(String).join(' ')),
        },
      ]
    }
  }

  class HvigorLogger {
    static instanceMap = new Map()

    constructor(category) {
      log4js.configure(updateLoggingConfiguration())
      this._logger = log4js.getLogger(category)
      this._logger.level = contextLogLevel
      this._filelogger = log4js.getLogger('debug-file')
      this.anonymizeFileLogger = new HvigorFileLogger(this._filelogger)
      this.durationId = undefined
    }

    static getInstance(logger, category) {
      const key = `${logger.name}:${category}`
      if (!this.instanceMap.has(key)) this.instanceMap.set(key, new logger(category))
      return this.instanceMap.get(key)
    }

    static getLogger(category) {
      return this.getInstance(HvigorLogger, category)
    }

    static getLoggerWithDurationId(category, durationId) {
      const logger = Object.setPrototypeOf({ ...this.getInstance(HvigorLogger, category) }, HvigorLogger.prototype)
      logger.durationId = durationId
      return logger
    }

    static clean() {
      this.instanceMap.clear()
    }

    log(level, ...args) {
      this.createLogEventByDurationId(level, 'info', ...args)
      this._logger.log(level, ...args)
      this._filelogger.log(level, ...args)
    }

    debug(message, ...args) {
      this.createLogEventByDurationId(message, 'debug', ...args)
      this._logger.debug(message, ...args)
      this._filelogger.debug(message, ...args)
    }

    info(message, ...args) {
      this.createLogEventByDurationId(message, 'info', ...args)
      this._logger.info(message, ...args)
      this._filelogger.info(message, ...args)
    }

    warn(message, ...args) {
      if (message === undefined || message === '') return
      this.createLogEventByDurationId(message, 'warn', ...args)
      this._logger.warn(message, ...args)
      this._filelogger.warn(message, ...args)
    }

    error(message, ...args) {
      this.createLogEventByDurationId(message, 'error', ...args)
      this._logger.error(message, ...args)
      this._filelogger.error(message, ...args)
    }

    anonymizeDebug(message, ...args) {
      this._logger.debug(message, ...args)
      const [anonymizedMessage, ...anonymizedArgs] = this.anonymizeFileLogger.debug(message, ...args)
      this.createLogEventByDurationId(anonymizedMessage, 'debug', ...anonymizedArgs)
    }

    _printTaskExecuteInfo(taskPath, time) {
      this.info(`Finished :${taskPath}... after ${time}`)
    }

    _printFailedTaskInfo(taskPath) {
      this.error(`Failed :${taskPath}... `)
    }

    _printDisabledTaskInfo(taskPath) {
      this.info(`Disabled :${taskPath}... `)
    }

    _printUpToDateTaskInfo(taskPath) {
      this.info(`UP-TO-DATE :${taskPath}...  `)
    }

    _printStackErrorToFile(message, ...args) {
      this._filelogger.error(message, ...args)
    }

    errorMessageExit(message, ...args) {
      throw new Error(util.format(message, ...args))
    }

    errorExit(error, message, ...args) {
      if (message) {
        this._logger.error(message, args)
        this._filelogger.error(message, args)
      }
      this._logger.error(error.stack)
      this._filelogger.error(error.stack)
      if (error.stack) throw error
    }

    getLevel() {
      return this._logger.level
    }

    setLevel(level) {
      this._logger.level = level
    }

    createLogEventByDurationId(message, logType, ...args) {
      if (typeof message === 'string' && host._metricCompatibility) {
        const event = host._metricCompatibility.MetricFactory.createLogEvent(this.getMessage(message, ...args), logType)
        if (this.durationId) event.setDurationId(this.durationId)
      }
      return message
    }

    getMessage(message, ...args) {
      return args.length ? util.format(message, ...args) : message
    }

    getAdaptor(errorId) {
      return new HvigorErrorAdaptorCompatibility(errorId)
    }

    combinePhase(adaptorErrorMessage) {
      if (typeof adaptorErrorMessage === 'string') return adaptorErrorMessage
      return adaptorErrorMessage?.message || util.inspect(adaptorErrorMessage)
    }

    formatErrorAdaptor(errorId, messages, solutions) {
      let adaptor = this.getAdaptor(errorId)
      if (messages) adaptor = adaptor.formatMessage(...messages)
      if (solutions) solutions.forEach((values, index) => adaptor.formatSolutions(index, ...values))
      return adaptor
    }

    printErrorWithAdaptorErrorMessage(adaptorErrorMessages, trySuggestion = '') {
      if (!Array.isArray(adaptorErrorMessages) || adaptorErrorMessages.length === 0) return
      this._logger.error(this.combinePhase(adaptorErrorMessages[0]) + trySuggestion)
      for (const message of adaptorErrorMessages.slice(1)) this.combinePhase(message)
    }

    printError(errorId, messages, solutions) {
      this.printErrorWithAdaptorErrorMessage(this.formatErrorAdaptor(errorId, messages, solutions).getErrorMessage())
    }

    printErrorExit(errorId, messages, solutions, stack) {
      const adaptor = this.formatErrorAdaptor(errorId, messages, solutions)
      const message = this.combinePhase(adaptor.getErrorMessage()[0])
      const error = new Error(message)
      error.name = 'AdaptorError'
      error.code = errorId
      if (stack) error.stack = stack
      throw error
    }

    printErrorExitWithoutStack(errorId, messages, solutions) {
      const adaptor = this.formatErrorAdaptor(errorId, messages, solutions)
      const error = new Error(this.combinePhase(adaptor.getErrorMessage()[0]))
      error.name = 'AdaptorError'
      error.code = errorId
      throw error
    }

    static redirectConsole(logger) {
      const snapshot = { ...console }
      console.debug = (...args) => logger.debug(...args)
      console.log = (...args) => logger.info(...args)
      console.info = (...args) => logger.info(...args)
      console.warn = (...args) => logger.warn(...args)
      console.error = (...args) => logger.error(...args)
      return snapshot
    }

    static restoreConsole(snapshot) {
      console.debug = snapshot.debug
      console.log = snapshot.log
      console.info = snapshot.info
      console.warn = snapshot.warn
      console.error = snapshot.error
    }
  }

  log4js.configure(updateLoggingConfiguration())

  const parameterRecordProxy = (read, set, replace) =>
    new Proxy(Object.create(null), {
      get: (_target, key) => (typeof key === 'string' ? read()[key] : undefined),
      set: (_target, key, value) => {
        if (typeof key === 'string') set(key, value)
        return true
      },
      deleteProperty: (_target, key) => {
        if (typeof key === 'string') {
          const next = { ...read() }
          delete next[key]
          replace(next)
        }
        return true
      },
      ownKeys: () => Reflect.ownKeys(read()),
      has: (_target, key) => Object.prototype.hasOwnProperty.call(read(), key),
      getOwnPropertyDescriptor: (_target, key) =>
        Object.prototype.hasOwnProperty.call(read(), key)
          ? { configurable: true, enumerable: true, writable: true, value: read()[key] }
          : undefined,
    })
  const propertiesProxy = parameterRecordProxy(
    () => host._parameter.getPropertiesInternal(),
    (key, value) => host._parameter.setPropertyInternal(key, value),
    (value) => host._parameter.replacePropertiesInternal(value),
  )
  const extParamsProxy = parameterRecordProxy(
    () => host._parameter.getExtParamsInternal(),
    (key, value) => {
      const next = { ...host._parameter.getExtParamsInternal(), [key]: value }
      host._parameter.replaceExtParamsInternal(next)
    },
    (value) => host._parameter.replaceExtParamsInternal(value),
  )
  const coreStartParamsProxy = new Proxy(Object.create(null), {
    get: (_target, key) => {
      const value = host._parameter.getCoreStartParams()[key]
      if (key === 'logLevel') return log4js.levels.getLevel(value, log4js.levels.INFO)
      return value
    },
    set: (_target, key, value) => {
      const startParams = host._parameter.getCoreStartParams()
      if (key === 'logLevel') {
        startParams.logLevel = log4js.levels.getLevel(value, log4js.levels.INFO).levelStr.toLowerCase()
      } else {
        startParams[key] = value
      }
      return true
    },
    ownKeys: () => Reflect.ownKeys(host._parameter.getCoreStartParams()),
    has: (_target, key) => Object.prototype.hasOwnProperty.call(host._parameter.getCoreStartParams(), key),
    getOwnPropertyDescriptor: (_target, key) =>
      Object.prototype.hasOwnProperty.call(host._parameter.getCoreStartParams(), key)
        ? {
            configurable: true,
            enumerable: true,
            writable: true,
            value: coreStartParamsProxy[key],
          }
        : undefined,
  })
  const coreParameter = {
    get properties() {
      return propertiesProxy
    },
    set properties(value) {
      host._parameter.replacePropertiesInternal(value)
    },
    get extParams() {
      return extParamsProxy
    },
    set extParams(value) {
      host._parameter.replaceExtParamsInternal(value)
    },
    get startParams() {
      return coreStartParamsProxy
    },
    get workspaceDir() {
      return host._parameter.getWorkspaceDirInternal()
    },
    set workspaceDir(value) {
      host._parameter.setWorkspaceDirInternal(value)
    },
    clean() {
      host._parameter.replacePropertiesInternal({
        enableSignTask: true,
        skipNativeIncremental: false,
        'hvigor.keepDependency': true,
      })
      host._parameter.replaceExtParamsInternal({})
      host._parameter.replaceCoreStartParams({
        hvigorfileTypeCheck: false,
        parallelExecution: true,
        incrementalExecution: true,
        printStackTrace: false,
        daemon: true,
        analyze: 0,
        logLevel: 'info',
        optimizationStrategy: 'memory',
        hotCompile: false,
        hotReloadBuild: false,
        customizedParam: undefined,
      })
      host._parameter.setWorkspaceDirInternal('')
    },
  }

  class HvigorConfigLoader {
    static instance
    static config

    constructor() {
      this.configPropertyMap = new Map()
      const projectConfigPath = path.resolve(host.projectRoot, 'hvigor', HVIGOR_DEFAULT_CONFIG_FILE_NAME)
      if (!fs.existsSync(projectConfigPath)) return
      const projectConfig = ParseJsonFile.parseJsonFile(projectConfigPath)
      const userConfigPath = path.resolve(getHvigorUserHome(), HVIGOR_DEFAULT_CONFIG_FILE_NAME)
      if (fs.existsSync(userConfigPath)) {
        const userConfig = ParseJsonFile.parseJsonFile(userConfigPath)
        projectConfig.properties = { ...userConfig.properties, ...projectConfig.properties }
      }
      this.hvigorConfig = projectConfig
    }

    static init(cliOptions) {
      if (cliOptions === undefined) {
        process.env.config = undefined
        return
      }
      const argvConfig = this.getConfigs()
      let config = {}
      cliOptions.config?.forEach((entry) => {
        const parts = entry.split('=')
        if (parts.length === 2) {
          config[parts[0]] = parts[1]
          this.initCommandLineProperties(parts[0], parts[1])
        }
      })
      if (Array.isArray(cliOptions.prop)) {
        cliOptions.prop.forEach((entry) => {
          const parts = entry.split('=')
          if (parts.length === 2) {
            config[parts[0]] = parts[1]
            this.initCommandLineProperties(parts[0], parts[1])
          }
        })
      }
      config = { ...argvConfig, ...config }
      process.env.config = JSON.stringify(config)
    }

    static initCommandLineProperties(key, value) {
      if (!key.startsWith('properties.')) return
      coreParameter.properties[key.slice('properties.'.length)] = this.convertToParamValue(value)
    }

    static convertToParamValue(value) {
      let converted = Number(value)
      if (value.length <= 16 && !Number.isNaN(converted)) return converted
      converted = value === 'true' || (value !== 'false' && converted)
      return typeof converted === 'boolean' ? converted : value.trim()
    }

    getHvigorConfig() {
      return this.hvigorConfig
    }

    getPropertiesConfigValue(key) {
      if (this.configPropertyMap.has(key)) return this.configPropertyMap.get(key)
      const argvValue = HvigorConfigLoader.getConfigs()[`properties.${key}`]
      const environmentConfig =
        (process.env.config !== undefined ? JSON.parse(process.env.config)[`properties.${key}`] : undefined) ??
        argvValue
      if (environmentConfig !== undefined) {
        const value = this.parseConfigValue(environmentConfig)
        this.configPropertyMap.set(key, value)
        return value
      }
      const value = this.hvigorConfig?.properties?.[key]
      this.configPropertyMap.set(key, value)
      return value
    }

    static clean() {
      this.instance = undefined
      this.config = undefined
    }

    static getInstance() {
      this.instance ||= new HvigorConfigLoader()
      return this.instance
    }

    static getConfigs() {
      if (this.config) return this.config
      const args = process.argv.slice(2)
      const prefixPattern = /^(--config|-c).*/
      const exactPattern = /^(--config|-c)$/
      const output = {}
      for (const [index, argument] of args.entries()) {
        if (exactPattern.test(argument)) {
          if (index + 1 < args.length) this.setConfig(args[index + 1].split('='), output)
        } else if (prefixPattern.test(argument)) {
          const prefix = argument.match(prefixPattern)
          if (prefix && prefix[0].length < argument.length) {
            this.setConfig(argument.slice(prefix[0].length).split('='), output)
          }
        }
      }
      this.config = output
      return output
    }

    static setConfig(parts, output) {
      if (parts.length === 2) output[parts[0]] = parts[parts.length - 1]
    }

    parseConfigValue(value) {
      if (value.toLowerCase() === 'true') return true
      if (value.toLowerCase() === 'false') return false
      const converted = Number(value)
      return Number.isNaN(converted) ? value : converted
    }
  }

  class HvigorLruCache {
    constructor(options) {
      this.capacity = options?.capacity ?? 4
      this.ttl = options?.ttl ?? 3600000
      if (this.capacity < 0) this.capacity = undefined
      if (this.ttl < 0) this.ttl = undefined
      this.keysByRecency = new Map()
    }

    access(key) {
      if (!this.keysByRecency.has(key)) return
      const birth = this.keysByRecency.get(key)
      this.keysByRecency.delete(key)
      this.keysByRecency.set(key, birth)
    }

    set(key) {
      if (this.keysByRecency.has(key)) {
        this.access(key)
        return undefined
      }
      this.keysByRecency.set(key, Date.now())
      if (this.capacity === undefined || this.keysByRecency.size <= this.capacity) return undefined
      const evicted = this.keysByRecency.keys().next().value
      this.keysByRecency.delete(evicted)
      return evicted
    }

    delete(key) {
      this.keysByRecency.delete(key)
    }

    clear() {
      this.keysByRecency.clear()
    }

    getTTL() {
      return this.ttl
    }
  }

  class CacheStore {
    constructor(name, lruCache, pinned = false) {
      this.caches = new Map()
      this.hvigorMemoryThreshold = HvigorConfigLoader.getInstance().getPropertiesConfigValue(
        'hvigor.memoryThreshold',
      )
      this.hvigorEnableMemoryCache = HvigorConfigLoader.getInstance().getPropertiesConfigValue(
        'hvigor.enableMemoryCache',
      )
      this.name = name
      this.lruCache = lruCache
      this.pinned = pinned
    }

    getCache(key) {
      this.lruCache.access(this.completeKey(key))
      return this.caches.get(key)
    }

    get(key) {
      return this.getCache(key)
    }

    set(key, value) {
      return this.setCache(key, value)
    }

    setCache(key, value, pinned = false) {
      if (
        (this.hvigorMemoryThreshold && this.hvigorMemoryThreshold > 0) ||
        this.hvigorEnableMemoryCache === false
      ) {
        return this
      }
      this.caches.set(key, value)
      if (pinned || this.pinned) return this
      const evicted = this.lruCache.set(this.completeKey(key))
      if (evicted) this.caches.delete(this.parseCompleteKey(evicted))
      return this
    }

    hasCache(key) {
      return this.caches.has(key)
    }

    clear() {
      for (const key of this.caches.keys()) this.lruCache.delete(this.completeKey(key))
      this.caches.clear()
    }

    delete(key) {
      this.lruCache.delete(this.completeKey(key))
      return this.caches.delete(key)
    }

    size() {
      return this.caches.size
    }

    keys() {
      return this.caches.keys()
    }

    completeKey(key) {
      return `${this.name}@${key}`
    }

    parseCompleteKey(key) {
      return key.substring(this.name.length + 1)
    }
  }

  class CacheStoreManager {
    constructor(options) {
      this.cacheStores = new Map()
      this.lruCache = new HvigorLruCache(options)
    }

    mount(key, pinned = false) {
      if (!this.cacheStores.has(key)) {
        this.cacheStores.set(key, new CacheStore(key, this.lruCache, pinned))
      }
      return this.cacheStores.get(key)
    }

    unmount(key) {
      this.cacheStores.get(key)?.clear()
      return this.cacheStores.delete(key)
    }

    clear() {
      this.lruCache.clear()
      this.cacheStores.clear()
    }

    size() {
      return this.cacheStores.size
    }

    keys() {
      return this.cacheStores.keys()
    }

    cacheItemSize() {
      let size = 0
      for (const store of this.cacheStores.values()) size += store.size()
      return size
    }
  }

  class LocalFileWriter {
    static instance

    constructor() {
      this._replacer = hvigorReplacer
      this._space = 2
    }

    withSpace(space) {
      this._space = space
    }

    withReplacer(replacer) {
      this._replacer = replacer
    }

    write(filePath, content) {
      try {
        this.writeStr(filePath, JSON.stringify(content, this._replacer, this._space))
      } catch {
        this.writeStrArr(filePath, this.chunkStringify(content))
      }
    }

    chunkStringify(content) {
      const output = ['{\n']
      const indentation = new Array(this._space).fill(' ').join('')
      for (const key of Object.keys(content)) {
        if (Array.isArray(content[key]) && content[key].length) {
          output.push(`${indentation}${JSON.stringify(key)}: [\n`)
          let chunk = ''
          for (const entry of content[key]) {
            const rendered = `${JSON.stringify(entry, this._replacer, this._space)
              .split('\n')
              .map((line) => `${indentation}${indentation}${line}`)
              .join('\n')},\n`
            if (chunk.length >= 1e8) {
              output.push(chunk)
              chunk = rendered
            } else {
              chunk += rendered
            }
          }
          output.push(`${chunk.replace(/,\n$/, '\n')}${indentation}],\n`)
        } else {
          output.push(
            `${indentation}${JSON.stringify(key)}: ${JSON.stringify(content[key], this._replacer, this._space)},\n`,
          )
        }
      }
      output[output.length - 1] = output[output.length - 1].replace(/,\n$/, '\n')
      output.push('}')
      return output
    }

    writeStr(filePath, content) {
      fs.mkdirSync(path.dirname(filePath), { recursive: true })
      fs.writeFileSync(filePath, content)
    }

    writeStrArr(filePath, contentArray) {
      fs.mkdirSync(path.dirname(filePath), { recursive: true })
      fs.writeFileSync(filePath, '')
      for (const content of contentArray) fs.appendFileSync(filePath, content)
    }

    static getInstance() {
      this.instance ||= new LocalFileWriter()
      return this.instance
    }
  }

  class GlobalParam {
    static instance

    static getInstance() {
      this.instance ||= new GlobalParam()
      return this.instance
    }

    constructor() {
      this._log = HvigorLogger.getLogger('Init global params')
      this.configuration = new Map()
      this._initialized = false
    }

    load() {
      const executableName = hvigorIsWindows() ? 'npm.cmd' : 'npm'
      const executable = path.resolve(path.dirname(process.execPath), executableName)
      if (!fs.existsSync(executable)) {
        this._log.debug('Npm not found.')
        return
      }
      const output = childProcess.execSync(`"${executable}" config list -l --json`, {
        encoding: 'utf-8',
        windowsHide: true,
      })
      try {
        const parsed = JSON.parse(output)
        for (const [key, value] of Object.entries(parsed)) this.configuration.set(key, value)
      } catch (error) {
        this._log.error(error.message)
      }
      this._initialized = true
    }

    getConfig(key) {
      this.check()
      return this.configuration.get(key)
    }

    setConfig(key, value) {
      this.configuration.set(key, value)
    }

    check() {
      if (!this._initialized) this.load()
    }
  }

  let lastBuildTimestamp = ''
  let buildCounter = 0
  const createBuildId = () => {
    const date = new Date()
    const timestamp = [
      date.getFullYear(),
      String(date.getMonth() + 1).padStart(2, '0'),
      String(date.getDate()).padStart(2, '0'),
      String(date.getHours()).padStart(2, '0'),
      String(date.getMinutes()).padStart(2, '0'),
      String(date.getSeconds()).padStart(2, '0'),
      String(date.getMilliseconds()).padStart(3, '0'),
    ].join('')
    if (timestamp !== lastBuildTimestamp) {
      lastBuildTimestamp = timestamp
      buildCounter = 0
    } else {
      buildCounter += 1
    }
    return `${timestamp}${buildCounter}`
  }
  const globalData = {
    cliEnv: process.env,
    cliOpts: host.config,
    buildId: createBuildId(),
    init(cliEnv, cliOptions) {
      this.cliEnv = cliEnv
      this.cliOpts = cliOptions
      this.buildId = createBuildId()
    },
    clean() {
      this.buildId = undefined
    },
  }
  let startEnvironment = {
    nodeHome: host.config.nodeHome ?? process.env.NODE_HOME ?? '',
    workspaceDir: host._parameter.getWorkspaceDirInternal(),
  }

  class HvigorExtensionHandlerRegistry {
    add(suffixes) {
      for (const suffix of suffixes) Module._extensions[suffix] = this.handle.bind(this)
    }

    handle(mod, filename) {
      const output = compileHvigorScriptSync(
        path.resolve(filename),
        Boolean(host._parameter.getCoreStartParams().printStackTrace),
      )
      mod._compile(output, filename)
    }
  }
  const extensionHandlerRegistry = new HvigorExtensionHandlerRegistry()
  const addExtensionHandler = (suffixes) => extensionHandlerRegistry.add(suffixes)
  const doCompilePluginTypeCheck = (pluginFile) => host._typeCheckCompilePlugin(pluginFile)
  const TrackAPI = (_target, propertyKey, descriptor) => {
    const original = descriptor.value
    descriptor.value = function (...args) {
      host._usedHvigorApis ||= new Set()
      host._usedHvigorApis.add(propertyKey)
      return original.apply(this, args)
    }
    return descriptor
  }
  const getHvigorConfigValue = (propertyPath) => {
    const projectConfigPath = path.resolve(host.projectRoot, 'hvigor', HVIGOR_DEFAULT_CONFIG_FILE_NAME)
    if (!fs.existsSync(projectConfigPath)) return false
    const projectConfig = Json5Reader.getJson5Obj(projectConfigPath)
    const projectValue = Json5Reader.getJson5ObjProp(projectConfig, propertyPath)
    if (projectConfig && projectValue !== undefined) return projectValue === false
    const userConfigPath = path.resolve(getHvigorUserHome(), HVIGOR_DEFAULT_CONFIG_FILE_NAME)
    if (!fs.existsSync(userConfigPath)) return false
    const userConfig = Json5Reader.getJson5Obj(userConfigPath)
    const userValue = Json5Reader.getJson5ObjProp(userConfig, propertyPath)
    return Boolean(userConfig && userValue !== undefined && userValue === false)
  }

  class HvigorIconv {
    constructor() {
      this.codecDataCache = {}
    }

    canonicalizeEncoding(encoding) {
      return encoding.toLowerCase().replace(/:\d{4}$|[^0-9a-z]/g, '')
    }

    resolveEncoding(encoding) {
      const canonical = this.canonicalizeEncoding(encoding)
      if (canonical === 'utf8' || canonical === 'gbk') return canonical
      throw new Error(`Encoding not recognized: '${encoding}' (searched as: '${canonical}')`)
    }

    decode(buffer, encoding, options) {
      return iconvLite.decode(Buffer.isBuffer(buffer) ? buffer : Buffer.from(buffer), this.resolveEncoding(encoding), options)
    }

    encode(content, encoding, options) {
      return iconvLite.encode(content, this.resolveEncoding(encoding), options)
    }

    encodingExists(encoding) {
      const canonical = this.canonicalizeEncoding(encoding)
      return canonical === 'utf8' || canonical === 'gbk'
    }

    getDecoder(encoding, options) {
      return iconvLite.getDecoder(this.resolveEncoding(encoding), options)
    }

    getEncoder(encoding, options) {
      return iconvLite.getEncoder(this.resolveEncoding(encoding), options)
    }
  }
  const iconv = new HvigorIconv()

  const WithParamReplacement = () => (_target, _propertyKey, descriptor) => {
    const original = descriptor.value
    descriptor.value = function (...args) {
      const value = original.apply(this, args)
      if (!(typeof args[0] === 'string' && args[0].endsWith(HvigorCommonConst.BUILD_PROFILE_FILE))) {
        return value
      }
      return value instanceof Promise
        ? value.then((resolved) =>
            HvigorReplacePlaceHolderUtil.getInstance().replacePlaceholders(resolved, customizedParams()),
          )
        : HvigorReplacePlaceHolderUtil.getInstance().replacePlaceholders(value, customizedParams())
    }
    return descriptor
  }

  class HvigorEventHead {
    constructor(id, name, description, type) {
      this.id = id
      this.name = name
      this.description = description
      this.type = type
    }
  }

  class HvigorEventBody {
    constructor(pid, tid) {
      this.pid = pid
      this.tid = tid
      this.startTime = Number(process.hrtime.bigint())
    }
  }

  class HvigorBaseEvent {
    constructor(head, body) {
      this.head = head
      this.body = body
      this.additional = {}
    }

    setStartTime(time) {
      this.body.startTime = time ?? Number(process.hrtime.bigint())
    }

    setEndTime(time) {
      this.body.endTime = time ?? Number(process.hrtime.bigint())
    }

    setTotalTime(time) {
      this.body.totalTime = time
    }

    getId() {
      return this.head.id
    }

    getName() {
      return this.head.name
    }

    getDescription() {
      return this.head.description
    }

    setName(name) {
      this.head.name = name
    }

    getType() {
      return this.head.type
    }

    setType(type) {
      this.head.type = type
    }

    getTid() {
      return this.body.tid
    }

    setTid(tid) {
      this.body.tid = tid
      return this
    }
  }

  class HvigorLogEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid, logType) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.LOG),
        new HvigorEventBody(pid, tid),
      )
      this.additional = { logType, children: [] }
    }

    getLogType() {
      return this.additional.logType
    }

    setLogType(type) {
      this.additional.logType = type
    }

    getDurationId() {
      return this.additional.durationId
    }

    setDurationId(durationId) {
      this.additional.durationId = durationId
    }

    getContinualId() {
      return this.additional.continualId
    }

    setContinualId(continualId) {
      this.additional.continualId = continualId
    }

    addChild(id) {
      if (id && !this.additional.children.includes(id)) this.additional.children.push(id)
    }

    setParent(id) {
      this.additional.parent ||= id
    }
  }

  class DurationEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, group, tid) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.DURATION),
        new HvigorEventBody(pid, tid),
      )
      const moduleSeparator = name.indexOf(':')
      const targetSeparator = name.indexOf('@')
      this.additional = {
        children: [],
        state: HVIGOR_DURATION_EVENT_STATE.CREATED,
        targetName:
          moduleSeparator > 0 && targetSeparator > moduleSeparator
            ? name.substring(moduleSeparator + 1, targetSeparator)
            : '',
        moduleName: moduleSeparator > 0 ? name.substring(0, moduleSeparator) : '',
        category: group,
        taskRunReasons: [],
      }
      this.log = HvigorLogger.getLogger('DurationEvent')
    }

    start(state = HVIGOR_DURATION_EVENT_STATE.RUNNING, time) {
      this.setState(state)
      this.setStartTime(time)
      return this
    }

    stop(state = HVIGOR_DURATION_EVENT_STATE.SUCCESS, time) {
      if (
        [
          HVIGOR_DURATION_EVENT_STATE.FAILED,
          HVIGOR_DURATION_EVENT_STATE.SUCCESS,
          HVIGOR_DURATION_EVENT_STATE.WARN,
        ].includes(this.additional.state)
      ) {
        return this
      }
      this.body.endTime = time ?? Number(process.hrtime.bigint())
      this.setState(state)
      const service = MetricService.getInstance()
      for (const childId of this.additional.children) {
        const child = service.getEventById(childId)
        if (child instanceof DurationEvent) child.stop(state)
      }
      return this
    }

    setState(state) {
      this.additional.state = state
    }

    createSubEvent(name, description) {
      const event = MetricFactory.createDurationEvent(name, description, '')
      event.setParent(this.getId())
      this.addChild(event.getId())
      return event
    }

    addChild(childId) {
      this.additional.children.push(childId)
    }

    setParent(parentId) {
      this.additional.parent = parentId
    }

    getParent() {
      return this.additional.parent
    }

    getChildren() {
      return this.additional.children
    }

    setLog(name, type = HVIGOR_METRIC_LOG_TYPE.INFO, description, totalTime) {
      const event = MetricFactory.createLogEvent(name ?? this.head.name, type, this.getTid(), description)
      event.setDurationId(this.getId())
      this.additional.logId = event.getId()
      event.setStartTime(this.body.startTime)
      event.setEndTime(this.body.endTime)
      if (totalTime) event.setTotalTime(totalTime)
      this.setParentLog(event)
      this.setChildrenLog(event)
    }

    setParentLog(logEvent) {
      const parent = MetricService.getInstance().getEventById(this.additional.parent)
      if (!(parent instanceof DurationEvent)) return
      const parentLog = MetricService.getInstance().getEventById(parent.additional.logId)
      if (parentLog instanceof HvigorLogEvent) {
        parentLog.addChild(logEvent.getId())
        logEvent.setParent(parentLog.getId())
      }
    }

    setChildrenLog(logEvent) {
      for (const childId of this.additional.children) {
        const child = MetricService.getInstance().getEventById(childId)
        if (!(child instanceof DurationEvent) && !(child instanceof ContinualEvent)) continue
        logEvent.addChild(child.additional.logId)
        const childLog = MetricService.getInstance().getEventById(child.additional.logId)
        if (childLog instanceof HvigorLogEvent) child.setParentLog(childLog)
      }
    }

    setDetail(name) {
      const event = MetricFactory.createLogEvent(name, HVIGOR_METRIC_LOG_TYPE.DETAIL, this.getTid())
      event.setDurationId(this.getId())
      this.additional.detailId = event.getId()
    }

    setCategory(category) {
      this.additional.category = category
    }

    addTaskRunReason(taskRunReason) {
      this.additional.taskRunReasons.push(taskRunReason)
    }
  }

  class ContinualEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid, totalTime, frequency) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.CONTINUAL),
        new HvigorEventBody(pid, tid),
      )
      this.additional = {
        totalTime: totalTime ?? 0,
        frequency: frequency ?? 0,
        children: [],
      }
    }

    setParent(parentId) {
      this.additional.parent = parentId
    }

    getParent() {
      return this.additional.parent
    }

    addChild(childId) {
      this.additional.children.push(childId)
    }

    getChildren() {
      return this.additional.children
    }

    createSubEvent(name, description) {
      const event = MetricFactory.createContinualEvent(name, description)
      event.setParent(this.getId())
      this.addChild(event.getId())
      return event
    }

    setLog(name, type, description) {
      const event = MetricFactory.createLogEvent(name, type, this.getTid(), description)
      event.setContinualId(this.getId())
      this.additional.logId = event.getId()
      event.setStartTime(this.body.startTime)
      event.setEndTime(this.body.endTime)
      this.setParentLog(event)
      this.setChildrenLog(event)
    }

    setParentLog(logEvent) {
      const parent = MetricService.getInstance().getEventById(this.additional.parent)
      if (!(parent instanceof ContinualEvent) && !(parent instanceof DurationEvent)) return
      const parentLog = MetricService.getInstance().getEventById(parent.additional.logId)
      if (parentLog instanceof HvigorLogEvent) {
        parentLog.addChild(logEvent.getId())
        logEvent.setParent(parentLog.getId())
      }
    }

    setDetail(name) {
      const event = MetricFactory.createLogEvent(name, HVIGOR_METRIC_LOG_TYPE.DETAIL, this.getTid())
      event.setContinualId(this.getId())
      this.additional.detailId = event.getId()
    }

    setChildrenLog(logEvent) {
      for (const childId of this.additional.children) {
        const child = MetricService.getInstance().getEventById(childId)
        if (!(child instanceof ContinualEvent)) continue
        logEvent.addChild(child.additional.logId)
        const childLog = MetricService.getInstance().getEventById(child.additional.logId)
        if (childLog instanceof HvigorLogEvent) child.setParentLog(childLog)
      }
    }
  }

  class MarkEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.MARK),
        new HvigorEventBody(pid, tid),
      )
      const now = new Date()
      this.additional = {
        time: {
          year: now.getFullYear(),
          month: now.getMonth() + 1,
          day: now.getDate(),
          hour: now.getHours(),
          minute: now.getMinutes(),
          second: now.getSeconds(),
        },
      }
    }

    start(state = HVIGOR_MARK_EVENT_STATE.RUNNING, time) {
      this.setState(state)
      this.setStartTime(time)
    }

    stop(state = HVIGOR_MARK_EVENT_STATE.SUCCESS, time) {
      if ([HVIGOR_MARK_EVENT_STATE.FAILED, HVIGOR_MARK_EVENT_STATE.SUCCESS].includes(this.additional.state)) {
        return
      }
      this.body.endTime = time ?? Number(process.hrtime.bigint())
      this.setState(state)
    }

    setMarkType(markType) {
      this.additional.markType = markType
    }

    setCategory(category) {
      this.additional.category = category
    }

    setState(state) {
      this.additional.state = state
    }

    setHvigorVersion(hvigorVersion) {
      this.additional.hvigorVersion = hvigorVersion
    }

    setCompleteCommand(completeCommand) {
      this.additional.completeCommand = completeCommand
    }

    setNodeVersion(nodeVersion) {
      this.additional.nodeVersion = nodeVersion
    }
  }

  class HvigorInstantEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.INSTANT),
        new HvigorEventBody(pid, tid),
      )
      this.additional = {}
      this.body.startTime = Number(process.hrtime.bigint())
    }

    setScope(scope) {
      this.additional.scope = scope
    }
  }

  class HvigorCounterEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid, success, failed) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.COUNTER),
        new HvigorEventBody(pid, tid),
      )
      this.additional = { success: success ?? 0, failed: failed ?? 0 }
      this.body.startTime = Number(process.hrtime.bigint())
    }
  }

  class HvigorGaugeEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid, utilization) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.GAUGE),
        new HvigorEventBody(pid, tid),
      )
      this.additional = { utilization }
      this.body.startTime = Number(process.hrtime.bigint())
    }
  }

  class HvigorObjectEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid, objectId, state, snapshot) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.OBJECT),
        new HvigorEventBody(pid, tid),
      )
      this.additional = { objectId, state, snapshot }
      this.body.startTime = Number(process.hrtime.bigint())
    }
  }

  class HvigorMetadataEvent extends HvigorBaseEvent {
    constructor(id, name, description, pid, tid, state) {
      super(
        new HvigorEventHead(id, name, description, HVIGOR_METRIC_EVENT_TYPE.METADATA),
        new HvigorEventBody(pid, tid),
      )
      this.additional = { state }
      this.body.startTime = Number(process.hrtime.bigint())
    }

    setCategory(category) {
      this.additional.category = category
    }

    setSortIndex(sortIndex) {
      this.additional.sortIndex = sortIndex
    }

    setLabel(label) {
      this.additional.label = label
    }

    setContent(content) {
      this.additional.content = content
    }
  }

  class HvigorReport {
    constructor(name, value) {
      this.name = name
      this.value = value
    }

    getName() {
      return this.name
    }

    getValue() {
      return this.value
    }
  }

  class MetricService {
    static instance

    constructor() {
      this.events = new Map()
    }

    submit(event) {
      this.events.set(event.getId(), event)
    }

    getEventById(id) {
      return id ? this.events.get(id) : undefined
    }

    queryReport() {
      let events = this.filterDurationEvent(Array.from(this.events.values()))
      events = this.filterLogEvent(events)
      return new HvigorReport('events', events)
    }

    filterDurationEvent(events) {
      return events.filter(
        (event) =>
          event.getType() !== HVIGOR_METRIC_EVENT_TYPE.DURATION ||
          event.additional.state !== HVIGOR_DURATION_EVENT_STATE.CREATED,
      )
    }

    filterLogEvent(events) {
      return events.filter((event) => {
        if (event.getType() !== HVIGOR_METRIC_EVENT_TYPE.LOG) return true
        const duration = this.getEventById(event.additional.durationId)
        if (duration?.additional.state === HVIGOR_DURATION_EVENT_STATE.CREATED) return false
        return event.additional.logType !== HVIGOR_METRIC_LOG_TYPE.DETAIL || Boolean(duration)
      })
    }

    clear() {
      this.events.clear()
    }

    static getInstance() {
      this.instance ||= new MetricService()
      return this.instance
    }
  }

  class MetricFactory {
    static getUuid() {
      return crypto.randomUUID()
    }

    static submit(event) {
      MetricService.getInstance().submit(event)
      return event
    }

    static createDurationEvent(name, description, group, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(new DurationEvent(this.getUuid(), name, description, process.pid, group, tid))
    }

    static createInstantEvent(name, description, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(new HvigorInstantEvent(this.getUuid(), name, description, process.pid, tid))
    }

    static createCounterEvent(name, description, success, failed, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(
        new HvigorCounterEvent(this.getUuid(), name, description, process.pid, tid, success, failed),
      )
    }

    static createGaugeEvent(name, utilization, description, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(
        new HvigorGaugeEvent(this.getUuid(), name, description, process.pid, tid, utilization),
      )
    }

    static createObjectEvent(name, objectId, state, description, snapshot, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(
        new HvigorObjectEvent(this.getUuid(), name, description, process.pid, tid, objectId, state, snapshot),
      )
    }

    static createMetadataEvent(name, state, description, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(new HvigorMetadataEvent(this.getUuid(), name, description, process.pid, tid, state))
    }

    static createMarkEvent(name, description, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(new MarkEvent(this.getUuid(), name, description, process.pid, tid))
    }

    static createLogEvent(name, type, tid = HVIGOR_MAIN_THREAD, description = '') {
      return this.submit(new HvigorLogEvent(this.getUuid(), name, description, process.pid, tid, type))
    }

    static createContinualEvent(name, description, totalTime, frequency, tid = HVIGOR_MAIN_THREAD) {
      return this.submit(
        new ContinualEvent(this.getUuid(), name, description, process.pid, tid, totalTime, frequency),
      )
    }
  }

  class ReportServiceImpl {
    static instance
    static buildId
    static data = []
    static REPORT_INTERVAL_MS = 1000
    static MB_CONVERTER = 1024

    constructor() {
      this.reportListeners = []
      this.monitorTimeId = undefined
    }

    report() {
      const report = this.getReport()
      const reportDirectory = PathUtil.getReportDirPath()
      fs.mkdirSync(reportDirectory, { recursive: true })
      this.deleteUnusableFiles(reportDirectory)
      this.storage(report, reportDirectory)
    }

    getReport() {
      const report = { version: '2.0', ppid: process.ppid }
      for (const listener of this.reportListeners) {
        const section = listener.queryReport()
        report[section.getName()] = section.getValue()
      }
      return report
    }

    storage(report, reportDirectory) {
      const reports = fs
        .readdirSync(reportDirectory)
        .filter((name) => name.startsWith('report-') && name.endsWith('json'))
        .sort(
          (left, right) =>
            fs.statSync(path.resolve(reportDirectory, right)).birthtimeMs -
            fs.statSync(path.resolve(reportDirectory, left)).birthtimeMs,
        )
      for (const stale of reports.slice(9)) fs.rmSync(path.resolve(reportDirectory, stale), { force: true })
      if (globalData.buildId === undefined) return
      ReportServiceImpl.buildId = globalData.buildId
      const baseName = `report-${globalData.buildId}`
      LocalFileWriter.getInstance().write(path.resolve(reportDirectory, `${baseName}.json`), report)
      if (coreParameter.properties['hvigor.analyzeHtml'] === true) {
        this.generateHtmlResource(reportDirectory, baseName, report)
      }
    }

    deleteUnusableFiles(reportDirectory) {
      for (const name of fs.readdirSync(reportDirectory)) {
        const retained =
          /^report(-monitor)?-[0-9]+\.json$/.test(name) ||
          (/^report-?[0-9]*\.html$/.test(name) && coreParameter.properties['hvigor.analyzeHtml'] !== true) ||
          name === 'htmlResource' ||
          name === 'upload'
        if (!retained) fs.rmSync(path.resolve(reportDirectory, name), { recursive: true, force: true })
      }
    }

    addListener(listener) {
      this.reportListeners.push(listener)
    }

    removeListener(listener) {
      const index = this.reportListeners.indexOf(listener)
      if (index >= 0) this.reportListeners.splice(index, 1)
    }

    generateHtmlResource(reportDirectory, baseName, report) {
      const payload = JSON.stringify(JSON.stringify(report)).replace(/<\//g, '<\\/')
      const html = [
        '<!doctype html>',
        '<html><head><meta charset="utf-8"><title>Hvigor report</title></head>',
        '<body><div id="root"></div>',
        `<script>window.__HVIGOR_REPORT__ = ${payload};<\/script>`,
        '</body></html>',
      ].join('')
      fs.writeFileSync(path.resolve(reportDirectory, `${baseName}.html`), html)
    }

    static getInstance() {
      if (!this.instance) {
        this.instance = new ReportServiceImpl()
        const metricService = MetricService.getInstance()
        this.instance.addListener(metricService)
      }
      return this.instance
    }

    startProcessMonitor() {
      if (this.monitorTimeId) clearInterval(this.monitorTimeId)
      ReportServiceImpl.data = []
      this.monitorTimeId = setInterval(() => {
        const memory = process.memoryUsage()
        ReportServiceImpl.data.push({
          time: Date.now(),
          rss: this.convertToMb(memory.rss),
          heapTotal: this.convertToMb(memory.heapTotal),
          heapUsed: this.convertToMb(memory.heapUsed),
          external: this.convertToMb(memory.external),
          arrayBuffers: this.convertToMb(memory.arrayBuffers),
        })
      }, ReportServiceImpl.REPORT_INTERVAL_MS)
      this.monitorTimeId.unref?.()
    }

    stopProcessMonitor() {
      if (!this.monitorTimeId) return
      clearInterval(this.monitorTimeId)
      this.monitorTimeId = undefined
      if (ReportServiceImpl.buildId === undefined) return
      const reportDirectory = PathUtil.getReportDirPath()
      fs.mkdirSync(reportDirectory, { recursive: true })
      fs.writeFileSync(
        path.join(reportDirectory, `report-monitor-${ReportServiceImpl.buildId}.json`),
        JSON.stringify({ version: '1.0', pid: process.pid, data: ReportServiceImpl.data }, null, 2),
        'utf-8',
      )
    }

    convertToMb(input) {
      return input / ReportServiceImpl.MB_CONVERTER / ReportServiceImpl.MB_CONVERTER
    }
  }

  class TraceManager {
    static instance

    constructor() {
      this.data = {}
      this.callBackList = {}
    }

    trace(key, data, callback) {
      this.data[key] = data
      if (typeof callback === 'function') this.callBackList[key] = callback
    }

    flush() {
      const data = structuredClone(this.data)
      for (const callback of Object.values(this.callBackList)) callback()
      const directory = path.resolve(PathUtil.getHvigorCacheDir(), 'outputs', 'logs', 'details')
      fs.mkdirSync(directory, { recursive: true })
      try {
        fs.writeFileSync(path.resolve(directory, 'details.json'), JSON.stringify(data, hvigorReplacer, 2))
      } catch {}
    }

    static transformKey(key) {
      return key.replace(/\./g, '_')
    }

    static anonymize(value) {
      return hvigorHash(value)
    }

    static trace(key, data, callback) {
      this.instance ||= new TraceManager()
      this.instance.trace(key, data, callback)
    }
  }

  class HvigorSnapshotCacheService {
    constructor(filePath, revive) {
      this.filePath = filePath
      this.revive = revive
      this.cacheEntries = new Map()
    }

    initialize() {
      if (!fs.existsSync(this.filePath)) return
      try {
        const parsed = JSON.parse(fs.readFileSync(this.filePath, 'utf-8'))
        for (const [key, value] of Object.entries(parsed || {})) {
          this.cacheEntries.set(key, this.revive ? this.revive(value) : value)
        }
      } catch {}
    }

    clone(service) {
      this.cacheEntries = new Map(service.cacheEntries)
    }

    close() {
      this.cacheEntries.clear()
    }

    get(key) {
      return this.cacheEntries.get(key)
    }

    set(key, value) {
      this.cacheEntries.set(key, value)
      return value
    }

    remove(key) {
      this.cacheEntries.delete(key)
    }

    size() {
      return this.cacheEntries.size
    }

    keys() {
      return [...this.cacheEntries.keys()]
    }

    getCacheEntryContentMap() {
      return new Map(this.cacheEntries)
    }

    serialize() {
      return JSON.stringify(Object.fromEntries(this.cacheEntries), hvigorReplacer)
    }
  }

  class HvigorValueEntry {
    constructor(name, value) {
      this.name = name
      this.value = value
    }

    getName() {
      return this.name
    }

    getValue() {
      return this.value
    }

    getHash() {
      return hvigorHash(JSON.stringify([this.name, this.value], hvigorReplacer))
    }

    static revive(value) {
      return Object.assign(new HvigorValueEntry(value.name, value.value), value)
    }
  }

  class HvigorTaskSnapshot {
    constructor(task) {
      const node = task.getNode()
      const project = node.getProject()
      this._inputs = []
      this._successful = false
      this._projectName = project.getName()
      this._moduleName = node.getName()
      this._taskName = task.getName()
      this._key = ProjectCacheService.taskKey(this._taskName, node)
      this._executionId = `${this._key}:${Date.now()}`
      for (const [name, value] of task.declareInputs().entries()) this.addInput(name, value)
      this._inputFiles = new Map(task.declareInputFiles().collect())
      this._outputFiles = new Map(task.declareOutputFiles().collect())
      if (task instanceof HvigorIncrementalExecTask) {
        this.addInput('command', task.declareExecutionCommand())
        this.addInput('toolchain', task.declareExecutionTool())
        const environment = Array.from(task.declareExecutionEnv().entries())
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, value]) => `${key}:${value}${path.delimiter}`)
          .join()
        this.addInput('env', environment)
      }
    }

    getKey() {
      return this._key
    }

    compareTo(other) {
      return (
        this._successful &&
        this._projectName === other.getProjectName() &&
        this._moduleName === other.getModuleName() &&
        this._taskName === other.getTaskName() &&
        this.compareInputs(other) &&
        HvigorTaskSnapshot.compareFiles(this._inputFiles, other.getInputFiles()) &&
        HvigorTaskSnapshot.compareFiles(this._outputFiles, other.getOutputFiles())
      )
    }

    updateFilesOptions(cacheService) {
      for (const [filePath, options] of this._inputFiles) {
        options.fileSnapShotHashValue = cacheService.currentFileSnapShotCacheService.get(filePath)?.hashValue
      }
      for (const [filePath, options] of this._outputFiles) {
        options.fileSnapShotHashValue = cacheService.currentFileSnapShotCacheService.get(filePath)?.hashValue
      }
    }

    updateCacheFilesOptions(cacheService) {
      for (const [filePath, options] of this._inputFiles) {
        options.fileSnapShotHashValue = cacheService.fileSnapShotCacheService.get(filePath)?.hashValue
      }
      for (const [filePath, options] of this._outputFiles) {
        options.fileSnapShotHashValue = cacheService.fileSnapShotCacheService.get(filePath)?.hashValue
      }
    }

    getModuleName() {
      return this._moduleName
    }

    getTaskName() {
      return this._taskName
    }

    getProjectName() {
      return this._projectName
    }

    isSuccessful() {
      return this._successful
    }

    setExecutedSuccessful() {
      this._successful = true
    }

    addInput(name, value) {
      const index = this._inputs.findIndex((entry) => entry.getName() === name)
      if (index >= 0) this._inputs.splice(index, 1)
      this._inputs.push(new HvigorValueEntry(name, value))
    }

    getInputs() {
      return this._inputs
    }

    getExecutionId() {
      return this._executionId
    }

    compareInputs(other) {
      const entries = other.getInputs()
      if (entries.length !== this._inputs.length) return false
      const hashEntries = (values) =>
        hvigorHash(
          [...values]
            .sort((left, right) => left.getName().localeCompare(right.getName()))
            .map((entry) => entry.getHash())
            .join(''),
        )
      return hashEntries(this._inputs) === hashEntries(entries)
    }

    getInputFiles() {
      return this._inputFiles
    }

    getOutputFiles() {
      return this._outputFiles
    }

    static compareFiles(left, right) {
      if (left.size !== right.size) return false
      for (const [filePath, options] of left) {
        if (!right.has(filePath)) return false
        if (JSON.stringify(options) !== JSON.stringify(right.get(filePath))) return false
      }
      return true
    }

    static revive(value) {
      const snapshot = Object.create(HvigorTaskSnapshot.prototype)
      Object.assign(snapshot, value)
      snapshot._inputs = (value._inputs || []).map(HvigorValueEntry.revive)
      snapshot._inputFiles = new Map(Object.entries(value._inputFiles || {}))
      snapshot._outputFiles = new Map(Object.entries(value._outputFiles || {}))
      return snapshot
    }
  }

  class HvigorFileSnapshot {
    constructor(filePath, options = {}, depth = 0) {
      this.name = path.basename(filePath)
      this.path = filePath
      this.hashValue = ''
      this.type = 'unknown'
      this.isSymbolicLink = false
      this.children = undefined
      this.fileMetaData = undefined
      this.capture(options, depth)
    }

    capture(options, depth) {
      let stats
      let linkStats
      try {
        stats = fs.statSync(this.path)
        linkStats = fs.lstatSync(this.path)
      } catch {
        return
      }
      this.isSymbolicLink = linkStats.isSymbolicLink()
      if (stats.isFile()) {
        this.type = 'file'
        this.fileMetaData = { size: stats.size, lastModifiedTime: stats.mtime.getTime() }
        this.hashValue =
          options.fileSnapShotHashValue ||
          hvigorHash(`${this.name}${this.fileMetaData.size}${this.fileMetaData.lastModifiedTime}`)
        return
      }
      if (!stats.isDirectory()) return
      this.type = 'directory'
      this.children = []
      if (options.depth !== undefined && depth >= options.depth) return
      let entries = []
      try {
        entries = fs.readdirSync(this.path).sort((left, right) => left.localeCompare(right))
      } catch (error) {
        if (!['EACCES', 'EPERM'].includes(error.code)) throw error
      }
      for (const name of entries) {
        const childPath = path.join(this.path, name)
        if (options.test instanceof RegExp) {
          options.test.lastIndex = 0
          if (!options.test.test(childPath)) continue
        }
        this.children.push(new HvigorFileSnapshot(childPath, options, depth + 1))
      }
      this.hashValue = hvigorHash(this.children.map((child) => `${child.name}${child.hashValue}`).join(''))
    }

    isHashEquals(other) {
      return this.hashValue === other.hashValue
    }

    toJsonString() {
      return JSON.stringify(this)
    }

    static revive(value) {
      const snapshot = Object.create(HvigorFileSnapshot.prototype)
      Object.assign(snapshot, value)
      if (Array.isArray(value.children)) snapshot.children = value.children.map(HvigorFileSnapshot.revive)
      return snapshot
    }
  }

  class ProjectCacheService {
    static instance

    constructor(project) {
      this.projectName = project.getName()
      this.projectNodeDir = project.getNodeDir()
      const cacheDirectory = path.resolve(PathUtil.getHvigorCacheDir(), 'cache')
      this.projectTaskSnapShotCacheFilePath = path.join(cacheDirectory, 'task-cache.json')
      this.projectFileSnapShotCacheFilePath = path.join(cacheDirectory, 'file-cache.json')
      this.projectBuildInfoCacheFilePath = path.join(cacheDirectory, 'last-build-info.json')
      this.metaFilePath = path.join(cacheDirectory, 'meta.json')
      this.taskSnapShotCacheService = new HvigorSnapshotCacheService(
        this.projectTaskSnapShotCacheFilePath,
        HvigorTaskSnapshot.revive,
      )
      this.fileSnapShotCacheService = new HvigorSnapshotCacheService(
        this.projectFileSnapShotCacheFilePath,
        HvigorFileSnapshot.revive,
      )
      this.buildInfoCacheService = new HvigorSnapshotCacheService(this.projectBuildInfoCacheFilePath)
      this.currentTaskSnapShotCacheService = new HvigorSnapshotCacheService(
        this.projectTaskSnapShotCacheFilePath,
      )
      this.currentFileSnapShotCacheService = new HvigorSnapshotCacheService(
        this.projectFileSnapShotCacheFilePath,
      )
    }

    initialize() {
      this.checkMetaInfo()
      this.taskSnapShotCacheService.initialize()
      this.fileSnapShotCacheService.initialize()
      this.currentTaskSnapShotCacheService.clone(this.taskSnapShotCacheService)
      this.currentFileSnapShotCacheService.clone(this.fileSnapShotCacheService)
      this.buildInfoCacheService.initialize()
    }

    getTaskSnapShot(taskName, node) {
      return this.taskSnapShotCacheService.get(ProjectCacheService.taskKey(taskName, node))
    }

    getFileSnapShot(filePath) {
      return this.fileSnapShotCacheService.get(filePath)
    }

    updateFileSnapShot(key, snapshot) {
      this.currentFileSnapShotCacheService.set(key, snapshot)
    }

    updateTaskSnapShot(key, snapshot) {
      this.currentTaskSnapShotCacheService.set(key, snapshot)
    }

    getBuildInfo() {
      return this.buildInfoCacheService.getCacheEntryContentMap()
    }

    close() {
      ProjectCacheService.instance = undefined
      if (!coreParameter.startParams.incrementalExecution) return
      fs.mkdirSync(path.dirname(this.projectTaskSnapShotCacheFilePath), { recursive: true })
      LocalFileWriter.getInstance().writeStr(
        this.projectTaskSnapShotCacheFilePath,
        this.currentTaskSnapShotCacheService.serialize(),
      )
      LocalFileWriter.getInstance().writeStr(
        this.projectFileSnapShotCacheFilePath,
        this.currentFileSnapShotCacheService.serialize(),
      )
      LocalFileWriter.getInstance().writeStr(
        this.projectBuildInfoCacheFilePath,
        this.buildInfoCacheService.serialize(),
      )
    }

    checkMetaInfo() {
      let lastVersion
      try {
        lastVersion = JSON.parse(fs.readFileSync(this.metaFilePath, 'utf-8')).hvigorVersion
      } catch {}
      if (lastVersion !== undefined && lastVersion !== HVIGOR_PARITY_VERSION) {
        fs.rmSync(this.projectTaskSnapShotCacheFilePath, { force: true })
        fs.rmSync(this.projectFileSnapShotCacheFilePath, { force: true })
      }
      fs.mkdirSync(path.dirname(this.metaFilePath), { recursive: true })
      fs.writeFileSync(this.metaFilePath, JSON.stringify({ hvigorVersion: HVIGOR_PARITY_VERSION }, null, 2))
    }

    static getInstance(project) {
      if (!this.instance) {
        this.instance = new ProjectCacheService(project)
        this.instance.initialize()
      }
      return this.instance
    }

    static taskKey(taskName, node) {
      const projectName = node.getProject().getName()
      return node.classKind === 'project'
        ? `:${projectName}:${taskName}`
        : `:${projectName}:${node.getName()}:${taskName}`
    }
  }

  class TaskProxy {
    constructor(task) {
      this.task = task
      this.taskNode = task.getNode()
      this.project = this.taskNode.getProject()
      this.projectCacheService = ProjectCacheService.getInstance(this.project)
      this.cacheTaskSnapShot = this.projectCacheService.getTaskSnapShot(task.getName(), this.taskNode)
      this.curInputFileSnapShotMap = new Map()
      this.curTaskSnapShot = undefined
      this.notDeclareAnyIncrementalLogic = false
      this.useInputOutputCache = Boolean(
        host._parameter.getPropertyInternal('hvigor.incremental.optimization'),
      )
      if (this.useInputOutputCache) {
        this.notDeclareAnyIncrementalLogic =
          task.declareInputs().size === 0 &&
          task.declareInputFiles().collect().size === 0 &&
          task.declareOutputFiles().collect().size === 0
        if (task instanceof HvigorIncrementalExecTask) {
          this.notDeclareAnyIncrementalLogic &&=
            task.declareExecutionTool() === '' &&
            task.declareExecutionCommand() === '' &&
            task.declareExecutionEnv().size === 0
        }
      }
    }

    getFileCompareResult(filePath, options) {
      const snapshot = new HvigorFileSnapshot(filePath, options)
      const cached = this.projectCacheService.getFileSnapShot(filePath)
      return [snapshot, cached ? !cached.isHashEquals(snapshot) : true]
    }

    taskInputValueChanged(inputKey) {
      const cached = this.cacheTaskSnapShot?.getInputs().find((entry) => entry.getName() === inputKey)
      this.curTaskSnapShot ||= new HvigorTaskSnapshot(this.task)
      const current = this.curTaskSnapShot.getInputs().find((entry) => entry.getName() === inputKey)
      return cached?.getValue() !== current?.getValue()
    }

    preExecute() {
      if (this.useInputOutputCache && this.notDeclareAnyIncrementalLogic) return false
      this.curTaskSnapShot = new HvigorTaskSnapshot(this.task)
      if (!this.cacheTaskSnapShot?.isSuccessful()) return false
      for (const [filePath, options] of this.task.declareInputFiles().collect()) {
        if (!fs.existsSync(filePath)) {
          throw new Error(`Failed to find the incremental input file: ${filePath}.`)
        }
        const [snapshot, changed] = this.getFileCompareResult(filePath, options)
        if (changed) {
          this.curInputFileSnapShotMap.set(filePath, snapshot)
          return false
        }
      }
      for (const [filePath, options] of this.task.declareOutputFiles().collect()) {
        if (!fs.existsSync(filePath) || this.getFileCompareResult(filePath, options)[1]) return false
      }
      this.curTaskSnapShot.updateCacheFilesOptions(this.projectCacheService)
      return this.cacheTaskSnapShot.compareTo(this.curTaskSnapShot)
    }

    async execute() {
      if (typeof this.task.execute === 'function') {
        await this.task.execute()
      } else {
        await this.task.getAction().call(this.task)
      }
      this.curTaskSnapShot ||= new HvigorTaskSnapshot(this.task)
      this.curTaskSnapShot.setExecutedSuccessful()
    }

    postExecute() {
      for (const [filePath, options] of this.task.declareInputFiles().collect()) {
        const snapshot =
          this.curInputFileSnapShotMap.get(filePath) || new HvigorFileSnapshot(filePath, options)
        this.projectCacheService.updateFileSnapShot(filePath, snapshot)
      }
      for (const [filePath, options] of this.task.declareOutputFiles().collect()) {
        this.projectCacheService.updateFileSnapShot(filePath, new HvigorFileSnapshot(filePath, options))
      }
      this.curTaskSnapShot ||= new HvigorTaskSnapshot(this.task)
      this.curTaskSnapShot.setExecutedSuccessful()
      this.curTaskSnapShot.updateFilesOptions(this.projectCacheService)
      this.projectCacheService.updateTaskSnapShot(this.curTaskSnapShot.getKey(), this.curTaskSnapShot)
    }

    getCurTaskSnapShot() {
      return this.curTaskSnapShot
    }
  }

  class WorkerPoolDelegator {
    static isActive = false
    static needWarmUp = false

    warmUp(_warmUpPath) {
      if (!WorkerPoolDelegator.needWarmUp) return
      for (let workerId = 0; workerId < host._workerPool.getMinPoolNum(); workerId++) {
        host._workerPool.availableSlot([workerId])
      }
      WorkerPoolDelegator.needWarmUp = false
      WorkerPoolDelegator.isActive = host._workerPool.isActive()
    }

    submit(task, workPath, options) {
      const publicNode = task?.getNode?.()
      const internalNode = publicNode?.__arkdownNode
      const internalTask =
        task?.__arkdownTaskWrapper?.task ||
        task?.task ||
        (internalNode && host._getTaskByName(internalNode, task?.getName?.()))
      if (!internalTask) {
        throw host._error(
          'FAILED_TO_SUBMIT_WORKER_TASK_NOT_FOUND',
          `worker task ${String(task?.getName?.())} is not registered on its Hvigor node`,
        )
      }
      WorkerPoolDelegator.isActive = true
      return host._workerPool.submit(internalTask, workPath, options)
    }

    async terminate() {
      const result = await host._workerPool.shutdown()
      WorkerPoolDelegator.isActive = !result
      return result
    }

    getState() {
      return host._workerPool.getState()
    }

    setState(state) {
      host._workerPool.setState(state)
    }

    setRecycleInterval(recycleInterval) {
      host._workerPool.setRecycleInterval(recycleInterval)
    }

    setMaxIdleTime(maxIdleTime) {
      host._workerPool.setMaxIdleTime(maxIdleTime)
    }

    setErrorCallback(errorCallback) {
      host._workerPool.setErrorCallback(errorCallback)
    }

    getLog(type) {
      return host._workerPool.getLog(type)
    }

    clearLog(type) {
      host._workerPool.clearLog(type)
    }

    getMaxPoolNum() {
      return host._workerPool.getMaxPoolNum()
    }

    getMinPoolNum() {
      return host._workerPool.getMinPoolNum()
    }

    setResident(resident) {
      host._workerPool.setResident(resident)
    }

    isActive() {
      return WorkerPoolDelegator.isActive
    }
  }

  class HvigorWatchWorker {
    constructor() {
      this.workerMap = new Map()
      this.nextWorkerId = 1
    }

    getWorker(id) {
      return this.workerMap.get(id)?.worker
    }

    async getLastWatchResult(id) {
      const item = this.workerMap.get(id)
      if (!item) return true
      if (item.lastWatchResult.finish) return item.lastWatchResult.result
      return new Promise((resolve) => item.resultResolvers.add(resolve))
    }

    sendCompileMsgToWorker() {
      for (const item of this.workerMap.values()) {
        if (item.taskName.includes('HotReload')) item.worker.postMessage({ event: 'HOT_COMPILE' })
      }
    }

    sendNewWatchMsgToProcess(id) {
      this.workerMap.get(id)?.worker.postMessage({ event: 'PAUSE_PRINT_ERR', data: false })
    }

    createWorker(task, filename, hasCustomTerminate, callback, options, customAction) {
      const workerId = this.nextWorkerId++
      const worker = new Worker(filename, options)
      const item = {
        worker,
        taskName: task.getName(),
        hasCustomTerminate,
        keepAlive: false,
        lastWatchResult: { finish: true, result: true },
        resultResolvers: new Set(),
        customActions: new Map(),
      }
      if (customAction) item.customActions.set(customAction.customType, customAction.action)
      this.workerMap.set(workerId, item)
      const completion = new Promise((resolve, reject) => {
        item.resolve = resolve
        item.reject = reject
      })
      task.pendingPromises.add(completion)
      worker.on('message', async (message) => {
        if (message?.event === HVIGOR_CUSTOM_WORKER_REQUEST) {
          const action = item.customActions.get(message.customType)
          if (action) await action(message.param)
          worker.postMessage({ ...message, done: true })
          return
        }
        if (message?.event === HvigorPoolConstant.WORK_DONE) {
          await callback(message.returnVal)
          item.resolve('succeed')
          return
        }
        if (message?.event === HvigorPoolConstant.WORK_ERROR) {
          await callback(message.returnVal)
          const detail = message.error || message.returnVal
          item.reject(
            detail?.warnMsg
              ? new HvigorExtendedErrorInfo(detail.message, { warnMsg: detail.warnMsg })
              : new Error(typeof detail === 'string' ? detail : detail?.message || 'watch worker failed'),
          )
          return
        }
        if (message?.type === HVIGOR_WATCH_EVENT.WATCH_START) {
          item.lastWatchResult.finish = false
        } else if (message?.type === HVIGOR_WATCH_EVENT.WATCH_RESULT) {
          for (const resolve of item.resultResolvers) resolve(message.content)
          item.resultResolvers.clear()
          item.lastWatchResult = { finish: true, result: message.content }
        } else if (message?.type === HVIGOR_WATCH_EVENT.TERMINATE_WORKER) {
          this.terminateWorker(message.content)
        }
        if (Object.values(HVIGOR_WATCH_EVENT).includes(message?.type)) {
          for (const lifecycleCallback of host._watchWorkerMessageCallbacks) {
            await lifecycleCallback(message)
          }
        }
      })
      worker.on('error', (error) => item.reject(error))
      this.sendNewWatchMsgToProcess(workerId)
      return String(workerId)
    }

    terminate(id) {
      const item = this.workerMap.get(id)
      if (item?.hasCustomTerminate) {
        item.worker.postMessage({ type: HVIGOR_WATCH_EVENT.TERMINATE_WORKER, content: id })
      } else {
        this.terminateWorker(id)
      }
    }

    terminateWorker(id) {
      this.workerMap.get(id)?.worker.terminate()
      this.workerMap.delete(id)
    }
  }

  class HvigorNormalWorker {
    constructor() {
      this.maxWorkerNum = HvigorPoolConstant.MAX_POOL_NUM > 1 ? HvigorPoolConstant.MAX_POOL_NUM - 1 : 1
      this.runningThreadCount = 0
      this.queue = []
      this.nextWorkerId = 1
    }

    createWorker(task, filename, _hasCustomTerminate, callback, options) {
      const workerId = this.nextWorkerId++
      const completion = new Promise((resolve, reject) => {
        this.queue.push({ workerId, task, filename, callback, options, resolve, reject })
        this.processNextTask()
      })
      task.pendingPromises.add(completion)
      return String(workerId)
    }

    reset() {
      this.runningThreadCount = 0
      this.queue.length = 0
    }

    processNextTask() {
      while (this.runningThreadCount < this.maxWorkerNum && this.queue.length > 0) {
        this.executeWorker(this.queue.shift())
      }
    }

    executeWorker(work) {
      const worker = new Worker(work.filename, work.options)
      this.runningThreadCount += 1
      const finish = () => {
        this.runningThreadCount -= 1
        this.processNextTask()
      }
      worker.on('message', async (message) => {
        try {
          if (message?.event === HvigorPoolConstant.WORK_DONE) {
            await work.callback(message.returnVal)
            await worker.terminate()
            finish()
            work.resolve(true)
          } else if (message?.event === HvigorPoolConstant.WORK_ERROR) {
            await work.callback(message.returnVal)
            finish()
            work.reject(new Error(message.error?.message || message.returnVal?.message || 'normal worker failed'))
          }
        } catch (error) {
          finish()
          work.reject(error)
        }
      })
      worker.on('error', (error) => {
        finish()
        work.reject(error)
      })
    }
  }

  class Tasks extends HvigorDefaultTask {
    constructor(node) {
      super(node, {
        name: 'tasks',
        group: HVIGOR_TASK_GROUP_TYPE.HELP_TASK_GROUP,
        isEnabled: true,
        sourcePluginId: 'hvigor',
        description: 'Displays the tasks of the HvigorCoreNode.',
      })
    }

    registryAction() {
      return () => {
        const lines = [
          '--------------------------------------------------------------------------',
          `All tasks from hvigor node '${this.node.getName()}'`,
          '--------------------------------------------------------------------------',
          ...this.transformInfos(this.node.getAllTasks()),
        ]
        HvigorLogger.getLogger('no-pattern-info').info(lines.join(os.EOL))
      }
    }

    transformInfos(tasks) {
      const groups = new Map()
      for (const task of tasks) {
        const group = task.getGroup()
        if (!groups.has(group)) groups.set(group, [])
        groups.get(group).push(task)
      }
      const lines = []
      for (const [group, groupTasks] of groups) {
        lines.push(`${group} tasks`, '-------------')
        for (const task of groupTasks) {
          lines.push(task.getDescription() ? `${task.getName()} - ${task.getDescription()}` : task.getName())
        }
        lines.push('')
      }
      return lines
    }
  }

  const projectTaskDag = new HvigorTaskDirectedAcyclicGraph()
  const watchWorker = new HvigorWatchWorker()
  const normalWorker = new HvigorNormalWorker()
  const readJavaDaemonInfoFromCacheFile = () => {
    const filePath = path.resolve(getHvigorUserHome(), 'daemon', 'cache', 'java-daemon.json')
    if (!fs.existsSync(filePath)) return undefined
    try {
      return JSON.parse(fs.readFileSync(filePath, 'utf-8'))
    } catch {
      return undefined
    }
  }

  host._metricCompatibility = { MetricFactory, MetricService }

  return {
    ParseJsonFile,
    parseJsonFile,
    Json5Reader,
    FileUtil,
    PathUtil,
    WithParamReplacement,
    GlobalParam,
    HvigorConfigLoader,
    HvigorLogger,
    LocalFileWriter,
    LogLevelMap,
    CacheStore,
    CacheStoreManager,
    ContinualEvent,
    DurationEvent,
    ExtendedErrorInfo: HvigorExtendedErrorInfo,
    HvigorSystemPlugin,
    MarkEvent,
    MarkEventType: HVIGOR_MARK_EVENT_TYPE,
    MetricEventType: HVIGOR_METRIC_EVENT_TYPE,
    MetricFactory,
    MetricLogType: HVIGOR_METRIC_LOG_TYPE,
    MetricService,
    PoolConstant: HvigorPoolConstant,
    ProjectCacheService,
    ReportServiceImpl,
    TaskDirectedAcyclicGraph: HvigorTaskDirectedAcyclicGraph,
    TaskProxy,
    Tasks,
    TcbStore: HvigorTcbStore,
    TraceManager,
    WatchEvent: HVIGOR_WATCH_EVENT,
    WorkerEvent: HVIGOR_WORKER_EVENT,
    WorkerPoolDelegator,
    normalWorker,
    projectTaskDag,
    readJavaDaemonInfoFromCacheFile,
    watchWorker,
    TrackAPI,
    addExtensionHandler,
    addLayout: log4js.addLayout,
    configure: log4js.configure,
    connectLogger: log4js.connectLogger,
    coreParameter,
    doCompilePluginTypeCheck,
    getConfiguration,
    getHvigorConfigValue,
    getLogger: log4js.getLogger,
    globalData,
    iconv,
    isConfigured: log4js.isConfigured,
    levelMap,
    levels: log4js.levels,
    recording: log4js.recording,
    setCategoriesLevel,
    shutdown: log4js.shutdown,
    get startEnvironment() {
      return startEnvironment
    },
    set startEnvironment(value) {
      startEnvironment = value
    },
  }
}

// Hooks routed into Rolldown. The two latest-hvigor additions that Rolldown
// does not own are handled by a thin internal adapter below this interface.
const ROLLUP_HOOKS = new Set([
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'buildEnd',
  'resolveDynamicImport',
])

const hvigorDependencyContexts = []
let hvigorResolvePatchInstalled = false
let hvigorTypeScriptExtensionInstalled = false

function createBuild(nativeBuild, inspectHvigorContext) {
  return async function build(config = {}, plugins) {
    const runtime = new RuntimePluginLoader(config, inspectHvigorContext)
    const loadedPlugins = await runtime.load()
    const explicitPlugins = normalizePluginList(plugins, runtime.projectRoot)
    const allPlugins = [...loadedPlugins, ...explicitPlugins]
    await runtime.hvigorHost.runTaskGraphResolved()
    runtime.hvigorHost.typeCheckBuildScripts()
    runtime.hvigorHost.prepareDirectBuildCycle()
    try {
      // Rust reports the owning hvigor task boundary immediately before and
      // after each native action. This keeps TypeScript task dependencies and
      // beforeRun/afterRun hooks in the real stage instead of flattening them
      // into a synthetic pre-build hook.
      let result
      try {
        result = await nativeBuild(
          runtime.toNativeBuildConfig(),
          allPlugins,
          runtime.hvigorHost.createNativeTaskBridge(),
        )
      } finally {
        runtime.hvigorHost.clearNativeTaskSelection()
      }
      await runtime.hvigorHost.writeBuildMeta()
      await runtime.hvigorHost.runBuildFinished(null, result)
      return result
    } catch (error) {
      await runtime.hvigorHost.runBuildFinished(error)
      throw error
    }
  }
}

/**
 * Hvigor-compatible command entry point.
 *
 * The latest CLI stores raw command task names before evaluating
 * hvigorconfig/hvigorfile, resolves the complete lazy graph afterwards, and
 * only then starts task execution. Keep the same ordering here so plugins see
 * `hvigor.getCommandEntryTask()` during configuration and can still mutate the
 * selected tasks in `taskGraphResolved`.
 */
function createHvigorCommand(nativeBuild, inspectHvigorContext, nativeRuntime = {}) {
  return async function runHvigorCommand(config = {}, plugins) {
    const invocation = new HvigorCommandInvocation(
      nativeBuild,
      inspectHvigorContext,
      config,
      plugins,
      nativeRuntime,
    )
    return invocation.run()
  }
}

class HvigorCommandInvocation {
  constructor(nativeBuild, inspectHvigorContext, config, plugins, nativeRuntime = {}) {
    this.nativeBuild = nativeBuild
    this.nativeRuntime = nativeRuntime
    this.runtime = new RuntimePluginLoader(config, inspectHvigorContext)
    this.explicitPlugins = plugins
  }

  async run() {
    const startedAt = Date.now()
    let report
    try {
      const loadedPlugins = await this.runtime.load()
      const plugins = [...loadedPlugins, ...normalizePluginList(this.explicitPlugins, this.runtime.projectRoot)]
      const host = this.runtime.hvigorHost
      await host.runTaskGraphResolved()
      host.typeCheckBuildScripts()
      const plan = host.commandPlan()
      report = await this.execute(plan, plugins)
      const result = {
        artifacts: report.artifacts,
        elapsedMs: Date.now() - startedAt,
        output: report.output,
      }
      if (this.runtime.config.sync) {
        await host.writeSyncInfo()
      }
      await host.writeBuildMeta()
      await host.runBuildFinished(null, result)
      return result
    } catch (error) {
      await this.runtime.hvigorHost.runBuildFinished(error, report)
      throw error
    }
  }

  async execute(plan, plugins) {
    const artifacts = []
    const output = []
    const nativeTasks = plan.tasks.filter((entry) => ['assembly', 'native'].includes(entry.commandKind))
    const syncCustomRoots = plan.tasks
      .filter((entry) => !entry.task.builtIn && entry.task.definition.__arkdownSyncTask)
      .map((entry) => entry.task)
    const customRoots = plan.tasks
      .filter((entry) => !entry.task.builtIn && !entry.task.definition.__arkdownSyncTask)
      .map((entry) => entry.task)
    const standaloneTasks = plan.tasks.filter(
      (entry) => !['assembly', 'native'].includes(entry.commandKind) && entry.task.builtIn,
    )

    for (const entry of standaloneTasks) {
      const taskOutput = await this.runtime.hvigorHost.executeCommandTask(
        entry,
        this.nativeRuntime.runRuntimeTasks,
      )
      output.push(...taskOutput)
    }
    await this.runtime.hvigorHost.executeCustomCommandTasks(
      syncCustomRoots,
      this.nativeRuntime.runRuntimeTasks,
    )

    if (nativeTasks.length === 0) {
      await this.runtime.hvigorHost.executeCustomCommandTasks(
        customRoots,
        this.nativeRuntime.runRuntimeTasks,
      )
      return { artifacts, output }
    }

    for (const request of plan.nativeRequests) {
      const nativeConfig = this.runtime.toNativeBuildConfig()
      nativeConfig.target = request.outputTarget ?? request.target
      nativeConfig.module = request.module
      nativeConfig.buildTargetName = request.targetName
      if (request.cutoffTaskRoots.length > 0) {
        nativeConfig.nativeTaskRootsJson = JSON.stringify(request.cutoffTaskRoots)
      }
      // DevEco Studio 6.1.1's cli.js accepts --watch, but 6.24.3 does not
      // route it into ordinary assemble task configuration. The command runs
      // once and exits; Arkdown's direct build({ watch: true }) API remains a
      // separate extension and still owns the persistent Rust watcher.
      nativeConfig.watch = false
      this.runtime.hvigorHost.prepareNativeCommandCycle(
        request.nodeNames,
        customRoots,
        request.rootTasks,
      )
      let result
      try {
        result = await this.nativeBuild(
          nativeConfig,
          plugins,
          this.runtime.hvigorHost.createNativeTaskBridge(),
        )
      } finally {
        this.runtime.hvigorHost.clearNativeTaskSelection()
      }
      artifacts.push(...toArray(result?.artifacts))
    }
    return { artifacts: [...new Set(artifacts)], output }
  }
}

function toNativeBuildConfig(config, overlay) {
  return nativeRuntimeBinding.prepareNativeBuildConfig(config || {}, overlay, process.execPath)
}

class RuntimePluginLoader {
  constructor(config, inspectHvigorContext) {
    this.config = config || {}
    this.projectRoot = path.resolve(this.config.project || process.cwd())
    this.hvigorHost = new HvigorRuntimeHost(this.projectRoot, this.config, inspectHvigorContext)
    this.projectRoot = this.hvigorHost.projectRoot
    this.hvigorWorkspaceNodeModules = null
    this.hvigorWorkspacePrepared = false
    this.seen = new Set()
    this.projectHvigorOverrides = {}
    this.moduleHvigorOverrides = new Map()
  }

  async load() {
    const plugins = []
    await this.prepareHvigorDependencyWorkspace()
    await this.hvigorHost.evaluateHvigorConfig()
    for (const plugin of await this.loadConfiguredPluginPaths()) {
      plugins.push(plugin)
    }
    for (const plugin of await this.loadHvigorfiles()) {
      plugins.push(plugin)
    }
    this.hvigorHost.prepareOhpmDependencyWorkspace()
    for (const plugin of await this.loadBuildProfileCompilePlugins()) {
      plugins.push(plugin)
    }
    return new HvigorCompilePluginPlanner(this.config).apply(plugins)
  }

  async prepareHvigorDependencyWorkspace() {
    if (this.hvigorWorkspacePrepared) return
    const request = this.hvigorHost.hvigorPluginWorkspaceRequest()
    if (request === null) {
      this.hvigorWorkspacePrepared = true
      return
    }
    const response = JSON.parse(
      await nativeRuntimeBinding.prepareHvigorPluginWorkspace(JSON.stringify(request)),
    )
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this.hvigorHost._error(code, message, details)
    }
    this.hvigorWorkspaceNodeModules = configureHvigorNodePath(
      this.projectRoot,
      request.projectConfig,
      response.workspace,
      response.nodeModules,
    )
    this.hvigorHost.hvigorWorkspaceNodeModules = this.hvigorWorkspaceNodeModules
    this.hvigorWorkspacePrepared = true
  }

  toNativeBuildConfig() {
    this.hvigorHost.assertSupportedBuildExtensions()
    const startParams = this.hvigorHost.startParameters()
    const runtimeProperties = this.hvigorHost.runtimeProperties(startParams)
    const projectHvigorOverridesJson =
      Object.keys(this.projectHvigorOverrides).length > 0
        ? this.serializeOverrides(
            this.projectHvigorOverrides,
            'project hvigor overrides',
          )
        : undefined
    const moduleHvigorOverridesJson =
      this.moduleHvigorOverrides.size > 0
        ? this.serializeOverrides(
            Object.fromEntries(this.moduleHvigorOverrides),
            'module hvigor overrides',
          )
        : undefined
    const runtimeContextOverrides = this.hvigorHost.runtimeContextOverrides()
    const alignDeviceTypes = this.hvigorHost.alignDeviceTypes()
    return toNativeBuildConfig(this.config, {
      defaultCache: startParams.incrementalExecution,
      mode: this.hvigorHost._effectiveBuildMode(),
      enableSignTask: this.hvigorHost.enableSignTask(),
      fastBuildApp: this.hvigorHost.fastBuildApp(),
      ohosDebugAsan: runtimeProperties.asanConfig,
      ohosEnableHwasan: runtimeProperties.hwasanConfig,
      ohosEnableTsan: runtimeProperties.tsanConfig,
      ohosEnableUbsan: runtimeProperties.ubsanConfig,
      ohosEnableBinxo: runtimeProperties.binxoConfig,
      projectHvigorOverridesJson,
      moduleHvigorOverridesJson,
      runtimeContextOverridesJson:
        runtimeContextOverrides === undefined
          ? undefined
          : this.serializeOverrides(runtimeContextOverrides, 'runtime context overrides'),
      packCompressLevel: this.hvigorHost.packCompressLevel(),
      alignDeviceTypes: alignDeviceTypes.length > 0 ? alignDeviceTypes : undefined,
      collectDebugSymbol: this.hvigorHost.collectDebugSymbol(),
      uiTransformOptimization: this.hvigorHost.uiTransformOptimization(),
      integratedHspUseGetCurrentBundleName: this.hvigorHost.integratedHspUseGetCurrentBundleName(),
      byteCodeHarIntegratedOptimization: this.hvigorHost.byteCodeHarIntegratedOptimization(),
      keepDependency: this.hvigorHost.keepDependency(),
      parameterFileOverride: this.hvigorHost.parameterFileOverride(),
      hvigorRuntimePropertiesJson: this.serializeOverrides(
        runtimeProperties,
        'hvigor runtime properties',
      ),
      hvigorCacheDir: this.hvigorHost.hvigorCacheDir(),
    })
  }

  serializeOverrides(value, label) {
    try {
      return JSON.stringify(value)
    } catch (error) {
      throw new TypeError(`failed to serialize ${label}: ${error.message}`)
    }
  }

  async loadConfiguredPluginPaths() {
    const plugins = []
    for (const key of ['hvigorPluginFile', 'compilePluginPath']) {
      for (const candidate of toArray(this.config[key])) {
        for (const plugin of await this.loadPluginPath(candidate, this.projectRoot, { requireDefault: true })) {
          plugins.push(plugin)
        }
      }
    }
    return plugins
  }

  async loadHvigorfiles() {
    const plugins = []
    const records = []
    const files = this.hvigorHost.discoverBuildScriptFiles(this.discoverHvigorRoots())
    this.hvigorHost.prepareHvigorfiles(files)
    await this.hvigorHost.runNodesInitialized()
    for (const file of files) {
      records.push(await this.loadHvigorfile(file))
    }
    await this.hvigorHost.runNodesEvaluated()
    for (const record of records) {
      for (const plugin of await this.collectHvigorfileEffects(record)) {
        plugins.push(plugin)
      }
    }
    return plugins
  }

  discoverHvigorRoots() {
    return [this.projectRoot, ...this.hvigorHost.moduleRoots()]
  }

  async loadHvigorfile(file) {
    // Hvigor owns one node/lifecycle host for the complete configuration
    // phase. Project and module hvigorfiles must therefore share the host.
    const host = this.hvigorHost
    await host.beginHvigorfile(file)
    const exports = await loadModule(file, host)
    const value = unwrapDefault(exports)
    host.bindHvigorfile(file, value)
    await host.applyCustomPlugins(file, value && value.plugins)
    await host.runAfterNodeEvaluate(file)
    return { file, value }
  }

  async collectHvigorfileEffects({ file, value }) {
    const host = this.hvigorHost
    const plugins = []
    const baseDir = path.dirname(file)
    const moduleRoot = path.resolve(baseDir) === this.projectRoot ? undefined : path.resolve(baseDir)
    const effects = host.effectsFor(file)
    const capturedBuildOption = this.captureHvigorOverrides(baseDir, value, effects, file)

    // A hvigorfile `plugins` array contains HvigorPlugin objects with
    // `pluginId/apply`, not Rolldown hooks. They were applied above after the
    // system plugin was bound, matching hvigor 6.24.3 configuration().
    for (const plugin of normalizePluginList(value, baseDir)) {
      plugins.push(plugin)
    }

    // Rust scans the stable hvigor option paths and applies extension
    // precedence. JavaScript only executes the selected plugin modules.
    const exportedPluginPaths = this.pluginPathPlan(value, baseDir)
    for (const candidate of exportedPluginPaths.hvigorPluginFiles) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }
    for (const candidate of exportedPluginPaths.compilePluginFiles) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }
    // Programmatic task-factory and module-context mutations are collected
    // from the same merged object that is forwarded to Rust. The latest IDE
    // schemas reject a declarative `compilePluginFile`; that case is diagnosed
    // before this compatibility path is reached.
    const capturedPluginPaths = this.pluginPathPlan(capturedBuildOption, baseDir, false)
    for (const candidate of capturedPluginPaths.compilePluginFiles) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }

    // Collect plugin paths captured from task-factory calls (HvigorRuntimeHost effects).
    // This handles `hapTasks({ buildOption: { arkOptions: { compilePluginPath: '...' } } })`.
    for (const candidate of effects.hvigorPluginFile) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }
    for (const candidate of effects.compilePluginPath) {
      for (const plugin of await this.loadPluginPath(candidate, baseDir, { requireDefault: true, moduleRoot })) {
        plugins.push(plugin)
      }
    }

    return plugins
  }

  async loadBuildProfileCompilePlugins() {
    const plugins = []
    for (const record of this.hvigorHost.compilePluginRecords()) {
      for (const plugin of await this.loadPluginPath(record.file, record.moduleRoot, {
        requireDefault: true,
        moduleRoot: record.moduleRoot,
        targetName: record.targetName,
      })) {
        plugins.push(plugin)
      }
    }
    return plugins
  }

  pluginPathPlan(value, baseDir, includeCompilePluginFile = true) {
    return JSON.parse(
      nativeRuntimeBinding.collectBuildScriptPluginPaths(
        value,
        baseDir,
        includeCompilePluginFile,
      ),
    )
  }

  captureHvigorOverrides(root, value, effects, file = path.join(root, 'hvigorfile.ts')) {
    const ohos = value && typeof value === 'object' ? value.config?.ohos : undefined
    const overrides = ohos && typeof ohos === 'object' ? ohos.overrides : undefined
    const normalizedRoot = path.resolve(root)

    if (normalizedRoot === this.projectRoot) {
      if (overrides && typeof overrides === 'object') {
        if (overrides.appOpt !== undefined) {
          this.projectHvigorOverrides.appOpt = mergeHvigorConfig(this.projectHvigorOverrides.appOpt, overrides.appOpt)
        }
        if (overrides.signingConfig !== undefined) {
          this.projectHvigorOverrides.signingConfig = mergeHvigorConfig(
            this.projectHvigorOverrides.signingConfig,
            overrides.signingConfig,
          )
        }
      }
      this.hvigorHost.recordProjectHvigorOverrides(this.projectHvigorOverrides)
      return undefined
    }

    const exportedBuildOption = overrides && typeof overrides === 'object' ? overrides.buildOption : undefined
    this.hvigorHost.validateBuildScriptBuildOption(
      exportedBuildOption,
      file,
      'config.ohos.overrides.buildOption',
    )
    // `effects.buildOption` retains compatibility with task-factory calls
    // used by older projects. Declarative `config.ohos.overrides.buildOption`
    // is applied last, matching hvigor's module config manager.
    const capturedBuildOption = effects.buildOption
    const buildOption = mergeHvigorConfig(
      capturedBuildOption && Object.keys(capturedBuildOption).length > 0 ? capturedBuildOption : undefined,
      exportedBuildOption,
    )
    if (buildOption !== undefined) {
      this.moduleHvigorOverrides.set(
        normalizedRoot,
        mergeHvigorConfig(this.moduleHvigorOverrides.get(normalizedRoot), buildOption),
      )
      this.hvigorHost.recordModuleHvigorOverrides(normalizedRoot, this.moduleHvigorOverrides.get(normalizedRoot))
    }
    return buildOption
  }

  async loadPluginPath(candidate, baseDir = this.projectRoot, options = {}) {
    if (!candidate || typeof candidate !== 'string') return []
    const file = nativeRuntimeBinding.resolveBuildScriptPluginPath(candidate, baseDir)
    const moduleRoot = options.moduleRoot ? path.resolve(options.moduleRoot) : undefined
    const targetName = typeof options.targetName === 'string' ? options.targetName : undefined
    const identity = `${file || ''}\0${moduleRoot || '*'}\0${targetName || '*'}`
    if (!file || this.seen.has(identity)) {
      return []
    }
    this.seen.add(identity)
    const exports = await loadModule(file)
    const value = options.requireDefault ? requireDefaultExport(exports, file) : unwrapDefault(exports)
    return normalizeSortedPluginList(value, path.dirname(file), file, moduleRoot, targetName)
  }
}

// ---------------------------------------------------------------------------
// HvigorRuntimeHost
// ---------------------------------------------------------------------------

class HvigorNormalizedFile {
  constructor(filePath) {
    this.filePath = path.resolve(filePath)
    Object.freeze(this)
  }

  asFileList() {
    if (!fs.existsSync(this.filePath)) {
      return []
    }
    const files = [this]
    if (!fs.statSync(this.filePath).isDirectory()) {
      return files
    }
    const pending = [this.filePath]
    while (pending.length > 0) {
      const directory = pending.pop()
      const entries = fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))
      for (const entry of entries) {
        const candidate = path.join(directory, entry.name)
        files.push(new HvigorNormalizedFile(candidate))
        if (entry.isDirectory()) {
          pending.push(candidate)
        }
      }
    }
    return files
  }

  file(relativePath) {
    const candidate = path.resolve(this.filePath, relativePath)
    return fs.existsSync(candidate) ? new HvigorNormalizedFile(candidate) : undefined
  }

  getPath() {
    return this.filePath
  }
}

class HvigorTaskInput {
  constructor() {
    this.declarations = { files: [], properties: new Map() }
  }

  file(filePath) {
    this.declarations.files.push(filePath)
    return this
  }

  files(filePaths) {
    this.declarations.files.push(...filePaths)
    return this
  }

  property(key, value) {
    this.declarations.properties.set(key, value)
    return this
  }
}

class HvigorTaskOutput {
  constructor() {
    this.declarations = { files: [] }
  }

  file(filePath) {
    this.declarations.files.push(filePath)
    return this
  }

  files(filePaths) {
    this.declarations.files.push(...filePaths)
    return this
  }
}

/** Exact public FileSet shape from latest hvigor 6.24.3. */
class HvigorFileSet {
  constructor() {
    this._group = new Map()
  }

  addEntry(filePath, options) {
    this._group.set(filePath, options || {})
    return this
  }

  deleteEntry(filePath) {
    if (this._group.has(filePath)) {
      this._group.delete(filePath)
    }
    return this
  }

  addEntries(entries, options) {
    for (const filePath of entries) {
      this._group.set(filePath, { ...options })
    }
    return this
  }

  collect() {
    return this._group
  }
}

const HVIGOR_INCREMENTAL_DECLARATIONS = Symbol('HVIGOR_INCREMENTAL_DECLARATIONS')
const HVIGOR_INCREMENTAL_DECORATOR_TYPE = Object.freeze({
  Input: 'Input',
  Inputs: 'Inputs',
  InputFile: 'InputFile',
  InputFiles: 'InputFiles',
  OutputFile: 'OutputFile',
  OutputFiles: 'OutputFiles',
  IgnoreSuperIncremental: 'IgnoreSuperIncremental',
})

function makeHvigorIncrementalDecorator(declaratorType, options) {
  return (target, propertyKey) => {
    if (!Object.prototype.hasOwnProperty.call(target, HVIGOR_INCREMENTAL_DECLARATIONS)) {
      Object.defineProperty(target, HVIGOR_INCREMENTAL_DECLARATIONS, {
        value: [],
        configurable: false,
        enumerable: false,
      })
    }
    target[HVIGOR_INCREMENTAL_DECLARATIONS].push({ propertyKey, declaratorType, options })
  }
}

const HvigorInput = () => makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.Input)
const HvigorInputs = () => makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.Inputs)
const HvigorInputFile = (options) =>
  makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.InputFile, options)
const HvigorInputFiles = (options) =>
  makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.InputFiles, options)
const HvigorOutputFile = (options) =>
  makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.OutputFile, options)
const HvigorOutputFiles = (options) =>
  makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.OutputFiles, options)
const HvigorIgnoreSuperIncremental = () =>
  makeHvigorIncrementalDecorator(HVIGOR_INCREMENTAL_DECORATOR_TYPE.IgnoreSuperIncremental)

function copyHvigorFileSet(source) {
  const output = new HvigorFileSet()
  if (!(source instanceof HvigorFileSet) && typeof source?.collect !== 'function') {
    return output
  }
  for (const [filePath, options] of source.collect()) {
    output.addEntry(filePath, options ? { ...options } : undefined)
  }
  return output
}

function hvigorIncrementalInputValue(value) {
  const primitiveTypes = new Set(['number', 'string', 'boolean'])
  const normalized = value ?? ''
  if (
    primitiveTypes.has(typeof normalized) ||
    (Array.isArray(normalized) && normalized.every((item) => primitiveTypes.has(typeof item)))
  ) {
    return { accepted: true, value: normalized }
  }
  try {
    return {
      accepted: true,
      value: crypto.createHash('sha256').update(JSON.stringify(normalized)).digest('hex'),
    }
  } catch {
    return { accepted: false, value: undefined }
  }
}

function collectHvigorIncrementalDeclarations(task) {
  const declaredInputs = task.declareInputs()
  const inputs = declaredInputs instanceof Map ? new Map(declaredInputs) : new Map()
  const inputFiles = copyHvigorFileSet(task.declareInputFiles())
  const outputFiles = copyHvigorFileSet(task.declareOutputFiles())
  const seenProperties = new Set()
  const ignoredTypes = new Set()
  let owner = task

  while (owner instanceof HvigorIncrementalTask) {
    const declarations = Object.prototype.hasOwnProperty.call(owner, HVIGOR_INCREMENTAL_DECLARATIONS)
      ? owner[HVIGOR_INCREMENTAL_DECLARATIONS]
      : []
    for (const declaration of declarations) {
      const { propertyKey, declaratorType, options } = declaration
      if (ignoredTypes.has(declaratorType) || seenProperties.has(propertyKey)) {
        continue
      }
      seenProperties.add(propertyKey)
      const value = task[propertyKey]
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.IgnoreSuperIncremental) {
        const ignored = {
          declareInputs: [
            HVIGOR_INCREMENTAL_DECORATOR_TYPE.Input,
            HVIGOR_INCREMENTAL_DECORATOR_TYPE.Inputs,
          ],
          declareInputFiles: [
            HVIGOR_INCREMENTAL_DECORATOR_TYPE.InputFile,
            HVIGOR_INCREMENTAL_DECORATOR_TYPE.InputFiles,
          ],
          declareOutputFiles: [
            HVIGOR_INCREMENTAL_DECORATOR_TYPE.OutputFile,
            HVIGOR_INCREMENTAL_DECORATOR_TYPE.OutputFiles,
          ],
        }[propertyKey]
        if (ignored) {
          for (const type of ignored) ignoredTypes.add(type)
        }
        continue
      }
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.Input) {
        const inputValue = hvigorIncrementalInputValue(value)
        if (inputValue.accepted) inputs.set(propertyKey, inputValue.value)
        continue
      }
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.Inputs) {
        if (value instanceof Map) {
          for (const [key, item] of value) {
            const inputValue = hvigorIncrementalInputValue(item)
            if (inputValue.accepted) inputs.set(`${propertyKey}-${key}`, inputValue.value)
          }
        }
        continue
      }
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.InputFile) {
        if (typeof value === 'string' && value !== '' && fs.existsSync(value)) {
          inputFiles.addEntry(value, options ? { ...options } : undefined)
        }
        continue
      }
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.InputFiles) {
        if (Array.isArray(value)) {
          for (const filePath of value) {
            if (typeof filePath === 'string' && filePath !== '' && fs.existsSync(filePath)) {
              inputFiles.addEntry(filePath, options ? { ...options } : undefined)
            }
          }
        }
        continue
      }
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.OutputFile) {
        if (typeof value === 'string' && value !== '') {
          outputFiles.addEntry(value, options ? { ...options } : undefined)
        }
        continue
      }
      if (declaratorType === HVIGOR_INCREMENTAL_DECORATOR_TYPE.OutputFiles && Array.isArray(value)) {
        for (const filePath of value) {
          if (typeof filePath === 'string' && filePath !== '') {
            outputFiles.addEntry(filePath, options ? { ...options } : undefined)
          }
        }
      }
    }
    owner = Object.getPrototypeOf(owner)
  }

  return { inputs, inputFiles, outputFiles }
}

/**
 * Lazy built-in task catalog observed from DevEco Studio 6.1.1 / hvigor
 * 6.24.3. `LazyTaskContainer.findTask()` realizes one task on demand while
 * `getAllTasks()` returns only tasks that have already been realized.
 *
 * Rust owns the observed 6.24.3 creation-action dependencies and path
 * qualification. This class only preserves LazyTaskContainer's JavaScript API
 * for TypeScript plugin authors.
 */
class HvigorTaskCatalog {
  constructor(host, node) {
    this.host = host
    this.node = node
    this.taskDefinitions = undefined
  }

  invalidate() {
    this.taskDefinitions = undefined
  }

  get(name) {
    if (!this.taskDefinitions) this.taskDefinitions = this._load()
    return this.taskDefinitions.get(name)
  }

  has(name) {
    return this.get(name) !== undefined
  }

  entries() {
    if (!this.taskDefinitions) this.taskDefinitions = this._load()
    return this.taskDefinitions.entries()
  }

  request() {
    const targetNames = this.host._targetEntriesFor(this.node).map((target) => target.name)
    const compileApiVersion = this.host._compileApiVersion()
    const compatibleApiVersion = this.host._compatibleApiVersion()
    const ohpmProject = this.host._isOhpmProject()
    const unitTestProcess = this.host._parameter.getExtParam('unitTestMode') === 'true'
    const ohosTestCoverage = this.host._parameter.getExtParam('ohos-test-coverage') === 'true'
    const previewProcess =
      this.host._parameter.getExtParam('previewMode') === 'true' ||
      this.host._commandEntryTasks.includes('PreviewBuild')
    const inTest =
      this.host._parameter.getExtParam('isLocalTest') === 'true' ||
      this.host._parameter.getExtParam('isOhosTest') === 'true'
    return {
      nodeName: this.node.name,
      kind: this.node.kind,
      projectTask: this.node.isProject,
      projectRoot: this.host.projectRoot,
      nodeRoot: this.node.root,
      etsLoaderPath: this.host.config.etsLoaderPath,
      jsLoaderPath: this.host.config.jsLoaderPath,
      hasRollupPluginInEtsLoader:
        this.host.config.hasRollupPluginInEtsLoader ?? this.host.config.hasRollupPlugin,
      hasRollupPluginInJsLoader: this.host.config.hasRollupPluginInJsLoader,
      buildTargetName: this.host.config.buildTargetName,
      targetNames,
      targetOptions: this.node.isProject
        ? []
          : targetNames.map((name) => {
            const buildOption = this.host._buildOptionFor(this.node, { name })
            return {
              name,
              deduplicateHar: Boolean(buildOption?.packOptions?.deduplicateHar),
              binxoEnabled: this.host.binxoEnabled(buildOption),
              ohpmProject,
              compileApiVersion,
              compatibleApiVersion,
              openSourceHar: this.host._isOpenSourceHar(buildOption),
              externalNativeOptions: Boolean(buildOption?.externalNativeOptions),
              useNormalizedOhmUrl: Boolean(buildOption?.strictMode?.useNormalizedOHMUrl),
              hspDependencies: this.host._taskDependencyTargets(this.node, name, 'hsp'),
              harDependencies: this.host._taskDependencyTargets(this.node, name, 'har'),
            }
          }),
      childModules: this.node.children.map((child) => ({ name: child.name, kind: child.kind })),
      syncVersionChanged: this.host._syncVersionChanged,
      fastBuildApp: this.host.fastBuildApp(),
      buildAppSkipSignHap: this.host.buildAppSkipSignHap(),
      commandEntryTest: this.host._commandEntryTasks.includes('test'),
      unitTestProcess,
      ohosTestCoverage,
      previewProcess,
      previewCompileResourceIncremental:
        this.host._parameter.getExtParam('compileResInc') === 'true',
      inTest,
      deviceTest: this.host._commandEntryTasks.includes('onDeviceTest'),
      hotReload:
        this.host._parameter.getExtParam('hotReload') === 'true' &&
        Boolean(this.host.startParameters().daemon),
      coldReload: this.host._commandEntryTasks.includes('assembleDevHqf'),
    }
  }

  _load() {
    const request = this.request()
    const entries = JSON.parse(nativeRuntimeBinding.runtimeTaskCatalog(JSON.stringify(request)))
    return new Map(entries.map((entry) => [entry.name, entry]))
  }
}

/**
 * Public Task wrapper used by HvigorNodeImpl in @ohos/hvigor 6.24.3.
 *
 * CoreTaskImpl stores before hooks in a stack and after hooks in a queue. Keep
 * those data structures explicit here so hooks added through getTaskByName()
 * compose with hooks declared on HvigorTask in exactly the same direction.
 */
class HvigorTaskView {
  constructor(task) {
    this.task = task
  }

  getName() {
    return this.task.definition.name
  }

  getNameInternal() {
    return this.getName()
  }

  getDependencies() {
    return [...this.task.dependencies, ...this.task.userDependencies].map((dependency) =>
      dependency.startsWith(':') ? dependency.slice(1) : dependency,
    )
  }

  getDependenciesInternal() {
    return this.getDependencies()
  }

  getGroup() {
    return this.task.definition.group || this.task.node.taskCatalog.get(this.getName())?.group || 'Other'
  }

  getDescription() {
    return this.task.definition.description || this.task.node.taskCatalog.get(this.getName())?.description || ''
  }

  getPath() {
    return this.task.path
  }

  getNode() {
    return this.task.node.wrapper
  }

  setEnable(enabled) {
    this.task.enabled = enabled
  }

  setEnableInternal(enabled) {
    this.setEnable(enabled)
  }

  beforeRun(callback) {
    this.task.beforeRunHooks.push({ callback, declared: false })
  }

  beforeRunInternal(callback) {
    this.beforeRun(callback)
  }

  afterRun(callback) {
    this.task.afterRunHooks.push({ callback, declared: false })
  }

  afterRunInternal(callback) {
    this.afterRun(callback)
  }
}

class HvigorNodeDescriptorView {
  constructor(name, srcPath, rootDescriptor, extraOptions = {}) {
    this.name = name
    this.srcPath = srcPath
    this.extraOptions = new Map(Object.entries(extraOptions || {}))
    this.rootDescriptor = rootDescriptor || this
    this.children = undefined
  }

  getChildNode() {
    return this.children
  }

  getChildNodeInternal() {
    return this.getChildNode()
  }

  getRootNode() {
    return this.rootDescriptor
  }

  getRootNodeInternal() {
    return this.getRootNode()
  }

  setChildNode(children) {
    this.children = children
  }
}

/**
 * Mutable pre-node model exposed to hvigorconfig.ts.
 *
 * Mirrors latest IDE HvigorConfig: include/exclude updates descriptors before
 * Hvigor nodes are initialized; later lifecycle callbacks observe the updated
 * node tree.
 */
class HvigorConfigView {
  constructor(host) {
    this.host = host
    this.rootDescriptor = new HvigorNodeDescriptorView(path.basename(host.projectRoot), host.projectRoot)
    this.children = host._moduleDescriptors.map((descriptor) =>
      this._child(descriptor.name, descriptor.srcPath || path.relative(host.projectRoot, descriptor.root), descriptor),
    )
    this.rootDescriptor.children = this.children
  }

  _child(name, srcPath, descriptor = {}) {
    const child = new HvigorNodeDescriptorView(name, srcPath, this.rootDescriptor, descriptor.extraOptions)
    child.root = descriptor.root ? path.resolve(descriptor.root) : path.resolve(this.host.projectRoot, srcPath)
    child.targets = descriptor.targets
    return child
  }

  includeNode(name, srcPath, extraOptions) {
    if (typeof srcPath !== 'string' || srcPath.length === 0) {
      throw this.host._error('SOME_PATH_NOT_FOUND', `module path does not exist: ${String(srcPath)}`)
    }
    if (path.isAbsolute(srcPath)) {
      throw this.host._error('THE_SRCPATH_IS_NOT_A_RELATIVE_PATH', `module srcPath must be relative: ${srcPath}`)
    }
    const root = path.resolve(this.host.projectRoot, srcPath)
    if (!fs.existsSync(root)) {
      throw this.host._error('SOME_PATH_NOT_FOUND', `module path does not exist: ${srcPath}`)
    }
    this.children.push(this._child(name, srcPath, { extraOptions }))
    this.rootDescriptor.setChildNode(this.children)
  }

  includeNodeInternal(name, srcPath, extraOptions) {
    this.includeNode(name, srcPath, extraOptions)
  }

  excludeNodeByName(name) {
    if (!this.children.some((descriptor) => descriptor.name === name)) {
      this.host._warn('HVIGOR_CONFIG_NODE_NOT_FOUND', `The Node to exclude '${name}' was not found.`)
    }
    this.children = this.children.filter((descriptor) => descriptor.name !== name)
    this.rootDescriptor.children = this.children
  }

  excludeNodeByNameInternal(name) {
    this.excludeNodeByName(name)
  }

  getAllNodeDescriptor() {
    return this.children
  }

  getAllNodeDescriptorInternal() {
    return this.getAllNodeDescriptor()
  }

  getRootNodeDescriptor() {
    return this.rootDescriptor
  }

  getRootNodeDescriptorInternal() {
    return this.getRootNodeDescriptor()
  }

  getNodeDescriptorByName(name) {
    return this.children.find((descriptor) => descriptor.name === name)
  }

  getNodeDescriptorByNameInternal(name) {
    return this.getNodeDescriptorByName(name)
  }

  applyToHost() {
    this.host._moduleDescriptors = this.children.map((descriptor) =>
      this.host._loadModuleDescriptor({
        name: descriptor.name,
        srcPath: descriptor.srcPath,
        root: descriptor.root,
        targets: descriptor.targets,
        extraOptions: Object.fromEntries(descriptor.extraOptions),
      }),
    )
    this.host._invalidateRuntimePlan()
  }
}

class HvigorBuildResult {
  constructor(error, report) {
    this.error = error
    this.report = report
  }

  getError() {
    return this.error
  }

  getReportJson() {
    return this.report
  }
}

class HvigorToolingModelRegistry {
  constructor() {
    this.models = new Map()
  }

  registry(modelBean) {
    this.models.set(modelBean?.modelId, modelBean)
  }

  getModelMap() {
    return this.models
  }

  clear() {
    this.models.clear()
  }
}

/**
 * Compatibility owner for class-based DefaultTask/DefaultSyncTask plugins.
 * Hvigor still exposes these classes in 6.24.3 even though newer plugins tend
 * to register TaskCreation objects. The runtime converts this object to the
 * same internal task record when the owning node calls registerTask().
 */
class HvigorCoreTask {}

/**
 * CoreTask adapter for Rust-owned built-in tasks and object-style HvigorTask
 * registrations. The public Task wrapper intentionally remains a distinct
 * object, matching CoreTaskImpl#getHvigorTask in hvigor 6.24.3.
 */
class HvigorCoreTaskView extends HvigorCoreTask {
  constructor(host, task) {
    super()
    this.host = host
    this.task = task
  }

  getAction() {
    if (typeof this.task.definition.run === 'function') return this.task.definition.run
    return async () => {
      const action = this.host._runtimeBuiltinActions.get(this.task.path)
      if (typeof action !== 'function') {
        throw this.host._error(
          'ARKDOWN_BUILTIN_TASK_ACTION_NOT_BOUND',
          `built-in task ${this.task.path} can only execute inside the Rust Hvigor task runtime`,
        )
      }
      return action()
    }
  }

  getNode() {
    return this.task.node.coreWrapper
  }

  getName() {
    return this.task.definition.name
  }

  getPath() {
    return this.task.path
  }

  getDependsOn() {
    return [...this.task.dependencies, ...this.task.userDependencies]
  }

  setDependsOn(...dependencies) {
    this.host._setCoreTaskDependencies(this.task, dependencies)
    return this
  }

  addDependsOn(dependency) {
    this.host._addCoreTaskDependency(this.task, dependency)
    return this
  }

  dependsOn(dependency, targetNode) {
    this.host._addCoreTaskDependency(this.task, dependency, targetNode)
    return this
  }

  getEnabled() {
    return this.task.enabled
  }

  setEnabled(enabled) {
    this.task.enabled = enabled
    return this
  }

  setDescription(description) {
    this.task.definition.description = description
    return this
  }

  getDescription() {
    return this.task.definition.description || ''
  }

  setGroup(group) {
    this.task.definition.group = group
    return this
  }

  getGroup() {
    return this.task.definition.group || 'Other'
  }

  doNotTrackState(reason) {
    this.task.trackState = false
    this.task.unTrackStateReason = reason
  }

  getSourcePluginId() {
    return this.task.definition.sourcePluginId || ''
  }

  onFailed(error) {
    return this.task.taskContext?.onFailed?.(error)
  }

  getHvigorTask() {
    return this.task.wrapper
  }

  beforeRun(callback) {
    this.task.beforeRunHooks.push({ callback, declared: false })
  }

  afterRun(callback) {
    this.task.afterRunHooks.push({ callback, declared: false })
  }

  getTaskLog() {
    return this.host._publicUtilities.HvigorLogger.getLogger(this.getPath())
  }

  getWorkerPool() {
    return new this.host._publicUtilities.WorkerPoolDelegator()
  }

  taskShouldDo() {
    return this.task.taskContext?.taskShouldDo?.() !== false
  }

  async execute() {
    await this.host._runTaskBeforeHooks(this.task)
    await this.getAction().call(this)
    await this.host._awaitRuntimeTaskPendingPromises(this.task)
  }
}

class HvigorTaskContainerView {
  constructor(host, node) {
    this.host = host
    this.node = node
  }

  addTask(task) {
    this.host._registerTask(this.node, task)
  }

  registerTask(creation) {
    this.host._registerLazyTaskCreation(this.node, creation)
  }

  deleteTask(taskName) {
    const existed = this.hasTask(taskName)
    this.node.tasks.delete(taskName)
    this.node.lazyTaskCreations.delete(taskName)
    if (this.node.taskCatalog.has(taskName)) this.node.deletedCatalogTasks.add(taskName)
    return existed
  }

  findTask(taskName) {
    const task = this.host._getTaskByName(this.node, taskName)
    return task ? this.host._coreTaskFor(task) : undefined
  }

  getTask(taskName) {
    return this.findTask(taskName)
  }

  getLazyTask(taskName) {
    return this.host._taskCreationFor(this.node, taskName)
  }

  getTaskDepends(taskName) {
    const task = this.host._getTaskByName(this.node, taskName)
    if (!task) throw new Error(`Task ${taskName} not find.`)
    return this.host._coreTaskFor(task).getDependsOn()
  }

  hasTask(taskName) {
    return this.host._hasTask(this.node, taskName)
  }

  getAllTasks() {
    return Array.from(this.node.tasks.values(), (task) => this.host._coreTaskFor(task))
  }

  getAllTasksCA() {
    return this.host._allTaskCreations(this.node)
  }

  getTaskPaths() {
    return this.host._allTaskNames(this.node)
  }

  clearTasks() {
    this.node.tasks.clear()
    this.node.lazyTaskCreations.clear()
    for (const [taskName] of this.node.taskCatalog.entries()) this.node.deletedCatalogTasks.add(taskName)
  }
}

/**
 * Per-build HvigorCore state. Official hvigor exposes this object as a
 * singleton; keeping it on HvigorRuntimeHost preserves identity across every
 * mocked `require('@ohos/hvigor')` call without leaking state across builds.
 */
class HvigorCoreRuntimeView {
  constructor(host) {
    this.host = host
    this.project = undefined
    this.extraConfig = new Map(Object.entries(host._parameter.getExtParamsInternal()))
    this.cacheMap = new Map()
    this.metaInfo = new Map()
    this.changedModuleNameList = new Set()
    this.moduleDepRelationMap = new Map()
    this.compileDependArr = new Set()
    this.afterNodeInternalHooks = new Map()
    this.nodesEvaluatedInternalHooks = []
    this.nodesBeforeEvaluatedInternalHooks = []
  }

  getHvigorVersion() {
    return HVIGOR_PARITY_VERSION
  }

  getModuleDepRelationMap() {
    return this.moduleDepRelationMap
  }

  setChangedModuleNameList(changedModuleNameList) {
    this.changedModuleNameList = changedModuleNameList
  }

  getChangedModuleNameList() {
    return this.changedModuleNameList
  }

  setCompileDependArr(compileDependArr) {
    this.compileDependArr = compileDependArr
  }

  getCompileDependArr() {
    return this.compileDependArr
  }

  _metaFilePath() {
    return path.join(this.host.hvigorCacheDir(), 'cache', 'meta.json')
  }

  _readMeta() {
    const metaFile = this._metaFilePath()
    if (!fs.existsSync(metaFile)) return {}
    try {
      const value = JSON.parse(fs.readFileSync(metaFile, 'utf8'))
      return value && typeof value === 'object' ? value : {}
    } catch (cause) {
      throw this.host._error('INVALID_HVIGOR_META_FILE', `failed to read Hvigor metadata ${metaFile}`, {
        cause,
        path: metaFile,
      })
    }
  }

  versionChanged() {
    const changed = this.getMetaProperty('hvigorVersion') !== HVIGOR_PARITY_VERSION
    if (changed) this.updateMetaProperty('hvigorVersion', HVIGOR_PARITY_VERSION)
    return changed
  }

  updateMetaProperty(property, value) {
    this.metaInfo.set(property, value)
  }

  writeMetaProperties() {
    if (this.metaInfo.size === 0) return
    const metaFile = this._metaFilePath()
    const metadata = this._readMeta()
    for (const [property, value] of this.metaInfo) metadata[property] = value
    fs.mkdirSync(path.dirname(metaFile), { recursive: true })
    fs.writeFileSync(metaFile, JSON.stringify(metadata))
    this.metaInfo.clear()
  }

  getMetaProperty(property) {
    return this._readMeta()[property]
  }

  getProject() {
    return this.project
  }

  getParameter() {
    return this.host._parameter
  }

  getModuleByScriptPath(buildScriptFilePath) {
    return this.host._nodeForFile(buildScriptFilePath, false)?.coreWrapper
  }

  getHvigorNodeByScriptPath(buildScriptFilePath) {
    return this.host._nodeForFile(buildScriptFilePath, false)?.wrapper
  }

  getParentNodeByScriptPath(buildScriptFilePath) {
    return this.host._nodeForFile(buildScriptFilePath, false)?.parent?.wrapper
  }

  getSubNodeByScriptPath(buildScriptFilePath) {
    const node = this.host._nodeForFile(buildScriptFilePath, false)
    return node ? node.children.map((child) => child.wrapper) : undefined
  }

  getExtraConfig() {
    return this.extraConfig
  }

  setExtraConfig(value) {
    this.extraConfig = value
  }

  isCommandEntryTask(taskName) {
    return this.host._commandEntryTasks.includes(taskName)
  }

  setCommandEntryTasks(commandEntryTasks) {
    this.host._commandEntryTasks = commandEntryTasks
  }

  getCommandEntryTasks() {
    return this.host._commandEntryTasks.length > 0 ? this.host._commandEntryTasks : undefined
  }

  initRootProject(project) {
    this.project = project
  }

  reset() {
    this.extraConfig.clear()
    for (const node of this.host._nodesByFile.values()) {
      node.taskContainer.clearTasks()
      node.taskGraph.clear()
    }
    this.host._nodesByFile.clear()
    this.host._nodeAliasesByFile.clear()
    this.host._nodesByName.clear()
    this.host._commandEntryTasks = []
    this.project = undefined
    this.host._invalidateRuntimePlan()
  }

  hvigorAfterNodeInternalHook(hvigorNodeName, callback) {
    const callbacks = this.afterNodeInternalHooks.get(hvigorNodeName) || []
    callbacks.push(callback)
    this.afterNodeInternalHooks.set(hvigorNodeName, callbacks)
  }

  getHvigorAfterNodeInternalHookList(hvigorNodeName) {
    return this.afterNodeInternalHooks.get(hvigorNodeName)
  }

  hvigorNodeEvaluatedInternalHook(callback) {
    this.nodesEvaluatedInternalHooks.push(callback)
  }

  hvigorNodeBeforeEvaluatedInternalHook(callback) {
    this.nodesBeforeEvaluatedInternalHooks.push(callback)
  }

  getCache(cacheName) {
    if (!this.cacheMap.has(cacheName)) this.cacheMap.set(cacheName, new Map())
    return this.cacheMap.get(cacheName)
  }

  cleanCache() {
    this.cacheMap.clear()
    this.changedModuleNameList.clear()
    this.moduleDepRelationMap.clear()
  }

  getHvigorNodesEvaluatedInternalHook() {
    return this.nodesEvaluatedInternalHooks
  }

  getHvigorNodesBeforeEvaluatedInternalHook() {
    return this.nodesBeforeEvaluatedInternalHooks
  }

  clearHvigorInternalHookList() {
    this.afterNodeInternalHooks.clear()
    this.nodesEvaluatedInternalHooks = []
    this.nodesBeforeEvaluatedInternalHooks = []
  }
}

class HvigorTaskPendingPromises {
  constructor() {
    this.promiseSet = new Set()
  }

  add(promise) {
    this.promiseSet.add(promise)
  }

  get() {
    return Array.from(this.promiseSet)
  }

  clear() {
    this.promiseSet.clear()
  }
}

class HvigorWorkerTaskControlBlock extends EventEmitter {
  constructor(pool, work, task, options, state = HVIGOR_TASK_STATE.WAITING) {
    super()
    this.pool = pool
    this.work = work
    this.task = task
    this.callback = typeof options?.callback === 'function' ? options.callback : () => {}
    this.callbackInput = Array.isArray(options?.callbackInput) ? options.callbackInput : []
    this.useReturnValue = Boolean(options?.useReturnVal)
    this.customActions = new Map()
    this.submitTime = Date.now()
    this.startTime = undefined
    this.endTime = undefined
    this.workerId = undefined
    this.state = state
  }

  getId() {
    return this.work.id
  }

  getWorkerId() {
    return this.workerId
  }

  getSubmitTime() {
    return this.submitTime
  }

  getStartTime() {
    return this.startTime
  }

  getEndTime() {
    return this.endTime
  }

  getState() {
    return this.state
  }

  getPriority() {
    return this.work.priority
  }

  getCallback() {
    return this.callback
  }

  getCallbackInput() {
    return this.callbackInput
  }

  setPriority(priority) {
    if (this.state !== HVIGOR_TASK_STATE.WAITING) return false
    this.work.priority = priority
    this.pool.update(this)
    return true
  }

  setCallback(callback, callbackInput) {
    if (![HVIGOR_TASK_STATE.WAITING, HVIGOR_TASK_STATE.RUNNING].includes(this.state)) return false
    this.callback = callback
    if (callbackInput !== undefined) this.callbackInput = callbackInput
    return true
  }

  setCustomAction(customType, action) {
    if (![HVIGOR_TASK_STATE.WAITING, HVIGOR_TASK_STATE.RUNNING].includes(this.state)) return false
    this.customActions.set(customType, action)
    return true
  }

  popCustomAction(customType) {
    const action = this.customActions.get(customType)
    this.customActions.delete(customType)
    return action
  }

  getTaskCompletePath() {
    return this.task.path
  }

  getTaskName() {
    return this.task.definition.name
  }

  getTaskPath() {
    return this.task.node.name
  }

  useReturnVal() {
    return this.useReturnValue
  }
}

class HvigorWorkerPool {
  constructor(host) {
    this.host = host
    this.maxWorkers = Math.max(0, os.cpus().length - 1)
    this.minWorkers = 0
    this.nextWorkId = 0
    this.nextSequence = 0
    this.queue = []
    this.slots = new Map()
    this.closing = false
    this.state = HVIGOR_POOL_STATE.RUNNING
    this.resident = false
    this.recycleInterval = HvigorPoolConstant.DEFAULT_RECYCLE_INTERVAL
    this.maxIdleTime = HvigorPoolConstant.DEFAULT_MAX_IDLE_TIME
    this.errorCallback = undefined
    this.logs = new Map()
  }

  submit(task, workPath, options = {}) {
    const targetWorkers = this.filteredTargetWorkers(options.targetWorkers)
    const work = {
      id: String(this.nextWorkId++),
      sequence: this.nextSequence++,
      priority: options.priority ?? HVIGOR_WORKER_PRIORITY.MEDIUM,
      targetWorkers,
      workPath,
      workInput: options.workInput,
    }
    const accepted =
      this.host.startParameters().parallelExecution &&
      !this.closing &&
      this.state === HVIGOR_POOL_STATE.RUNNING &&
      this.maxWorkers > 0 &&
      (targetWorkers === undefined || targetWorkers.length > 0)
    const tcb = new HvigorWorkerTaskControlBlock(
      this,
      work,
      task,
      options,
      accepted ? HVIGOR_TASK_STATE.WAITING : HVIGOR_TASK_STATE.REJECT,
    )
    HvigorTcbStore.add(tcb)
    if (!accepted) return tcb

    const completion = new Promise((resolve, reject) => {
      work.resolve = resolve
      work.reject = reject
    })
    const pendingPromises = task.taskContext?.pendingPromises
    if (!pendingPromises || typeof pendingPromises.add !== 'function') {
      throw this.host._error(
        'FAILED_TO_SUBMIT_WORKER_TASK_NOT_SUPPORTED',
        `task ${task.path} does not expose Hvigor pending promises`,
      )
    }
    pendingPromises.add(completion)
    work.tcb = tcb
    this.queue.push(work)
    this.sortQueue()
    this.dispatch()
    return tcb
  }

  filteredTargetWorkers(targetWorkers) {
    if (!Array.isArray(targetWorkers)) return undefined
    const seen = new Set()
    return targetWorkers.filter((workerId) => {
      if (!Number.isInteger(workerId) || workerId < 0 || workerId >= this.maxWorkers || seen.has(workerId)) {
        return false
      }
      seen.add(workerId)
      return true
    })
  }

  update(tcb) {
    if (tcb.getState() !== HVIGOR_TASK_STATE.WAITING) return
    this.sortQueue()
    this.dispatch()
  }

  sortQueue() {
    this.queue.sort((left, right) => left.priority - right.priority || left.sequence - right.sequence)
  }

  dispatch() {
    if (this.closing) return
    let dispatched
    do {
      dispatched = false
      for (let index = 0; index < this.queue.length; index++) {
        const work = this.queue[index]
        const slot = this.availableSlot(work.targetWorkers)
        if (slot === undefined) continue
        this.queue.splice(index, 1)
        this.start(slot, work)
        dispatched = true
        break
      }
    } while (dispatched)
  }

  availableSlot(targetWorkers) {
    const allowed = targetWorkers || Array.from({ length: this.maxWorkers }, (_, index) => index)
    for (const workerId of allowed) {
      const slot = this.slots.get(workerId)
      if (slot === undefined) return this.createSlot(workerId)
      if (!slot.busy) return slot
    }
    return undefined
  }

  createSlot(workerId) {
    const worker = new Worker(path.join(__dirname, 'hvigor-worker.js'))
    const slot = { workerId, worker, busy: false, work: undefined }
    worker.on('message', (message) => this.onMessage(slot, message))
    worker.on('error', (error) => this.onWorkerError(slot, error))
    worker.on('exit', (code) => this.onWorkerExit(slot, code))
    worker.unref()
    this.slots.set(workerId, slot)
    return slot
  }

  start(slot, work) {
    slot.busy = true
    slot.work = work
    slot.worker.ref()
    work.tcb.workerId = slot.workerId
    work.tcb.state = HVIGOR_TASK_STATE.RUNNING
    slot.worker.postMessage({
      id: work.id,
      workPath: work.workPath,
      workInput: work.workInput,
    })
  }

  async onMessage(slot, message) {
    const work = slot.work
    if (!work || message?.id !== work.id) return
    if (message.event === HVIGOR_CUSTOM_WORKER_REQUEST) {
      const action = work.tcb.popCustomAction(message.customType)
      try {
        await action?.(message.param)
      } finally {
        slot.worker.postMessage({ ...message, done: true })
      }
      return
    }
    work.tcb.startTime = message.startTime
    work.tcb.endTime = message.endTime ?? Number(process.hrtime.bigint())
    if (message.event === 'work error') {
      const error = this.workerError(message.returnVal)
      this.finish(slot, work, HVIGOR_TASK_STATE.ERROR, error)
      return
    }
    try {
      const callbackArgs = work.tcb.useReturnVal()
        ? [message.returnVal, slot.workerId]
        : [...work.tcb.getCallbackInput(), slot.workerId]
      await work.tcb.getCallback()(...callbackArgs)
      this.finish(slot, work, HVIGOR_TASK_STATE.END)
    } catch (error) {
      this.finish(slot, work, HVIGOR_TASK_STATE.ERROR, error)
    }
  }

  workerError(value) {
    const error = new Error(value?.message || 'worker execution failed')
    error.name = value?.name || 'Error'
    if (value?.stack) error.stack = value.stack
    if (value?.extraInfo !== undefined) error.extraInfo = value.extraInfo
    return error
  }

  finish(slot, work, state, error) {
    work.tcb.state = state
    slot.busy = false
    slot.work = undefined
    slot.worker.unref()
    if (error === undefined) {
      work.resolve(true)
      work.tcb.emit('work done', work.id)
    } else {
      work.reject(error)
      work.tcb.emit('work error', { id: work.id, error })
      this.errorCallback?.(error)
    }
    this.dispatch()
  }

  onWorkerError(slot, error) {
    const work = slot.work
    if (work) {
      work.tcb.endTime = Number(process.hrtime.bigint())
      this.finish(slot, work, HVIGOR_TASK_STATE.ERROR, error)
    }
  }

  onWorkerExit(slot, code) {
    this.slots.delete(slot.workerId)
    const work = slot.work
    if (work && work.tcb.getState() === HVIGOR_TASK_STATE.RUNNING) {
      const error = new Error(`Hvigor worker ${slot.workerId} exited with code ${code}`)
      work.tcb.endTime = Number(process.hrtime.bigint())
      work.tcb.state = HVIGOR_TASK_STATE.ERROR
      work.reject(error)
      work.tcb.emit('work error', { id: work.id, error })
    }
    this.dispatch()
  }

  async shutdown() {
    this.closing = true
    this.state = HVIGOR_POOL_STATE.TERMINATED
    const workers = Array.from(this.slots.values(), (slot) => slot.worker.terminate())
    this.slots.clear()
    await Promise.allSettled(workers)
    return true
  }

  getState() {
    return this.state
  }

  setState(state) {
    this.state = state
    if (state === HVIGOR_POOL_STATE.RUNNING) {
      this.closing = false
      this.dispatch()
    } else if (state === HVIGOR_POOL_STATE.TERMINATED) {
      this.closing = true
    }
  }

  setRecycleInterval(recycleInterval) {
    this.recycleInterval = recycleInterval
  }

  setMaxIdleTime(maxIdleTime) {
    this.maxIdleTime = maxIdleTime
  }

  setErrorCallback(errorCallback) {
    this.errorCallback = errorCallback
  }

  getLog(type) {
    return this.logs.get(type)
  }

  clearLog(type) {
    if (type === undefined) this.logs.clear()
    else this.logs.delete(type)
  }

  getMaxPoolNum() {
    return this.maxWorkers
  }

  getMinPoolNum() {
    return this.minWorkers
  }

  setResident(resident) {
    this.resident = resident
  }

  isActive() {
    return this.state !== HVIGOR_POOL_STATE.TERMINATED && (this.slots.size > 0 || this.queue.length > 0)
  }
}

class HvigorDefaultTask extends HvigorCoreTask {
  constructor(node, taskInfo) {
    super()
    const details = typeof taskInfo === 'string' ? { name: taskInfo || 'default' } : taskInfo || {}
    this.node = node
    this.name = details.name
    this.group = details.group || 'Other'
    this.isEnabled = details.isEnabled ?? true
    this.description = details.description || ''
    this.sourcePluginId = details.sourcePluginId || ''
    this.dependsTask = []
    this.beforeRunHooks = []
    this.afterRunHooks = []
    this.unTrackStateReason = undefined
    this.pendingPromises = new HvigorTaskPendingPromises()
    this.subDurationEventMap = new Map()
    this.workerDelegator = undefined
    this.taskExecutedStatus = {
      upToDate: false,
      successful: false,
      workerTimePeriod: undefined,
      isUpToDate() {
        return this.upToDate
      },
      setUpToDate(value = true) {
        this.upToDate = value
      },
      setWorkerTimePeriod(period) {
        this.workerTimePeriod = period
      },
    }
    const metricFactory = node?.__arkdownHost?._metricCompatibility?.MetricFactory
    this.durationEvent = metricFactory?.createDurationEvent(
      node.classKind === 'project' ? `:${this.name}` : `${node.getName()}:${this.name}`,
      this.description,
      this.group,
    )
    this.__arkdownTaskWrapper = undefined
    this.__arkdownClassTask = true
  }

  registryAction() {
    return () => {}
  }

  getAction() {
    return this.registryAction()
  }

  getWorkerPool() {
    const WorkerPoolDelegator = this.node?.__arkdownHost?._publicUtilities?.WorkerPoolDelegator
    if (!WorkerPoolDelegator) {
      throw new Error('Hvigor worker pool is unavailable before the task is bound to an Hvigor node')
    }
    this.workerDelegator ||= new WorkerPoolDelegator()
    return this.workerDelegator
  }

  async execute() {
    try {
      for (const callback of [...this.beforeRunHooks].reverse()) await callback.call(this)
      await this.getAction().call(this)
      const pending = this.pendingPromises.get()
      this.pendingPromises.clear()
      await Promise.all(pending)
      for (const callback of this.afterRunHooks) await callback.call(this)
      this.taskExecutedStatus.successful = true
    } catch (error) {
      await this.onFailed(error)
      throw error
    }
  }

  getNode() {
    return this.node
  }

  getName() {
    return this.name
  }

  getPath() {
    return this.node.classKind === 'project' ? `:${this.name}` : `${this.node.getName()}:${this.name}`
  }

  getEnabled() {
    return this.isEnabled
  }

  setEnabled(enabled) {
    this.isEnabled = enabled
    return this
  }

  setEnable(enabled) {
    return this.setEnabled(enabled)
  }

  setDescription(description) {
    this.description = description
    return this
  }

  getDescription() {
    return this.description
  }

  setGroup(group) {
    this.group = group
    return this
  }

  getGroup() {
    return this.group
  }

  getSourcePluginId() {
    return this.sourcePluginId
  }

  setDependsOn(...dependencies) {
    this.dependsTask = [...dependencies]
    return this
  }

  getDependsOn() {
    return this.dependsTask
  }

  addDependsOn(dependency) {
    this.dependsTask.push(dependency)
    return this
  }

  dependsOn(dependency, targetNode) {
    const name = typeof dependency?.getName === 'function' ? dependency.getName() : dependency
    let owner = targetNode
    if (typeof targetNode === 'string') {
      const project = this.node.getProject()
      owner = targetNode === ':' || targetNode === project.getName() ? project : project.findModuleByName(targetNode)
    }
    owner ||= this.node
    this.dependsTask.push(owner.classKind === 'project' ? `:${name}` : `${owner.getName()}:${name}`)
    return this
  }

  beforeRun(callback) {
    this.beforeRunHooks.push(callback)
  }

  afterRun(callback) {
    this.afterRunHooks.push(callback)
  }

  taskShouldDo() {
    return true
  }

  doNotTrackState(reason) {
    this.unTrackStateReason = reason
  }

  onFailed(_error) {}

  getTaskLog() {
    return this.node?.__arkdownHost?._publicUtilities?.HvigorLogger.getLogger(this.getPath())
  }

  addSubDurationEvent(workId) {
    const event = this.durationEvent?.createSubEvent(`work:${workId}`, '')
    if (event) this.subDurationEventMap.set(workId, event)
    return event
  }

  getSubDurationEvent(workId) {
    return this.subDurationEventMap.get(workId)
  }

  getHvigorTask() {
    return this.__arkdownTaskWrapper
  }
}

class HvigorIncrementalTask extends HvigorDefaultTask {
  declareInputs() {
    return new Map()
  }

  declareInputFiles() {
    return new HvigorFileSet()
  }

  declareOutputFiles() {
    return new HvigorFileSet()
  }

  async beforeAlwaysAction() {}
}

class HvigorIncrementalExecTask extends HvigorIncrementalTask {
  declareExecutionTool() {
    return ''
  }

  declareExecutionCommand() {
    return ''
  }

  declareExecutionEnv() {
    return new Map()
  }
}

class HvigorDefaultSyncTask extends HvigorDefaultTask {
  constructor(node, taskInfo) {
    super(node, taskInfo)
    this.group = 'Sync'
    this.__arkdownSyncTask = true
  }
}

/**
 * Runtime host for the two hvigorfile plugin layers:
 *
 * 1. bind the exported OHOS system plugin and expose its node context;
 * 2. bind and execute TypeScript HvigorPlugin objects (`pluginId`, `context`,
 *    `apply(node)`) after each node is evaluated.
 *
 * The ordering follows @ohos/hvigor 6.24.3
 * `src/base/internal/lifecycle/configuration.js`: project first, modules in
 * sequence, system binding before custom-plugin application, then the global
 * `nodesEvaluated` hook. Package-bound tasks use the dependency direction from
 * `src/base/vigor/task/impl/transform-hvigor-task.js`.
 */
class HvigorRuntimeHost {
  constructor(projectRoot = process.cwd(), config = {}, inspectHvigorContext) {
    this.projectRoot = path.resolve(projectRoot)
    this.config = config || {}
    this.inspectHvigorContext = inspectHvigorContext
    this._nodesByFile = new Map()
    // CommonJS canonicalizes paths through filesystem aliases (for example
    // `/var` to `/private/var` on macOS) before exposing `__filename`. Keep
    // those loader-discovered spellings separate from the primary inventory
    // so node iteration remains unique and performs no additional realpath IO.
    this._nodeAliasesByFile = new Map()
    this._nodesByName = new Map()
    this._configEvaluatedCallbacks = []
    this._nodesInitializedCallbacks = []
    this._beforeNodeEvaluateCallbacks = []
    this._afterNodeEvaluateCallbacks = []
    this._nodesEvaluatedCallbacks = []
    this._taskGraphResolvedCallbacks = []
    this._buildFinishedCallbacks = []
    this._watchWorkerMessageCallbacks = []
    this._fallbackEffects = this._newEffects()
    this._effects = this._fallbackEffects
    this._activeFile = null
    this.hvigorWorkspaceNodeModules = null
    const runtimeModel = this._loadRuntimeModel()
    this.projectRoot = path.resolve(runtimeModel.projectRoot)
    this._projectCacheHash = runtimeModel.projectCacheHash
    this._projectProfile = runtimeModel.projectProfile || {}
    this._projectOhPackageExists = Boolean(runtimeModel.projectOhPackageExists)
    this._projectNpmPackageExists = Boolean(runtimeModel.projectNpmPackageExists)
    this._appJson = runtimeModel.appJson
    this._projectHvigorConfigPath = runtimeModel.projectHvigorConfigPath
    this._hvigorFileConfig = runtimeModel.projectHvigorConfig || {}
    this._userHvigorConfig = runtimeModel.userHvigorConfig || {}
    this._buildConfigFile = runtimeModel.buildConfigFile
    this._fallbackModuleRoots = toArray(runtimeModel.fallbackModuleRoots).map((root) => path.resolve(root))
    this._runtimeModuleModelsByRoot = new Map()
    this._runtimePlanCache = undefined
    this._commandEntryTasks = toArray(this.config.tasks ?? this.config.task)
    this._selectedProductName = this.config.product || this._projectProfile.app?.products?.[0]?.name || 'default'
    this._usesOhpm = this._isOhpmProject()
    this._projectPackage = this._usesOhpm
      ? runtimeModel.projectPackage || {}
      : runtimeModel.projectNpmPackage || {}
    this._moduleDescriptors = toArray(runtimeModel.modules).map((module) =>
      this._descriptorFromRuntimeModel(module),
    )
    this._hvigorConfig = new HvigorConfigView(this)
    this._parameter = this._makeParameter()
    this._hvigorCore = new HvigorCoreRuntimeView(this)
    this._hvigorApi = undefined
    this._hvigorExports = new Map()
    this._nativeCommandRoots = []
    this._nativeSelectionRoots = undefined
    this._nativeRequiredBoundaryTasks = []
    this._nativeExecutionNodeNames = undefined
    this._runtimeTasksByPath = new Map()
    this._runtimeTaskFailures = new Map()
    this._runtimeBuiltinActions = new Map()
    this._hvigorTypeScriptRoots = new Set()
    this._runtimeProjectOverrides = {}
    this._runtimeModuleOverrides = new Map()
    this._ohpmPackageModelUpdated = false
    this._projectHvigorOverrides = {}
    this._moduleHvigorOverrides = new Map()
    this._contextInspection = undefined
    this._toolingModelRegistry = new HvigorToolingModelRegistry()
    this._commandBuilderModifiers = new Map()
    this._workerPool = new HvigorWorkerPool(this)
    this._syncNodeNames = undefined
    this._syncVersionChanged = this._readSyncVersionChanged()
  }

  runtimeContextOverrides() {
    if (Object.keys(this._runtimeProjectOverrides).length === 0 && this._runtimeModuleOverrides.size === 0) {
      return undefined
    }
    return {
      project: this._clone(this._runtimeProjectOverrides),
      modules: Object.fromEntries(
        Array.from(this._runtimeModuleOverrides, ([root, value]) => [root, this._clone(value)]),
      ),
    }
  }

  startParameters() {
    return this._parameter.getCoreStartParams()
  }

  packCompressLevel() {
    return {
      fast: 1,
      standard: 5,
      ultimate: 9,
    }[this._parameter.getProperty('ohos.pack.compressLevel')]
  }

  alignDeviceTypes() {
    const value = this._parameter.getPropertyInternal('ohos.align.deviceTypes')
    if (typeof value === 'string') return value.split(',')
    return Array.isArray(value) ? [...value] : []
  }

  collectDebugSymbol() {
    const value = this._parameter.getPropertyInternal('ohos.collect.debugSymbol')
    return value === undefined ? undefined : Boolean(value)
  }

  uiTransformOptimization() {
    return this._parameter.getProperty('ohos.uiTransform.Optimization') === true
  }

  integratedHspUseGetCurrentBundleName() {
    return this._parameter.getProperty('ohos.integratedHsp.useGetCurrentBundleName') === true
  }

  byteCodeHarIntegratedOptimization() {
    // DevEco Studio 6.1.1
    // InjectUtil.isByteCodeHarOptimize() uses getPropertyInternal and requires
    // the literal boolean true; string values such as "true" stay disabled.
    return this._parameter.getPropertyInternal('ohos.byteCodeHar.integratedOptimization') === true
  }

  keepDependency() {
    // ProcessHarArtifacts.deleteOhPkgDep uses JavaScript truthiness. The
    // latest core seeds this property with true, so only a falsy override
    // enables bundled ByteCode HAR dependency pruning.
    return Boolean(this._parameter.getPropertyInternal('hvigor.keepDependency'))
  }

  enableSignTask() {
    return Boolean(this._parameter.getPropertyInternal('enableSignTask'))
  }

  fastBuildApp() {
    const enabled = Boolean(
      this._selectedProductDefinition()?.buildOption?.packOptions?.fastBuildApp ??
        this._projectProfile.app?.buildOption?.packOptions?.fastBuildApp,
    )
    if (!enabled) return false
    return this.config.target === 'app' || this._commandEntryTasks.some((task) => /(^|:)assembleApp$/.test(task))
  }

  buildAppSkipSignHap() {
    const enabled = Boolean(
      this._selectedProductDefinition()?.buildOption?.packOptions?.buildAppSkipSignHap ??
        this._projectProfile.app?.buildOption?.packOptions?.buildAppSkipSignHap,
    )
    return enabled && this._commandEntryTasks.includes('assembleApp')
  }

  _apiVersion(value) {
    if (typeof value === 'number' && Number.isFinite(value)) return Math.trunc(value)
    if (typeof value !== 'string') return undefined
    const normalized = value.trim()
    if (/^\d+$/.test(normalized)) return Number(normalized)
    const parenthesized = normalized.match(/\((\d+)\)$/)
    return parenthesized ? Number(parenthesized[1]) : undefined
  }

  _compileApiVersion() {
    const product = this._selectedProductDefinition()
    return (
      this._apiVersion(product.compileSdkVersion) ??
      this._apiVersion(this.config.compileApiVersion) ??
      this._apiVersion(this.config.compileSdkVersion) ??
      this._apiVersion(product.targetSdkVersion) ??
      this._apiVersion(product.compatibleSdkVersion) ??
      10
    )
  }

  _compatibleApiVersion() {
    const product = this._selectedProductDefinition()
    return (
      this._apiVersion(product.compatibleSdkVersion) ??
      this._apiVersion(this.config.compatibleApiVersion) ??
      this._apiVersion(this.config.compatibleSdkVersion) ??
      this._compileApiVersion()
    )
  }

  _isOhpmProject() {
    return (
      this._compileApiVersion() >= 9 &&
      (this._projectOhPackageExists || !this._projectNpmPackageExists)
    )
  }

  _isOpenSourceHar(buildOption) {
    const debug = buildOption?.debuggable === undefined || Boolean(buildOption.debuggable)
    if (debug) return true
    return buildOption?.arkOptions?.obfuscation?.ruleOptions?.enable !== true
  }

  _localModuleDependencies(node) {
    const project = node.isProject ? node : node.parent
    if (!project) return []
    const declarations = {
      ...node.modulePackage?.dependencies,
      ...node.modulePackage?.dynamicDependencies,
    }
    const dependencies = []
    const seen = new Set()
    for (const [alias, specifier] of Object.entries(declarations)) {
      let dependency = project.children.find(
        (candidate) => candidate !== node && (candidate.name === alias || candidate.modulePackage?.name === alias),
      )
      if (!dependency && typeof specifier === 'string' && specifier.startsWith('file:')) {
        const declaredRoot = path.resolve(node.root, specifier.slice('file:'.length))
        dependency = project.children.find(
          (candidate) => candidate !== node && this._pathIdentity(candidate.root) === this._pathIdentity(declaredRoot),
        )
      }
      if (!dependency || !['hsp', 'har'].includes(dependency.kind) || seen.has(dependency.name)) continue
      seen.add(dependency.name)
      dependencies.push(dependency)
    }
    return dependencies
  }

  _dependencyTargetName(ownerTargetName, dependency, kind) {
    const targets = this._targetEntriesFor(dependency)
    const targetNames = targets.map((target) => target.name)
    const explicit = this._moduleSelections().find(
      (selection) => selection.moduleName === dependency.name && selection.targetName,
    )?.targetName
    if (explicit && targetNames.includes(explicit)) return explicit
    if (kind === 'har' && ownerTargetName === 'ohosTest' && targetNames.includes('default')) {
      return 'default'
    }
    const aligned = this._parameter.getPropertyInternal('ohos.align.target')
    if (typeof aligned === 'string' && targetNames.includes(aligned)) return aligned
    if (targetNames.includes(ownerTargetName)) return ownerTargetName
    if (targetNames.includes('default')) return 'default'
    return targetNames[0] || 'default'
  }

  _taskDependencyTargets(node, targetName, kind) {
    return this._localModuleDependencies(node)
      .filter((dependency) => dependency.kind === kind)
      .map((dependency) => {
        const dependencyTargetName = this._dependencyTargetName(targetName, dependency, kind)
        const dependencyTarget = { name: dependencyTargetName }
        const buildOption = this._buildOptionFor(dependency, dependencyTarget)
        return {
          moduleName: dependency.name,
          moduleRoot: dependency.root,
          targetName: dependencyTargetName,
          externalNativeOptions: Boolean(buildOption?.externalNativeOptions),
          useNormalizedOhmUrl: Boolean(buildOption?.strictMode?.useNormalizedOHMUrl),
        }
      })
  }

  assertSupportedBuildExtensions() {
    for (const node of this._nodesByName.values()) {
      if (node.isProject) continue
      for (const target of this._targetEntriesFor(node)) {
        const cangjieOptions = this._buildOptionFor(node, target)?.cangjieOptions
        if (
          cangjieOptions &&
          typeof cangjieOptions === 'object' &&
          Object.prototype.hasOwnProperty.call(cangjieOptions, 'path')
        ) {
          throw this._error(
            'ARKDOWN_UNSUPPORTED_CANGJIE_BUILD',
            `Cangjie compilation is configured for ${node.name}:${target.name}, but Arkdown cannot execute the external @ohos/cangjie-build-support task pipeline yet`,
            {
              module: node.name,
              target: target.name,
              feature: 'cangjieOptions',
            },
          )
        }
      }
    }
  }

  binxoEnabled(buildOption) {
    const configured = this._parameter.getExtParam('ohos-enable-binxo')
    if (configured) return configured === 'true'
    const values = buildOption?.externalNativeOptions?.arguments
    const argumentsList = (typeof values === 'string' ? [values] : toArray(values))
      .flatMap((value) => (typeof value === 'string' ? value.trim().split(' ') : []))
      .filter((value) => value.startsWith('-'))
    return argumentsList.includes('-DOHOS_ENABLE_BINXO=ON')
  }

  runtimeProperties(startParams) {
    const properties = this._parameter.getPropertiesInternal()
    const alignTarget = properties['ohos.align.target']
    if (alignTarget !== undefined && typeof alignTarget !== 'string') {
      throw this._error('INVALID_ALIGN_TARGET', 'ohos.align.target must be a string')
    }
    const sourceMapDir = properties[OHOS_ARK_COMPILE_SOURCE_MAP_DIR]
    if (sourceMapDir !== undefined && typeof sourceMapDir !== 'string') {
      throw this._error(
        'INVALID_SOURCE_MAP_DIR',
        `${OHOS_ARK_COMPILE_SOURCE_MAP_DIR} must be a string`,
      )
    }
    const fallback = properties['ohos.fallback.target']
    const fallbackTargets =
      fallback === undefined || fallback === ''
        ? []
        : typeof fallback === 'string'
          ? fallback.split(',')
          : Array.isArray(fallback) && fallback.every((value) => typeof value === 'string')
            ? fallback.slice()
            : null
    if (fallbackTargets === null) {
      throw this._error(
        'INVALID_FALLBACK_TARGET',
        'ohos.fallback.target must be a comma-separated string or string array',
      )
    }
    const releaseCmakeBuildType = properties['ohos.defaults.release.cmakebuildtype']
    if (releaseCmakeBuildType !== undefined && typeof releaseCmakeBuildType !== 'string') {
      throw this._error(
        'INVALID_CMAKE_BUILD_TYPE',
        'ohos.defaults.release.cmakebuildtype must be a string',
      )
    }
    const compileLibEntryFile = properties['ohos.compile.lib.entryfile']
    if (compileLibEntryFile !== undefined && typeof compileLibEntryFile !== 'boolean') {
      throw this._error(
        'INVALID_COMPILE_LIB_ENTRY_FILE',
        'ohos.compile.lib.entryfile must be a boolean',
      )
    }
    const previewerReplacePage = this._parameter.getExtParam('previewer.replace.page')
    const mainModule = this._parameter.getExtParam('mainModule')
    const pageType = this._parameter.getExtParam('pageType')
    for (const [name, value] of [
      ['previewer.replace.page', previewerReplacePage],
      ['mainModule', mainModule],
      ['pageType', pageType],
    ]) {
      if (value !== undefined && typeof value !== 'string') {
        throw this._error('INVALID_PREVIEW_PROPERTY', `${name} must be a string`)
      }
    }
    const compileHspDependencies = this._parameter.getExtParam('compileHspDependencies')
    const asanConfig = this._parameter.getExtParam('ohos-debug-asan')
    const tsanConfig = this._parameter.getExtParam('ohos-enable-tsan')
    const hwasanConfig = this._parameter.getExtParam('ohos-enable-hwasan')
    const ubsanConfig = this._parameter.getExtParam('ohos-enable-ubsan')
    const binxoConfig = this._parameter.getExtParam('ohos-enable-binxo')
    const runtimeProperties = {
      daemon: Boolean(startParams?.daemon),
      harExcludeHspDependencies: properties['ohos.har.excludeHspDependencies'] === true,
      dependenciesTypesEnable: properties['ohos.dependencies.types.enable'] === true,
      singleFileEmit: Boolean(properties['ohos.arkCompile.singleFileEmit']),
      rollupCacheUseSourceHash: properties['ohos.rollupCache.useSourceHash'] === true,
      processLibOptimization: Boolean(properties['ohos.processLib.optimization']),
      fallbackTargets,
      obfuscationRulesOptimization: properties['ohos.obfuscationRules.optimization'] === true,
      signHar: properties['ohos.sign.har'] === true,
      previewerCompileResIncremental:
        this._parameter.getExtParam('compileResInc') === 'true',
      // Upstream intentionally has two different predicates for this value:
      // PreviewerArkCompile compares with the literal string "true", while
      // ReplacePreviewerPage uses JavaScript truthiness.
      compileHspDependencies: compileHspDependencies === 'true',
      compileHspDependenciesTruthy: Boolean(compileHspDependencies),
    }
    if (typeof alignTarget === 'string' && alignTarget.length > 0) runtimeProperties.alignTarget = alignTarget
    if (sourceMapDir !== undefined) runtimeProperties.sourceMapDir = sourceMapDir
    if (compileLibEntryFile !== undefined) runtimeProperties.compileLibEntryFile = compileLibEntryFile
    if (releaseCmakeBuildType !== undefined) runtimeProperties.releaseCmakeBuildType = releaseCmakeBuildType
    if (previewerReplacePage !== undefined) runtimeProperties.previewerReplacePage = previewerReplacePage
    if (mainModule !== undefined) runtimeProperties.mainModule = mainModule
    if (pageType !== undefined) runtimeProperties.pageType = pageType
    if (asanConfig !== undefined) runtimeProperties.asanConfig = asanConfig === 'true'
    if (tsanConfig !== undefined) runtimeProperties.tsanConfig = tsanConfig === 'true'
    if (ubsanConfig !== undefined) runtimeProperties.ubsanConfig = ubsanConfig === 'true'
    // BinxoValidationService treats only a truthy extra-config value as an
    // override; an absent or empty string falls back to CMake arguments. A
    // truthy value is enabled only when it is the literal string "true".
    if (hwasanConfig) runtimeProperties.hwasanConfig = hwasanConfig === 'true'
    if (binxoConfig) runtimeProperties.binxoConfig = binxoConfig === 'true'
    const withDeduplicate = this._parameter.getExtParam('withDeduplicate')
    if (withDeduplicate !== undefined) {
      runtimeProperties.withDeduplicate = withDeduplicate === 'true'
    }
    return runtimeProperties
  }

  parameterFileOverride() {
    if (!this._usesOhpm) return undefined
    const value = this._parameter.getExtParamInternal('parameterFile')
    const parameterFile = typeof value === 'string' && value.length > 0 ? value : this._projectPackage?.parameterFile
    return this._replaceStringParameters(parameterFile, this._projectPackage?.properties)
  }

  hvigorCacheDir(logger) {
    let configured =
      this._parameter.getExtParam('build-cache-dir') ?? this._parameter.getPropertyInternal('hvigor.cacheDir')
    if (typeof configured !== 'string' || !path.isAbsolute(configured)) {
      if (configured && logger) logger.warn(`Invalid custom hvigor data dir:${configured}`)
      return path.join(this.projectRoot, '.hvigor')
    }
    if (fs.existsSync(configured)) configured = fs.realpathSync.native(configured)
    return path.resolve(configured, path.basename(this.projectRoot), '.hvigor')
  }

  /**
   * Materialize the in-memory oh-package models and invoke OHPM at the same
   * post-nodesEvaluated boundary as
   * `OhpmLoadInstallService.ohpmLoadInstallForBuild` in Hvigor 6.24.3.
   * Original project manifests remain untouched; OHPM consumes the generated
   * dependency-map tree under the Hvigor cache directory.
   */
  prepareOhpmDependencyWorkspace() {
    if (!this._usesOhpm || this._isOnlyCleanCommand()) return

    const targetName = typeof this.config.buildTargetName === 'string' ? this.config.buildTargetName : ''
    const targetDir = path.resolve(this.hvigorCacheDir(), 'dependencyMap', targetName)
    const mapFile = path.join(targetDir, 'dependencyMap.json5')
    const hadDependencyMap = fs.existsSync(mapFile)
    fs.mkdirSync(targetDir, { recursive: true })

    const parameterFile = this.parameterFileOverride()
    const projectPackage = this._clone(this._projectPackage) || {}
    projectPackage.parameterFile = parameterFile
    let changed = this._writeJsonIfChanged(path.join(targetDir, HvigorCommonConst.OH_PACKAGE_JSON), projectPackage)

    const dependencyMap = {}
    const modules = []
    const moduleNames = new Set()
    for (const descriptor of this._moduleDescriptors) {
      const name = descriptor.name
      this._validateDependencyMapModuleName(name)
      const node = Array.from(this._nodesByFile.values()).find(
        (candidate) => !candidate.isProject && this._pathIdentity(candidate.root) === this._pathIdentity(descriptor.root),
      )
      const modulePackage = this._clone(node?.modulePackage ?? descriptor.modulePackage) || {}
      const moduleDir = path.join(targetDir, name)
      fs.mkdirSync(moduleDir, { recursive: true })
      changed = this._writeJsonIfChanged(path.join(moduleDir, HvigorCommonConst.OH_PACKAGE_JSON), modulePackage) || changed
      dependencyMap[name] = `./${name}/${HvigorCommonConst.OH_PACKAGE_JSON}`
      modules.push({ name, srcPath: path.relative(mapFile, descriptor.root) })
      moduleNames.add(name)
    }

    for (const entry of fs.readdirSync(targetDir, { withFileTypes: true })) {
      if (!entry.isDirectory() || moduleNames.has(entry.name)) continue
      const redundant = path.join(targetDir, entry.name)
      fs.rmSync(redundant, { recursive: true, force: true })
      changed = true
    }

    const map = {
      basePath: mapFile,
      targetName: targetName || undefined,
      rootDependency: `./${HvigorCommonConst.OH_PACKAGE_JSON}`,
      dependencyMap,
      modules,
    }
    changed = this._writeJsonIfChanged(mapFile, map) || changed

    const enforce = this._parameter.getExtParamInternal('enforce-ohpm') === 'true'
    const moduleConfigurationChanged = hadDependencyMap && changed
    if (!this._ohpmPackageModelUpdated && !moduleConfigurationChanged && !enforce) return

    const configuredExecutable = process.env.ohpmBin
    const executable =
      typeof configuredExecutable === 'string' && configuredExecutable.length > 0 && fs.existsSync(configuredExecutable)
        ? configuredExecutable
        : 'ohpm'
    const probe = childProcess.spawnSync(executable, ['-v'], {
      cwd: this.projectRoot,
      stdio: 'ignore',
      windowsHide: true,
    })
    if (probe.error?.code === 'ENOENT') {
      this._warn(
        'OHPM_EXECUTABLE_NOT_FOUND',
        'need to configure ohpm environment in your system, please see official guide to configure ohpm environment',
      )
      return
    }
    if (probe.error) {
      throw this._error('OHPM_INSTALL_FAILED', `failed to execute ${executable}: ${probe.error.message}`, {
        cause: probe.error,
        executable,
      })
    }

    const args = ['i', '--all', '--target_path', targetDir]
    if (parameterFile) args.push(`--parameterFile=${parameterFile}`)
    const install = childProcess.spawnSync(executable, args, {
      cwd: this.projectRoot,
      stdio: 'inherit',
      windowsHide: true,
    })
    if (install.error || install.status !== 0) {
      throw this._error(
        'OHPM_INSTALL_FAILED',
        install.error
          ? `failed to execute ${executable}: ${install.error.message}`
          : `${executable} exited with status ${install.status}`,
        { cause: install.error, executable, status: install.status, args },
      )
    }
  }

  _isOnlyCleanCommand() {
    return this._commandEntryTasks.length === 1 && this._commandEntryTasks[0] === 'clean'
  }

  _writeJsonIfChanged(file, value) {
    const content = JSON.stringify(value)
    try {
      if (fs.readFileSync(file, 'utf8') === content) return false
    } catch (error) {
      if (error.code !== 'ENOENT') throw error
    }
    fs.writeFileSync(file, content)
    return true
  }

  _replaceStringParameters(value, properties) {
    if (typeof value !== 'string' || !properties || !value.includes('${')) return value
    return value.replace(/\$\{([^}]+)\}/g, (match, key) => {
      let current = properties
      for (const component of key.trim().split('.')) {
        if (current === null || typeof current !== 'object') return match
        current = current[component]
      }
      return current !== null && typeof current !== 'object' && current !== undefined ? String(current) : match
    })
  }

  _validateDependencyMapModuleName(name) {
    if (
      typeof name !== 'string' ||
      name.length === 0 ||
      name === '.' ||
      name === '..' ||
      name.includes('/') ||
      name.includes('\\')
    ) {
      throw this._error('INVALID_HVIGOR_MODULE_NAME', `module name cannot be materialized in dependencyMap: ${name}`)
    }
  }

  recordProjectHvigorOverrides(overrides) {
    this._projectHvigorOverrides = this._clone(overrides || {})
  }

  recordModuleHvigorOverrides(root, overrides) {
    const normalized = path.resolve(root)
    if (overrides === undefined) {
      this._moduleHvigorOverrides.delete(normalized)
    } else {
      this._moduleHvigorOverrides.set(normalized, this._clone(overrides))
    }
    this._invalidateRuntimePlan()
  }

  async writeSyncInfo() {
    const customModels = {}
    const customModelTaskInfo = {}
    for (const [modelId, bean] of this._toolingModelRegistry.getModelMap()) {
      if (typeof modelId !== 'string' || typeof bean?.buildModel !== 'function') {
        throw this._error(
          'INVALID_HVIGOR_TOOLING_MODEL',
          'ToolingModelBean must expose a string modelId and buildModel() method',
        )
      }
      customModels[modelId] = await bean.buildModel()
      customModelTaskInfo[modelId] = typeof bean.taskInfo === 'function' ? await bean.taskInfo() : []
    }

    const nodes = Array.from(this._nodesByFile.values())
    const projectNode = nodes.find((node) => node.isProject)
    const modules = nodes
      .filter((node) => !node.isProject && ['hap', 'hsp', 'har'].includes(node.kind))
      .map((node) => {
        const selectedTargets = this._targetEntriesFor(node)
        const buildTargets = selectedTargets.length > 0 ? selectedTargets : [{ name: 'default' }]
        return {
          name: node.name,
          root: node.root,
          kind: node.kind,
          belongProjectPath: node.descriptor?.belongProjectPath,
          moduleProfile: this._clone(node.moduleProfile || {}),
          selectedTargets: buildTargets.map((target) => ({
            name: target.name,
            buildOption: this._buildOptionFor(node, target),
          })),
          customTasks: this._syncCustomTasks(node),
        }
      })
    const configuredBuildDir =
      this._parameter.getExtParam('build-cache-dir') ?? this._parameter.getPropertyInternal('ohos.buildDir')
    const request = {
      cache: this._hvigorCacheStateRequest(),
      projectName: projectNode?.name || path.basename(this.projectRoot),
      selectedProductName: this._selectedProductName,
      mode: this._effectiveBuildMode(),
      configBuildVersion: this.config.buildVersion,
      parameterBuildVersion: this._parameter.getExtParam('buildVersion'),
      projectProfile: this._clone(this._projectProfile) || {},
      appJson: this._clone(this._appJson),
      projectHvigorOverrides: this._clone(this._projectHvigorOverrides) || {},
      properties: { ...this._parameter.getPropertiesInternal() },
      configuredBuildDir: typeof configuredBuildDir === 'string' ? configuredBuildDir : undefined,
      modules,
      selectedNodeNames: this._syncNodeNames ? Array.from(this._syncNodeNames) : undefined,
      projectCustomTasks: projectNode ? this._syncCustomTasks(projectNode) : [],
      customModels,
      customModelTaskInfo,
      syncVersionChanged: this._syncVersionChanged,
    }
    await nativeRuntimeBinding.writeHvigorSyncModel(JSON.stringify(request))
  }

  async writeBuildMeta() {
    await nativeRuntimeBinding.writeHvigorBuildMeta(JSON.stringify(this._hvigorCacheStateRequest()))
    this._syncVersionChanged = false
  }

  _readSyncVersionChanged() {
    return nativeRuntimeBinding.hvigorSyncVersionChanged(JSON.stringify(this._hvigorCacheStateRequest()))
  }

  _hvigorCacheStateRequest() {
    const configuredCacheDir =
      this._parameter.getExtParam('build-cache-dir') ?? this._parameter.getPropertyInternal('hvigor.cacheDir')
    return {
      projectRoot: this.projectRoot,
      configuredCacheDir: typeof configuredCacheDir === 'string' ? configuredCacheDir : undefined,
      sdkPath: typeof this.config.sdkPath === 'string' ? this.config.sdkPath : undefined,
      compileSdkVersion:
        typeof this.config.compileSdkVersion === 'string' ? this.config.compileSdkVersion : undefined,
    }
  }

  _syncCustomTasks(node) {
    const tasks = Array.from(node.tasks.values())
      .filter((task) => !task.builtIn)
      .map((task) => ({
        taskName: task.definition.name,
        group: task.definition.group || 'Customize',
        description: task.definition.description || '',
        sourcePluginId: task.definition.sourcePluginId || '',
      }))
    for (const [taskName, creation] of node.lazyTaskCreations) {
      if (node.tasks.has(taskName)) continue
      tasks.push({
        taskName,
        group: creation.details?.group || 'Customize',
        description: creation.details?.description || '',
        sourcePluginId: creation.details?.sourcePluginId || '',
      })
    }
    return tasks
  }

  _replaceProjectRuntimeValue(key, value) {
    this._runtimeProjectOverrides[key] = this._clone(value)
    if (key === 'package') this._ohpmPackageModelUpdated = true
    this._contextInspection = undefined
    if (key === 'buildProfile') this._invalidateRuntimePlan()
  }

  _replaceModuleRuntimeValue(node, key, value) {
    const root = path.resolve(node.root)
    const current = this._runtimeModuleOverrides.get(root) || {}
    current[key] = this._clone(value)
    this._runtimeModuleOverrides.set(root, current)
    if (key === 'package') this._ohpmPackageModelUpdated = true
    this._contextInspection = undefined
    if (key === 'buildProfile') this._invalidateRuntimePlan()
  }

  _newEffects() {
    return {
      compilePluginPath: [],
      hvigorPluginFile: [],
      target: null,
      buildOption: {},
    }
  }

  moduleRoots() {
    const declared = this._moduleDescriptors.map((module) => module.root)
    return declared.length > 0 ? declared : [...this._fallbackModuleRoots]
  }

  async evaluateHvigorConfig() {
    if (this._buildConfigFile) {
      this._activeFile = this._buildConfigFile
      if (this._buildConfigFile.endsWith('.ts')) this._hvigorTypeScriptRoots.add(this._buildConfigFile)
      await loadModule(this._buildConfigFile, this)
    }
    for (const callback of this._configEvaluatedCallbacks.splice(0)) {
      await callback(this._hvigorConfig)
    }
    this._hvigorConfig.applyToHost()
  }

  async runNodesInitialized() {
    const callbacks = this._nodesInitializedCallbacks.splice(0)
    const hvigor = this._makeHvigorMock(this._activeFile).hvigor
    for (const callback of callbacks) {
      await callback(hvigor)
    }
  }

  async runTaskGraphResolved() {
    const callbacks = this._taskGraphResolvedCallbacks.splice(0)
    const hvigor = this._makeHvigorMock(this._activeFile).hvigor
    for (const callback of callbacks) {
      await callback(hvigor)
    }
  }

  commandPlan() {
    const allNodes = Array.from(this._nodesByFile.values())
    const request = {
      projectName: path.basename(this.projectRoot),
      executionMode: this._effectiveExecutionMode(),
      sync: Boolean(this.config.sync),
      syncVersionChanged: this._syncVersionChanged,
      taskNames: this._commandEntryTasks,
      moduleSelections: this._moduleSelections(),
      buildTargetName: this.config.buildTargetName,
      nodes: allNodes.map((node) => ({
        name: node.name,
        kind: node.kind,
        projectTask: node.isProject,
        childNames: node.children.map((child) => child.name),
        customSyncTaskPaths: Array.from(node.tasks.values())
          .filter((task) => !task.builtIn && task.definition.__arkdownSyncTask)
          .map((task) => task.path),
      })),
      catalogs: this._runtimeTaskCatalogRequests(),
      removedTaskPaths: this._removedRuntimeTaskPaths(),
      tasks: this._runtimeTaskOverlays(),
    }
    const response = JSON.parse(nativeRuntimeBinding.runtimeCommandPlan(JSON.stringify(request)))
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this._error(code, message, details)
    }
    const materializedTasks = new Map()
    const materialize = (taskPath) => {
      let task = materializedTasks.get(taskPath)
      if (!task) {
        task = this._commandTaskFromPath(taskPath)
        materializedTasks.set(taskPath, task)
      }
      return task
    }
    if (this.config.sync) {
      this._syncNodeNames = new Set(response.plan.selectedNodeNames)
    }
    return {
      tasks: response.plan.tasks.map(({ taskPath, ...entry }) => ({ ...entry, task: materialize(taskPath) })),
      nativeRequests: response.plan.nativeRequests.map(({ rootTaskPaths, ...nativeRequest }) => ({
        ...nativeRequest,
        rootTasks: rootTaskPaths.map(materialize),
      })),
    }
  }

  _commandTaskFromPath(taskPath) {
    const separator = taskPath.indexOf(':')
    const nodeName = separator < 0 ? taskPath : taskPath.slice(0, separator)
    const taskName = separator < 0 ? taskPath : taskPath.slice(separator + 1)
    const node = nodeName
      ? this._nodesByName.get(nodeName)
      : Array.from(this._nodesByFile.values()).find((candidate) => candidate.isProject)
    const task = node ? this._getTaskByName(node, taskName) : undefined
    if (!task) {
      throw this._error(
        'ARKDOWN_RUNTIME_TASK_NOT_FOUND',
        `Rust command plan references unknown runtime task ${JSON.stringify(taskPath)}`,
        { taskPath },
      )
    }
    return task
  }

  async executeCommandTask(entry, runRuntimeTasks) {
    const { task, commandKind } = entry
    let action
    if (commandKind === 'tasks') {
      action = () => this._runtimeTaskReport(task.node, 'tasks')
    } else if (commandKind === 'taskTree') {
      action = () => this._runtimeTaskReport(task.node, 'taskTree')
    } else if (commandKind === 'clean') {
      action = async () => {
        const configuredBuildDir =
          this._parameter.getExtParam('build-cache-dir') ?? this._parameter.getPropertyInternal('ohos.buildDir')
        const responseJson = await nativeRuntimeBinding.runHvigorClean(
          JSON.stringify({
            projectRoot: this.projectRoot,
            nodeRoot: task.node.root,
            projectTask: task.node.isProject,
            configuredBuildDir: typeof configuredBuildDir === 'string' ? configuredBuildDir : undefined,
            sdkPath: typeof this.config.sdkPath === 'string' ? this.config.sdkPath : undefined,
            projectProfile: this._projectProfile,
            targetNames: task.node.isProject
              ? []
              : this._targetEntriesFor(task.node).map((target) => target.name),
            buildMode: this._effectiveBuildMode(),
          }),
        )
        return this._decodeMaintenanceOutput(responseJson, 'clean')
      }
    } else if (commandKind === 'prune') {
      action = () => this.pruneHvigorCaches()
    } else if (commandKind === 'sync') {
      action = () => []
    } else if (commandKind === 'buildInfo') {
      action = () => {
        const responseJson = nativeRuntimeBinding.runHvigorBuildInfo(
          JSON.stringify({
            projectRoot: this.projectRoot,
            nodeName: task.node.name,
            projectTask: task.node.isProject,
            module: this._parameter.getExtParam('module'),
            json: this._parameter.getExtParam('json') === '',
            resolve: this._parameter.getExtParam('buildOption') === '',
          }),
        )
        return this._decodeMaintenanceOutput(responseJson, 'buildInfo')
      }
    } else if (commandKind === 'collectCoverage') {
      action = async () => {
        const message = 'Generate coverage statistics reports based on instrumentation dotting data'
        await this._generateCoverageReport(
          this._parameter.getExtParam('projectPath'),
          this._parameter.getExtParam('reportPath'),
          this._parameter.getExtParam('coverageFile'),
        )
        return [message]
      }
    } else {
      throw this._error('INVALID_ARKDOWN_COMMAND_TASK', `task ${task.path} is not a standalone command task`)
    }
    this._runtimeBuiltinActions.set(task.path, action)
    return this._executeRuntimeCommandTasks([task], runRuntimeTasks)
  }

  async pruneHvigorCaches(hvigorUserHome = getHvigorUserHome()) {
    const responseJson = await nativeRuntimeBinding.runHvigorPrune(JSON.stringify({ hvigorUserHome }))
    return this._decodeMaintenanceOutput(responseJson, 'prune')
  }

  async _generateCoverageReport(projectPath, reportPath, coverageFile) {
    try {
      const pluginEntry = this.hvigorWorkspaceNodeModules
        ? require.resolve('@ohos/hvigor-ohos-plugin', { paths: [this.hvigorWorkspaceNodeModules] })
        : require.resolve('@ohos/hvigor-ohos-plugin')
      const pluginRequire = Module.createRequire(pluginEntry)
      const compose = pluginRequire('@ohos/hvigor-arkts-compose')
      if (typeof compose.generateReport !== 'function') {
        throw new TypeError('@ohos/hvigor-arkts-compose does not export generateReport')
      }
      await compose.generateReport(projectPath, reportPath, coverageFile)
    } catch (cause) {
      throw this._error(
        'ARKDOWN_COVERAGE_RUNTIME_NOT_FOUND',
        `collectCoverage requires the Hvigor 6.24.3 coverage runtime: ${cause.message}`,
        { cause },
      )
    }
  }

  _decodeMaintenanceOutput(responseJson, operation) {
    let response
    try {
      response = JSON.parse(responseJson)
    } catch (cause) {
      throw this._error(
        'INVALID_ARKDOWN_MAINTENANCE_RESPONSE',
        `Rust ${operation} runner returned invalid JSON: ${cause.message}`,
        { cause, operation },
      )
    }
    if (!response || typeof response !== 'object') {
      throw this._error(
        'INVALID_ARKDOWN_MAINTENANCE_RESPONSE',
        `Rust ${operation} runner returned an invalid response`,
        { operation },
      )
    }
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this._error(code, message, details)
    }
    if (!Array.isArray(response.output)) {
      throw this._error(
        'INVALID_ARKDOWN_MAINTENANCE_RESPONSE',
        `Rust ${operation} runner returned no output array`,
        { operation },
      )
    }
    return response.output
  }

  async executeCustomCommandTasks(tasks, runRuntimeTasks) {
    if (tasks.length === 0) return
    await this._executeRuntimeCommandTasks(tasks, runRuntimeTasks)
  }

  async _executeRuntimeCommandTasks(tasks, runRuntimeTasks) {
    if (typeof runRuntimeTasks !== 'function') {
      throw this._error(
        'ARKDOWN_NATIVE_TASK_RUNTIME_UNAVAILABLE',
        'the Rust runtime task engine is required to execute hvigor command tasks',
      )
    }
    this._nativeSelectionRoots = [...tasks]
    this._nativeCommandRoots = [...tasks]
    this._nativeRequiredBoundaryTasks = []
    this._nativeExecutionNodeNames = new Set(tasks.map((task) => task.node.name))
    try {
      return await runRuntimeTasks(this.createNativeTaskBridge({ validateBoundaries: false }))
    } finally {
      this.clearNativeTaskSelection()
    }
  }

  prepareNativeCommandCycle(nodeNames, customRoots, rootTasks) {
    this._nativeExecutionNodeNames = new Set(nodeNames)
    this._nativeCommandRoots = [...customRoots]
    this._nativeSelectionRoots = [...rootTasks, ...customRoots]
    this._nativeRequiredBoundaryTasks = [...rootTasks]
  }

  prepareDirectBuildCycle() {
    const target = this.config.target || 'hap'
    const selectedModuleNames = new Set(this._moduleSelections().map((selection) => selection.moduleName))
    const roots = []
    for (const node of this._nodesByFile.values()) {
      if (target === 'app') {
        if (node.isProject && node.kind === 'app') roots.push(this._getTaskByName(node, 'assembleApp'))
        continue
      }
      if (node.isProject || node.kind !== target) continue
      if (selectedModuleNames.size > 0 && !selectedModuleNames.has(node.name)) continue
      const taskName = `assemble${target[0].toUpperCase()}${target.slice(1)}`
      roots.push(this._getTaskByName(node, taskName))
    }
    const selectedRoots = roots.filter(Boolean)
    this._nativeExecutionNodeNames = new Set(
      selectedRoots.flatMap((task) =>
        task.node.isProject ? [...task.node.children.map((child) => child.name), task.node.name] : [task.node.name],
      ),
    )
    this._nativeCommandRoots = []
    this._nativeSelectionRoots = selectedRoots
    this._nativeRequiredBoundaryTasks = []
    // Direct build() accepts third-party native executors that may only emit
    // boundaries carrying plugin work. Reachability still follows the full
    // assembly DAG, while command-mode partial tasks below require their
    // selected root boundary explicitly.
  }

  createNativeTaskBridge(options = {}) {
    const tasks = this._runtimeTaskOverlays()
    const selection = {
      projectRoot: this.projectRoot,
      cacheEnabled: Boolean(this._parameter.getCoreStartParams().incrementalExecution),
      expectedRootPaths: this._nativeSelectionRoots?.map((task) => task.path),
      requiredBoundaryPaths: this._nativeRequiredBoundaryTasks.map((task) => task.path),
      commandRootPaths: this._nativeCommandRoots.map((task) => task.path),
      executionNodeNames: Array.from(this._nativeExecutionNodeNames || []),
      validateBoundaries: options.validateBoundaries !== false,
    }
    return {
      tasksJson: JSON.stringify(tasks),
      catalogsJson: JSON.stringify(this._runtimeTaskCatalogRequests()),
      removedTaskPathsJson: JSON.stringify(this._removedRuntimeTaskPaths()),
      selectionJson: JSON.stringify(selection),
      commandModifierTypes: Array.from(this._commandBuilderModifiers)
        .filter(([, modifier]) => modifier)
        .map(([builderType]) => builderType),
      invoke: (operationJson) => this.invokeNativeTaskOperation(operationJson),
    }
  }

  clearNativeTaskSelection() {
    this._nativeSelectionRoots = undefined
    this._nativeRequiredBoundaryTasks = []
    this._nativeCommandRoots = []
    this._nativeExecutionNodeNames = undefined
    this._runtimeTasksByPath.clear()
    this._runtimeTaskFailures.clear()
    this._runtimeBuiltinActions.clear()
  }

  _runtimeTaskCatalogRequests() {
    return Array.from(this._nodesByFile.values(), (node) => node.taskCatalog.request())
  }

  _removedRuntimeTaskPaths() {
    const paths = []
    for (const node of this._nodesByFile.values()) {
      for (const taskName of node.deletedCatalogTasks) {
        paths.push(node.isProject ? `:${taskName}` : `${node.name}:${taskName}`)
      }
    }
    paths.sort()
    return paths
  }

  _runtimeTaskOverlays() {
    const descriptors = []
    this._runtimeTasksByPath.clear()
    for (const node of this._nodesByFile.values()) {
      let realizationOrder = 0
      for (const task of node.tasks.values()) {
        this._runtimeTasksByPath.set(task.path, task)
        descriptors.push(this._runtimeTaskDescriptor(task, realizationOrder))
        realizationOrder += 1
      }
      for (const [taskName, creation] of node.lazyTaskCreations) {
        if (node.tasks.has(taskName)) continue
        descriptors.push(this._runtimeLazyTaskDescriptor(node, creation, realizationOrder))
        realizationOrder += 1
      }
    }
    descriptors.sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0))
    return descriptors
  }

  _runtimeTaskReportDescriptors() {
    const descriptors = []
    this._runtimeTasksByPath.clear()
    for (const node of this._nodesByFile.values()) {
      let realizationOrder = 0
      for (const task of node.tasks.values()) {
        this._runtimeTasksByPath.set(task.path, task)
        descriptors.push(this._runtimeTaskDescriptor(task, realizationOrder))
        realizationOrder += 1
      }
      for (const [taskName, creation] of node.lazyTaskCreations) {
        if (node.tasks.has(taskName)) continue
        descriptors.push(this._runtimeLazyTaskDescriptor(node, creation, realizationOrder))
        realizationOrder += 1
      }
      for (const [taskName, definition] of node.taskCatalog.entries()) {
        if (!node.tasks.has(taskName)) {
          descriptors.push(this._runtimeBuiltinTaskDescriptor(node, definition, realizationOrder))
          realizationOrder += 1
        }
      }
    }
    descriptors.sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0))
    return descriptors
  }

  _runtimeTaskDescriptor(task, realizationOrder) {
    const catalogDefinition = task.node.taskCatalog.get(task.definition.name)
    return {
      path: task.path,
      nodeName: task.node.name,
      taskName: task.definition.name,
      builtIn: task.builtIn,
      projectTask: task.node.isProject,
      lazy: false,
      realized: true,
      group: task.builtIn
        ? (task.definition.group ?? catalogDefinition?.group ?? 'Other')
        : (task.definition.group ?? 'Customize'),
      description: task.definition.description ?? catalogDefinition?.description ?? '',
      catalogOrder: catalogDefinition?.catalogOrder,
      realizationOrder,
      enabled: task.enabled,
      trackState: task.trackState !== false,
      refreshDeclarations: Boolean(task.definition.__arkdownIncrementalTask),
      hasBeforeRun: task.beforeRunHooks.length > 0,
      hasAfterRun: task.afterRunHooks.length > 0,
      hasFailed: typeof task.taskContext?.onFailed === 'function',
      hasTaskShouldDo: typeof task.taskContext?.taskShouldDo === 'function',
      graphBoundCustom:
        !task.builtIn &&
        (toArray(task.definition.dependencies).length > 0 ||
          toArray(task.definition.postDependencies).length > 0),
      dependencies: [...task.dependencies],
      userDependencies: [...task.userDependencies],
      inputProperties: Array.from(task.input.properties, ([name, value]) => ({
        name: String(name),
        value: String(value),
        valueType: typeof value,
      })),
      inputFiles: [...task.input.files],
      outputFiles: [...task.output.files],
    }
  }

  _runtimeLazyTaskDescriptor(node, creation, realizationOrder) {
    const taskName = creation.details.name
    const dependencies = [...toArray(creation.depends), ...toArray(creation.dependsDefinedByUser)]
      .filter((reference) => typeof reference === 'string')
      .map((reference) => this._qualifiedTaskDependency(node, reference))
    return {
      path: node.isProject ? `:${taskName}` : `${node.name}:${taskName}`,
      nodeName: node.name,
      taskName,
      builtIn: false,
      projectTask: node.isProject,
      lazy: true,
      realized: false,
      group: creation.details.group || 'Customize',
      description: creation.details.description || '',
      realizationOrder,
      enabled: true,
      trackState: true,
      refreshDeclarations: false,
      hasBeforeRun: false,
      hasAfterRun: false,
      hasFailed: false,
      hasTaskShouldDo: false,
      graphBoundCustom: dependencies.length > 0,
      dependencies,
      userDependencies: [],
      inputProperties: [],
      inputFiles: [],
      outputFiles: [],
    }
  }

  _runtimeBuiltinTaskDescriptor(node, definition, realizationOrder) {
    return {
      path: node.isProject ? `:${definition.name}` : `${node.name}:${definition.name}`,
      nodeName: node.name,
      taskName: definition.name,
      builtIn: true,
      projectTask: node.isProject,
      lazy: false,
      realized: false,
      group: definition.group,
      description: definition.description,
      catalogOrder: definition.catalogOrder,
      realizationOrder,
      enabled: true,
      trackState: true,
      refreshDeclarations: false,
      hasBeforeRun: false,
      hasAfterRun: false,
      hasFailed: false,
      hasTaskShouldDo: false,
      graphBoundCustom: false,
      dependencies: [...definition.dependencies],
      userDependencies: [],
      inputProperties: [],
      inputFiles: [],
      outputFiles: [],
    }
  }

  async invokeNativeTaskOperation(operationJson) {
    const operation = typeof operationJson === 'string' ? JSON.parse(operationJson) : operationJson
    if (operation?.phase === 'transformAbc') {
      await this._runTransformAbcEvent(JSON.parse(operation.nativeEventJson))
      return '{}'
    }
    if (operation?.phase === 'modifyNativeCommand') {
      const command = this._runNativeCommandModifier(JSON.parse(operation.nativeEventJson))
      return JSON.stringify({ command })
    }
    const task = this._runtimeTaskForPath(operation?.taskPath)
    if (!task) {
      throw this._error(
        'ARKDOWN_RUNTIME_TASK_NOT_FOUND',
        `Rust requested unknown runtime task ${JSON.stringify(operation?.taskPath)}`,
      )
    }
    try {
      if (operation.phase === 'realize') {
        const descriptor = this._runtimeTaskDescriptor(task, 0)
        return JSON.stringify({
          enabled: descriptor.enabled,
          trackState: descriptor.trackState,
          refreshDeclarations: descriptor.refreshDeclarations,
          hasBeforeRun: descriptor.hasBeforeRun,
          hasAfterRun: descriptor.hasAfterRun,
          hasFailed: descriptor.hasFailed,
          hasTaskShouldDo: descriptor.hasTaskShouldDo,
          inputProperties: descriptor.inputProperties,
          inputFiles: descriptor.inputFiles,
          outputFiles: descriptor.outputFiles,
        })
      }
      if (operation.phase === 'beforeAlways') {
        await task.taskContext.beforeAlwaysAction()
        const declarations = collectHvigorIncrementalDeclarations(task.taskContext)
        task.input.properties = declarations.inputs
        task.input.files = Array.from(declarations.inputFiles.collect().keys())
        task.output.files = Array.from(declarations.outputFiles.collect().keys())
        return JSON.stringify({
          inputProperties: Array.from(task.input.properties, ([name, value]) => ({
            name: String(name),
            value: String(value),
            valueType: typeof value,
          })),
          inputFiles: [...task.input.files],
          outputFiles: [...task.output.files],
        })
      }
      if (operation.phase === 'taskShouldDo') {
        return JSON.stringify({ taskShouldDo: task.taskContext.taskShouldDo() !== false })
      }
      if (operation.phase === 'beforeRun') {
        await this._runTaskBeforeHooks(task)
        return '{}'
      }
      if (operation.phase === 'run') {
        if (task.builtIn) {
          const action = this._runtimeBuiltinActions.get(task.path)
          if (typeof action !== 'function') {
            throw this._error(
              'ARKDOWN_BUILTIN_TASK_ACTION_NOT_BOUND',
              `Rust requested an unbound standalone built-in task action for ${task.path}`,
            )
          }
          return JSON.stringify({ output: toArray(await action()) })
        }
        if (typeof task.definition.run !== 'function') {
          throw this._error('INVALID_HVIGOR_TASK_RUN', `task ${task.key} must contain a run function`)
        }
        await task.definition.run.apply(task.taskContext, [task.runContext])
        await this._awaitRuntimeTaskPendingPromises(task)
        return '{}'
      }
      if (operation.phase === 'afterRun') {
        await this._runTaskAfterHooks(task)
        return '{}'
      }
      if (operation.phase === 'failed') {
        const cause = this._runtimeTaskFailures.get(task.path) || new Error(operation.error || 'task failed')
        this._runtimeTaskFailures.delete(task.path)
        task.taskContext.onFailed(cause)
        return '{}'
      }
      throw this._error(
        'INVALID_ARKDOWN_RUNTIME_TASK_OPERATION',
        `invalid Rust runtime task operation: ${JSON.stringify(operation)}`,
      )
    } catch (error) {
      if (operation.phase !== 'failed') this._runtimeTaskFailures.set(task.path, error)
      throw error
    }
  }

  _runtimeTaskForPath(taskPath) {
    const existing = this._runtimeTasksByPath.get(taskPath)
    if (existing || typeof taskPath !== 'string') return existing
    const separator = taskPath.indexOf(':')
    if (separator < 0) return undefined
    const nodeName = taskPath.slice(0, separator)
    const taskName = taskPath.slice(separator + 1)
    const node = nodeName
      ? this._nodesByName.get(nodeName)
      : Array.from(this._nodesByFile.values()).find((candidate) => candidate.isProject)
    const task = node ? this._getTaskByName(node, taskName) : undefined
    if (task) this._runtimeTasksByPath.set(task.path, task)
    return task
  }

  async _awaitRuntimeTaskPendingPromises(task) {
    const pendingPromises = task.taskContext?.pendingPromises
    if (!pendingPromises || typeof pendingPromises.get !== 'function') return
    const pending = toArray(pendingPromises.get())
    pendingPromises.clear?.()
    await Promise.all(pending)
  }

  _runtimeTaskReport(node, reportKind) {
    return [
      nativeRuntimeBinding.runtimeTaskReport(
        JSON.stringify({
          nodeName: node.name,
          projectTask: node.isProject,
          kind: node.kind,
          reportKind,
          tasks: this._runtimeTaskReportDescriptors(),
        }),
      ),
    ]
  }

  async runBuildFinished(error, report = null) {
    const callbacks = this._buildFinishedCallbacks.splice(0)
    const result = new HvigorBuildResult(error, report)
    try {
      for (const callback of callbacks) {
        await callback(result)
      }
    } finally {
      await this._workerPool.shutdown()
    }
  }

  async runWatchWorkerMessage(message) {
    for (const callback of this._watchWorkerMessageCallbacks) {
      await callback(message)
    }
  }

  _loadRuntimeModel() {
    let runtimeModel
    try {
      runtimeModel = JSON.parse(
        nativeRuntimeBinding.loadBuildScriptRuntimeModel(
          JSON.stringify({ projectRoot: this.projectRoot, hvigorUserHome: getHvigorUserHome() }),
        ),
      )
    } catch (cause) {
      throw this._error(
        'INVALID_HVIGOR_MODEL_FILE',
        `failed to load the Rust build-script runtime model for ${this.projectRoot}: ${cause.message}`,
        { cause },
      )
    }
    this._throwBuildScriptDiagnostic(runtimeModel.diagnostic)
    return runtimeModel
  }

  _runtimeRootKey(root) {
    const resolved = path.resolve(root)
    return process.platform === 'win32' ? resolved.toLowerCase() : resolved
  }

  _descriptorFromRuntimeModel(model, overlay = {}) {
    const root = path.resolve(model.root)
    this._runtimeModuleModelsByRoot.set(this._runtimeRootKey(root), model)
    const descriptor = { ...(model.descriptor || {}) }
    for (const [key, value] of Object.entries(overlay)) {
      if (value !== undefined) descriptor[key] = value
    }
    return {
      ...descriptor,
      name: model.name,
      srcPath: model.srcPath,
      root,
      moduleProfile: model.buildProfile || {},
      moduleJson: model.moduleJson,
      modulePackage: this._usesOhpm ? model.package || {} : model.npmPackage || {},
    }
  }

  _loadModuleDescriptor(descriptor) {
    const root = path.resolve(
      descriptor.root || path.resolve(this.projectRoot, descriptor.srcPath || descriptor.name || ''),
    )
    const key = this._runtimeRootKey(root)
    let model = this._runtimeModuleModelsByRoot.get(key)
    if (!model) {
      try {
        model = JSON.parse(
          nativeRuntimeBinding.loadBuildScriptModuleModel(
            JSON.stringify({ projectRoot: this.projectRoot, descriptor: { ...descriptor, root } }),
          ),
        )
      } catch (cause) {
        throw this._error(
          'INVALID_HVIGOR_MODEL_FILE',
          `failed to load the Rust module runtime model for ${root}: ${cause.message}`,
          { cause },
        )
      }
      this._throwBuildScriptDiagnostic(model.diagnostic)
    }
    return this._descriptorFromRuntimeModel(model, descriptor)
  }

  discoverBuildScriptFiles(roots) {
    try {
      return JSON.parse(
        nativeRuntimeBinding.discoverBuildScriptFiles(JSON.stringify({ roots: toArray(roots) })),
      )
    } catch (cause) {
      throw this._error(
        'INVALID_HVIGOR_MODEL_FILE',
        `failed to discover build scripts in Rust: ${cause.message}`,
        { cause },
      )
    }
  }

  hvigorPluginWorkspaceRequest() {
    if (!this._projectHvigorConfigPath) return null
    return {
      projectRoot: this.projectRoot,
      hvigorUserHome: getHvigorUserHome(),
      homeDir: os.homedir(),
      processExecutable: process.execPath,
      nodeVersion: process.version,
      projectConfig: this._clone(this._hvigorFileConfig),
      userConfig: this._clone(this._userHvigorConfig),
    }
  }

  validateBuildScriptBuildOption(buildOption, sourceFile, fieldPath) {
    let response
    try {
      response = JSON.parse(
        nativeRuntimeBinding.validateBuildScriptBuildOption(
          JSON.stringify({ buildOption, sourceFile, fieldPath }),
        ),
      )
    } catch (cause) {
      throw this._error(
        'INVALID_HVIGOR_MODEL_FILE',
        `failed to validate the Rust build-script schema for ${sourceFile}: ${cause.message}`,
        { cause },
      )
    }
    this._throwBuildScriptDiagnostic(response.diagnostic)
  }

  _throwBuildScriptDiagnostic(diagnostic) {
    if (!diagnostic) return
    const { code, message, ...details } = diagnostic
    throw this._error(code, message, details)
  }

  _clone(value) {
    if (value === undefined) {
      return undefined
    }
    return JSON.parse(JSON.stringify(value))
  }

  _deepFreeze(value) {
    if (!value || typeof value !== 'object' || Object.isFrozen(value)) {
      return value
    }
    for (const child of Object.values(value)) {
      this._deepFreeze(child)
    }
    return Object.freeze(value)
  }

  _freezeClone(value) {
    return this._deepFreeze(this._clone(value))
  }

  _makeParameter() {
    const userHvigorConfig = this._userHvigorConfig
    const properties = {
      enableSignTask: true,
      skipNativeIncremental: false,
      'hvigor.keepDependency': true,
      ...(userHvigorConfig.properties && typeof userHvigorConfig.properties === 'object'
        ? userHvigorConfig.properties
        : {}),
      ...(this._hvigorFileConfig.properties && typeof this._hvigorFileConfig.properties === 'object'
        ? this._hvigorFileConfig.properties
        : {}),
      ...(this.config.hvigorProperties && typeof this.config.hvigorProperties === 'object'
        ? this.config.hvigorProperties
        : {}),
    }
    const extParams = {
      ...(this.config.extParams && typeof this.config.extParams === 'object' ? this.config.extParams : {}),
    }
    for (const entry of toArray(this.config.prop)) {
      if (typeof entry !== 'string') continue
      const separator = entry.indexOf('=')
      const key = separator < 0 ? entry : entry.slice(0, separator)
      const value = separator < 0 ? undefined : entry.slice(separator + 1)
      extParams[key === 'coverage' ? 'ohos-test-coverage' : key] = value
      if (key.startsWith('properties.')) {
        properties[key.slice('properties.'.length)] = this._configPropertyValue(value)
      }
    }
    for (const entry of toArray(this.config.configOverrides)) {
      if (typeof entry !== 'string') continue
      const separator = entry.indexOf('=')
      if (separator < 0) continue
      const key = entry.slice(0, separator)
      if (key.startsWith('properties.')) {
        properties[key.slice('properties.'.length)] = this._configPropertyValue(entry.slice(separator + 1))
      }
    }
    this._applyCommandDefaultExtParams(extParams)
    const execution = this._hvigorFileConfig.execution || {}
    const debugging = this._hvigorFileConfig.debugging || {}
    const logging = this._hvigorFileConfig.logging || {}
    const incrementalExecution = Boolean(this.config.cache ?? execution.incremental ?? true)
    const parallelExecution = Boolean(this.config.parallel ?? execution.parallel ?? true)
    const hvigorfileTypeCheck = Boolean(
      this.config.enableBuildScriptTypeCheck ?? this.config.typeCheck ?? execution.typeCheck ?? false,
    )
    const printStackTrace = Boolean(this.config.stacktrace ?? debugging.stacktrace ?? false)
    const daemon = Boolean(this.config.daemon ?? execution.daemon ?? true)
    const analyze = this.config.analyze ?? execution.analyze ?? 'normal'
    const optimizationStrategy = this.config.optimizationStrategy ?? execution.optimizationStrategy ?? 'memory'
    const logLevel = this.config.logLevel ?? logging.level ?? 'info'
    const coreStartParams = {
      analyze: HVIGOR_ANALYZE_MODE_MAP.get(analyze) ?? analyze,
      daemon,
      parallelExecution,
      incrementalExecution,
      hvigorfileTypeCheck,
      printStackTrace,
      logLevel,
      optimizationStrategy,
      hotCompile: Boolean(this.config.hotCompile),
      hotReloadBuild: Boolean(this.config.hotReloadBuild),
      customizedParam:
        this.config.customizedParam && typeof this.config.customizedParam === 'object'
          ? this._clone(this.config.customizedParam)
          : undefined,
    }
    let workspaceDir = process.env.WORKSPACE_DIR || this.projectRoot
    const externalAnalyze = () =>
      new Map([
        [0, 'normal'],
        [1, 'advanced'],
        [2, false],
        [3, 'ultrafine'],
      ]).get(coreStartParams.analyze)
    const externalStartParams = () =>
      this._freezeClone({
        daemon: coreStartParams.daemon,
        analyze: externalAnalyze(),
        parallel: coreStartParams.parallelExecution,
        incremental: coreStartParams.incrementalExecution,
        logLevel: String(coreStartParams.logLevel?.levelStr || coreStartParams.logLevel).toLowerCase(),
        typeCheck: coreStartParams.hvigorfileTypeCheck,
        optimizationStrategy: coreStartParams.optimizationStrategy,
        hotCompile: coreStartParams.hotCompile,
        hotReloadBuild: coreStartParams.hotReloadBuild,
      })
    const replaceRecord = (target, value) => {
      for (const key of Object.keys(target)) delete target[key]
      if (value && typeof value === 'object') Object.assign(target, value)
    }
    return {
      getProperty: (key) => properties[key],
      getPropertyInternal: (key) => properties[key],
      setProperty: (key, value) => {
        properties[key] = value
      },
      setPropertyInternal: (key, value) => {
        properties[key] = value
      },
      getProperties: () => this._freezeClone(properties),
      getPropertiesInternal: () => this._freezeClone(properties),
      replacePropertiesInternal: (value) => replaceRecord(properties, value),
      getExtParam: (key) => extParams[key],
      getExtParamInternal: (key) => extParams[key],
      getExtParams: () => this._freezeClone(extParams),
      getExtParamsInternal: () => this._freezeClone(extParams),
      replaceExtParamsInternal: (value) => {
        replaceRecord(extParams, value)
        this._invalidateRuntimePlan()
      },
      getStartParams: externalStartParams,
      getStartParamsInternal: externalStartParams,
      getCoreStartParams: () => coreStartParams,
      replaceCoreStartParams: (value) => replaceRecord(coreStartParams, value),
      getWorkspaceDir: () => this._freezeClone(workspaceDir),
      getWorkspaceDirInternal: () => this._freezeClone(workspaceDir),
      setWorkspaceDirInternal: (value) => {
        workspaceDir = value
      },
    }
  }

  _applyCommandDefaultExtParams(extParams) {
    // Hvigor 6.24.3 `global-data.js::{setTestDefaultParameters,
    // setDeviceTestDefaultParameters}` fills only absent `-p` values. Keep the
    // same ordering: explicit extParams/prop values win over command defaults.
    const defaults = this._commandEntryTasks.includes('test')
      ? {
          pageType: 'page',
          product: 'default',
          buildRoot: '.test',
          unitTestMode: 'true',
          isLocalTest: 'true',
          'ohos-test-coverage': 'true',
          'unit.test.replace.page': '../../../.test/testability/pages/Index',
        }
      : this._commandEntryTasks.includes('onDeviceTest')
        ? {
            product: 'default',
            buildMode: 'test',
            isOhosTest: 'true',
            'ohos-test-coverage': 'true',
          }
        : undefined
    if (!defaults) return
    for (const [key, value] of Object.entries(defaults)) {
      if (!Object.hasOwn(extParams, key)) extParams[key] = value
    }
  }

  _effectiveBuildMode() {
    return this._parameter?.getExtParam('buildMode') || this.config.mode || this.config.buildMode || 'debug'
  }

  _effectiveExecutionMode() {
    if (this.config.executionMode) return this.config.executionMode
    return this._commandEntryTasks.some((task) => task === 'test' || task === 'onDeviceTest')
      ? 'module'
      : undefined
  }

  _configPropertyValue(value) {
    if (typeof value !== 'string') return value
    if (value === 'true') return true
    if (value === 'false') return false
    const number = Number(value)
    return value.length <= 16 && !Number.isNaN(number) ? number : value.trim()
  }

  _error(code, message, extra = {}) {
    const error = new Error(message, extra.cause ? { cause: extra.cause } : undefined)
    error.code = code
    for (const [key, value] of Object.entries(extra)) {
      if (key !== 'cause') {
        error[key] = value
      }
    }
    return error
  }

  _warn(code, message) {
    emitWarningDiagnostic({ code, message })
  }

  prepareHvigorfiles(files) {
    for (const file of files) {
      if (file.endsWith('.ts')) this._hvigorTypeScriptRoots.add(file)
    }
    for (const file of files) {
      this._nodeForFile(file, true)
    }
    const projectNode =
      this._nodesByFile.get(this._normalizeFile(path.join(this.projectRoot, 'hvigorfile.ts'))) ||
      Array.from(this._nodesByFile.values()).find((node) => node.root === this.projectRoot)
    if (projectNode) {
      for (const node of this._nodesByFile.values()) {
        if (node !== projectNode) {
          node.parent = projectNode
          projectNode.children.push(node)
        }
      }
      // hvigor-core initRootProject runs after the complete project/module
      // hierarchy exists. Plugins can then query either the core nodes or the
      // public HvigorNode wrappers without forcing another filesystem scan.
      this._hvigorCore.initRootProject(projectNode.coreWrapper)
    }
  }

  async beginHvigorfile(file) {
    const node = this._nodeForFile(file, true)
    this._activeFile = node.file
    this._effects = node.effects
    for (const callback of this._beforeNodeEvaluateCallbacks) {
      await callback(node.wrapper)
    }
    for (const callback of node.beforeNodeEvaluateCallbacks.splice(0)) {
      await callback.call(node.wrapper, node.wrapper)
    }
    return node.wrapper
  }

  effectsFor(file) {
    return this._nodeForFile(file, true).effects
  }

  bindHvigorfile(file, value) {
    const node = this._nodeForFile(file, true)
    node.loaderConfig = value && typeof value === 'object' ? value.config : undefined
    node.exportValue = value
    const system = value && typeof value === 'object' && value.system ? value.system : value
    for (const plugin of this._systemPluginsFromExport(system)) {
      this._bindSystemPlugin(node, plugin)
    }
  }

  _systemPluginsFromExport(system) {
    if (system?.__arkdownHvigorSystemKind) {
      return [system]
    }
    if (!system || (typeof system !== 'object' && typeof system !== 'function')) {
      return []
    }
    // hvigor 6.24.3 configuration.bindSystemPlugins() iterates every named
    // export when the hvigorfile has no default `{ system }` declaration.
    // This is the shape produced by `export { hapTasks } from ...`.
    return Object.keys(system)
      .map((name) => system[name])
      .filter((plugin) => plugin?.__arkdownHvigorSystemKind)
  }

  _bindSystemPlugin(node, system) {
    const kind = system.__arkdownHvigorSystemKind
    node.kind = kind
    node.taskCatalog.invalidate()
    node.parent?.taskCatalog.invalidate()
    const pluginId = this._pluginIdForKind(kind)
    const systemPlugin = {
      getPluginId: () => pluginId,
      getContext: () => this._ohosContextFor(node),
    }
    node.systemPlugins.set(pluginId, systemPlugin)
    node.contextFactories.set(pluginId, systemPlugin.getContext)
    node.pluginIds.push(pluginId)
  }

  async applyCustomPlugins(file, plugins) {
    const node = this._nodeForFile(file, true)
    if (!plugins || (Array.isArray(plugins) && plugins.length === 0)) {
      return
    }
    if (!Array.isArray(plugins)) {
      this._warn('INVALID_HVIGOR_CUSTOM_PLUGINS', `invalid custom plugins config in ${file}; expected an array`)
      return
    }

    const validPlugins = []
    for (let index = 0; index < plugins.length; index++) {
      const plugin = plugins[index]
      if (!plugin || typeof plugin !== 'object' || typeof plugin.apply !== 'function') {
        this._warn('INVALID_HVIGOR_CUSTOM_PLUGIN', `plugins[${index}] in ${file} must contain an apply(node) function`)
        continue
      }
      if (typeof plugin.pluginId !== 'string') {
        this._warn('INVALID_HVIGOR_CUSTOM_PLUGIN_ID', `plugins[${index}] in ${file} must contain a string pluginId`)
      }
      if (plugin.context) {
        // hvigor 6.24.3 binds context before any plugin apply callback runs.
        // Calling `.bind` intentionally preserves hvigor's failure for a
        // truthy non-function context declaration.
        node.contextFactories.set(plugin.pluginId, plugin.context.bind(plugin))
        node.pluginIds.push(plugin.pluginId)
      }
      validPlugins.push(plugin)
    }

    for (const plugin of validPlugins) {
      try {
        await plugin.apply(node.wrapper)
      } catch (cause) {
        throw this._error(
          'HVIGOR_CUSTOM_PLUGIN_APPLY_FAILED',
          `custom plugin ${JSON.stringify(plugin.pluginId)} failed while applying to ${node.name}: ${cause.message}`,
          { cause, pluginId: plugin.pluginId, hvigorfile: file },
        )
      }
    }
  }

  async runAfterNodeEvaluate(file) {
    const node = this._nodeForFile(file, true)
    for (const callback of this._afterNodeEvaluateCallbacks) {
      await callback(node.wrapper)
    }
    for (const callback of node.afterNodeEvaluateCallbacks.splice(0)) {
      await callback.call(node.wrapper, node.wrapper)
    }
  }

  async runNodesEvaluated() {
    const callbacks = this._nodesEvaluatedCallbacks.splice(0)
    const hvigor = this._makeHvigorMock(this._activeFile).hvigor
    for (const callback of callbacks) {
      await callback(hvigor)
    }
  }

  _normalizeFile(file) {
    const resolved = path.resolve(file)
    for (const extension of ['.ts', '.mjs', '.cjs', '.js']) {
      if (resolved.endsWith(extension)) {
        return resolved.slice(0, -extension.length)
      }
    }
    return resolved
  }

  _nodeForFile(file, create = false) {
    const normalized = this._normalizeFile(file)
    let node = this._nodesByFile.get(normalized) || this._nodeAliasesByFile.get(normalized)
    if (node || !create) {
      return node
    }
    const actualFile = path.resolve(file)
    const root = path.dirname(actualFile)
    const isProject = root === this.projectRoot
    const descriptor = isProject
      ? undefined
      : this._moduleDescriptors.find(
          (module) => this._runtimeRootKey(module.root) === this._runtimeRootKey(root),
        ) ||
        this._loadModuleDescriptor({
          name: path.basename(root),
          srcPath: path.relative(this.projectRoot, root),
          root,
        })
    const name = descriptor?.name || (isProject ? path.basename(this.projectRoot) : path.basename(root))
    node = {
      file: actualFile,
      root,
      name,
      isProject,
      descriptor,
      kind: null,
      parent: null,
      children: [],
      effects: this._newEffects(),
      contextFactories: new Map(),
      systemPlugins: new Map(),
      pluginIds: [],
      tasks: new Map(),
      lazyTaskCreations: new Map(),
      catalogTaskCreations: new Map(),
      deletedCatalogTasks: new Set(),
      taskGraph: new HvigorTaskDirectedAcyclicGraph(),
      taskMap: new Map(),
      externalSubModules: new Map(),
      transformAbcActions: new Map(),
      extraOptions: new Map(Object.entries(descriptor?.extraOptions || {})),
      beforeNodeEvaluateCallbacks: [],
      afterNodeEvaluateCallbacks: [],
      afterBindSystemPluginsCallbacks: [],
      loaderConfig: undefined,
      exportValue: undefined,
      moduleProfile: isProject ? this._projectProfile : descriptor.moduleProfile || {},
      moduleJson: isProject ? undefined : descriptor.moduleJson,
      modulePackage: isProject ? this._projectPackage : descriptor.modulePackage || {},
    }
    node.taskCatalog = new HvigorTaskCatalog(this, node)
    node.taskContainer = new HvigorTaskContainerView(this, node)
    node.coreWrapper = this._makeCoreNodeWrapper(node)
    node.wrapper = this._makeNodeWrapper(node)
    this._initializeCoreTasks(node)
    this._nodesByFile.set(normalized, node)
    this._nodesByName.set(name, node)
    this._invalidateRuntimePlan()
    return node
  }

  _aliasBuildScriptFile(alias, source) {
    const node = this._nodeForFile(source, false)
    if (node) {
      this._nodeAliasesByFile.set(this._normalizeFile(alias), node)
    }
  }

  _makeCoreNodeWrapper(node) {
    const host = this
    const wrapper = {
      classKind: node.isProject ? 'project' : 'module',
      getName: () => node.name,
      getBuildFilePath: () => host._normalizeFile(node.file),
      getBuildProfilePath: () => path.join(node.root, HvigorCommonConst.BUILD_PROFILE_FILE),
      getNodeDir: () => node.root,
      getPackageJsonPath: () =>
        path.join(
          node.root,
          host._usesOhpm ? HvigorCommonConst.OH_PACKAGE_JSON : HvigorCommonConst.PACKAGE_JSON,
        ),
      findModuleByName: (name) => {
        const project = node.isProject ? node : node.parent
        return (
          project?.children.find((child) => child.name === name)?.coreWrapper ||
          project?.externalSubModules.get(name)
        )
      },
      bindPlugin: (plugin) => {
        const pluginId = plugin?.getPluginId?.()
        if (typeof pluginId !== 'string' || pluginId.length === 0) {
          throw host._error('REGISTRY_PLUGIN_ERROR', 'Hvigor system plugin must expose a non-empty plugin id')
        }
        if (node.systemPlugins.has(pluginId)) {
          throw host._error('REGISTRY_PLUGIN_ERROR', `plugin ${pluginId} is already bound to ${node.name}`)
        }
        node.systemPlugins.set(pluginId, plugin)
        return plugin
      },
      bindPluginContextFunc: (pluginId, factory) => {
        node.contextFactories.set(pluginId, factory)
        node.pluginIds.push(pluginId)
      },
      getPluginById: (pluginId) => node.systemPlugins.get(pluginId),
      getContext: (pluginId) => {
        const factory = node.contextFactories.get(pluginId)
        return typeof factory === 'function' ? factory() : undefined
      },
      getAllPluginIds: () => node.pluginIds,
      getProject: () => (node.isProject ? node : node.parent)?.coreWrapper,
      loadConfig: (config) => {
        node.loaderConfig = config
      },
      getConfigOpt: () => host._makeConfig(node),
      addExtraOption: (key, value) => node.extraOptions.set(key, value),
      getExtraOption: (key) => node.extraOptions.get(key),
      getTaskPaths: () =>
        host
          ._allTaskNames(node)
          .map((taskName) => (node.isProject ? `:${taskName}` : `${node.name}:${taskName}`)),
      getTaskDepends: (taskName) => {
        const separator = taskName.lastIndexOf(':')
        const localName = separator >= 0 ? taskName.slice(separator + 1) : taskName
        return node.taskContainer.getTaskDepends(localName)
      },
      getAllTasks: () => node.taskContainer.getAllTasks(),
      getAllTasksCA: () => node.taskContainer.getAllTasksCA(),
      getTaskByName: (name) => node.taskContainer.findTask(name),
      getTaskCAByName: (name) => node.taskContainer.getLazyTask(name),
      task: (action, taskInfo) => {
        const task = new HvigorDefaultTask(wrapper, taskInfo)
        task.registryAction = () => (typeof action === 'function' ? action : hvigorNoop)
        host._registerTask(node, task)
        return task
      },
      registry: (task) => {
        host._registerTask(node, task)
        return task
      },
      registerTask: (task) => host._registerTask(node, task),
      getTaskContext: () => ({ moduleName: node.name, modulePath: node.root }),
      hasTask: (name) => host._hasTask(node, name),
      registryDependsOnTask: (originalTask, ...dependsOnTasks) => {
        node.taskMap.set(originalTask, dependsOnTasks)
        for (const dependency of dependsOnTasks) originalTask.dependsOn(dependency)
      },
      getTaskContainer: () => node.taskContainer,
      getTaskGraph: () => node.taskGraph,
      clearTaskGraph: () => node.taskGraph.clear(),
      beforeNodeEvaluate: (callback) =>
        node.beforeNodeEvaluateCallbacks.push({ callback, thisArg: wrapper }),
      afterNodeEvaluate: (callback) =>
        node.afterNodeEvaluateCallbacks.push({ callback, thisArg: wrapper }),
      executeNodeHook: (hookType) => host._executeNodeHook(node, hookType),
      afterBindSystemPlugins: (callback) =>
        node.afterBindSystemPluginsCallbacks.push({ callback, thisArg: wrapper }),
      executeAfterBindSystemPluginsHook: () => host._executeAfterBindSystemPluginsHooks(node),
    }
    if (node.isProject) {
      wrapper.getSubModules = () => {
        const modules = new Map(node.children.map((child) => [child.name, child.coreWrapper]))
        for (const [name, module] of node.externalSubModules) modules.set(name, module)
        return modules
      }
      wrapper.addSubModule = (module) => {
        const name = module?.getName?.()
        if (typeof name !== 'string' || name.length === 0) {
          throw host._error('INVALID_HVIGOR_MODULE_NAME', 'Hvigor module must expose a non-empty name')
        }
        const internalModule = module.__arkdownNode
        if (internalModule) {
          internalModule.parent = node
          if (!node.children.includes(internalModule)) node.children.push(internalModule)
        } else {
          node.externalSubModules.set(name, module)
        }
      }
      wrapper.getAllSubModules = () => Array.from(wrapper.getSubModules().values())
    }
    Object.defineProperties(wrapper, {
      __arkdownHost: { value: host },
      __arkdownNode: { value: node },
    })
    return wrapper
  }

  _makeNodeWrapper(node) {
    const normalizedFile = new HvigorNormalizedFile(node.root)
    const wrapper = {
      nodeDir: normalizedFile,
      classKind: node.isProject ? 'project' : 'module',
      getNodeDir: () => normalizedFile,
      getNodeName: () => node.name,
      getNodeNameInternal: () => node.name,
      getNodePath: () => node.root,
      getNodePathInternal: () => node.root,
      getParentNode: () => node.parent?.wrapper,
      getParentNodeInternal: () => node.parent?.wrapper,
      getProject: () => (node.isProject ? node : node.parent)?.wrapper,
      findModuleByName: (name) => {
        const project = node.isProject ? node : node.parent
        return project?.children.find((child) => child.name === name)?.wrapper
      },
      subNodes: (callback) => node.children.forEach((child) => callback(child.wrapper)),
      subNodesInternal: (callback) => node.children.forEach((child) => callback(child.wrapper)),
      getSubNodeByName: (name) => node.children.find((child) => child.name === name)?.wrapper,
      getSubNodeByNameInternal: (name) => node.children.find((child) => child.name === name)?.wrapper,
      getContext: (pluginId) => {
        const factory = node.contextFactories.get(pluginId)
        return typeof factory === 'function' ? factory() : undefined
      },
      getContextInternal: (pluginId) => {
        const factory = node.contextFactories.get(pluginId)
        return typeof factory === 'function' ? factory() : undefined
      },
      getAllPluginIds: () => [...node.pluginIds],
      getAllPluginIdsInternal: () => [...node.pluginIds],
      getConfigOpt: () => this._makeConfig(node),
      getConfigOptInternal: () => this._makeConfig(node),
      addExtraOption: (key, value) => node.extraOptions.set(key, value),
      addExtraOptionInternal: (key, value) => node.extraOptions.set(key, value),
      getExtraOption: (key) => node.extraOptions.get(key),
      getExtraOptionInternal: (key) => node.extraOptions.get(key),
      registerTask: (task) => this._registerTask(node, task),
      registerTaskInternal: (task) => this._registerTask(node, task),
      getTaskByName: (name) => {
        const task = this._getTaskByName(node, name)
        if (!task) {
          this._warn('HVIGOR_TASK_NOT_FOUND', `The task '${name}' cannot be found on the current node.`)
        }
        return task?.wrapper
      },
      getTaskByNameInternal: (name) => {
        const task = this._getTaskByName(node, name)
        if (!task) {
          this._warn('HVIGOR_TASK_NOT_FOUND', `The task '${name}' cannot be found on the current node.`)
        }
        return task?.wrapper
      },
      getAllTasks: () => Array.from(node.tasks.values(), (task) => task.wrapper),
      getAllTasksInternal: () => Array.from(node.tasks.values(), (task) => task.wrapper),
      beforeNodeEvaluate: (callback) =>
        node.beforeNodeEvaluateCallbacks.push({ callback, thisArg: wrapper }),
      beforeNodeEvaluateInternal: (callback) =>
        node.beforeNodeEvaluateCallbacks.push({ callback, thisArg: wrapper }),
      afterNodeEvaluate: (callback) =>
        node.afterNodeEvaluateCallbacks.push({ callback, thisArg: wrapper }),
      afterNodeEvaluateInternal: (callback) =>
        node.afterNodeEvaluateCallbacks.push({ callback, thisArg: wrapper }),
      getPluginById: (pluginId) => node.systemPlugins.get(pluginId),
    }
    // getNode(__filename) historically exposed these aliases as well.
    wrapper.getName = wrapper.getNodeName
    wrapper.getNodeDirInternal = wrapper.getNodeDir
    Object.defineProperties(wrapper, {
      __arkdownHost: { value: this },
      __arkdownNode: { value: node },
    })
    return wrapper
  }

  _makeConfig(node) {
    return {
      getObject: (name) => node.loaderConfig?.[name],
      setObject: (name, value) => {
        if (!node.loaderConfig || typeof node.loaderConfig !== 'object') {
          node.loaderConfig = {}
        }
        node.loaderConfig[name] = value
      },
    }
  }

  _pluginIdForKind(kind) {
    return {
      app: 'com.ohos.app',
      hap: 'com.ohos.hap',
      har: 'com.ohos.har',
      hsp: 'com.ohos.hsp',
    }[kind]
  }

  _moduleTypeForKind(kind) {
    return { hap: 'entry', har: 'har', hsp: 'shared' }[kind] || kind
  }

  _invalidateRuntimePlan() {
    this._runtimePlanCache = undefined
  }

  _runtimePlanModules() {
    const nodesByRoot = new Map()
    for (const node of this._nodesByFile.values()) {
      if (!node.isProject) nodesByRoot.set(this._runtimeRootKey(node.root), node)
    }
    const modules = this._moduleDescriptors.map((descriptor) => {
      const node = nodesByRoot.get(this._runtimeRootKey(descriptor.root))
      if (node) nodesByRoot.delete(this._runtimeRootKey(descriptor.root))
      return node || {
        name: descriptor.name,
        root: descriptor.root,
        descriptor,
        moduleProfile: descriptor.moduleProfile || {},
      }
    })
    modules.push(...nodesByRoot.values())
    return modules.map((module) => ({
      name: module.name,
      root: module.root,
      descriptor: module.descriptor || {},
      buildProfile: module.moduleProfile || {},
      buildScriptOverride: this._moduleHvigorOverrides.get(path.resolve(module.root)),
    }))
  }

  _runtimePlan() {
    if (this._runtimePlanCache) return this._runtimePlanCache
    const rawModuleSelection = this._parameter.getExtParam('module') ?? this.config.module
    const response = JSON.parse(
      nativeRuntimeBinding.planBuildScriptRuntime(
        JSON.stringify({
          projectRoot: this.projectRoot,
          projectProfile: this._projectProfile,
          modules: this._runtimePlanModules(),
          productName: this._selectedProductName,
          buildModeName: this._effectiveBuildMode(),
          moduleSelection: typeof rawModuleSelection === 'string' ? rawModuleSelection : undefined,
          buildTargetName: this.config.buildTargetName,
          deviceTest: this._commandEntryTasks.includes('onDeviceTest'),
        }),
      ),
    )
    if (response.diagnostic) {
      const { code, message, ...details } = response.diagnostic
      throw this._error(code, message, details)
    }
    this._runtimePlanCache = response.plan
    return this._runtimePlanCache
  }

  _runtimePlanFor(node) {
    const key = this._runtimeRootKey(node.root)
    const module = this._runtimePlan().modules.find((candidate) => this._runtimeRootKey(candidate.root) === key)
    if (!module) {
      throw this._error(
        'ARKDOWN_BUILD_SCRIPT_RUNTIME_MODULE_NOT_FOUND',
        `Rust build-script runtime plan does not contain module ${JSON.stringify(node.name)} at ${node.root}`,
        { moduleName: node.name },
      )
    }
    return module
  }

  _moduleSelections() {
    return this._clone(this._runtimePlan().moduleSelections)
  }

  _effectiveTargetFor(node) {
    return node.isProject ? undefined : this._clone(this._runtimePlanFor(node).effectiveTarget)
  }

  _targetEntriesFor(node) {
    return node.isProject ? [{ name: 'default' }] : this._clone(this._runtimePlanFor(node).selectedTargets)
  }

  compilePluginRecords() {
    return this._clone(this._runtimePlan().compilePluginRecords)
  }

  _selectedProduct() {
    const product = this._selectedProductDefinition()
    const appOpt = this._projectHvigorOverrides?.appOpt || {}
    return {
      getProductName: () => product.name,
      getBundleType: () => product.bundleType || 'app',
      getBundleName: () => appOpt.bundleName ?? product.bundleName,
      getOutputArtifactName: () => product.output?.artifactName,
    }
  }

  _selectedProductDefinition() {
    return (
      toArray(this._projectProfile.app?.products).find(
        (candidate) => candidate.name === this._selectedProductName,
      ) || { name: this._selectedProductName }
    )
  }

  _buildOptionFor(node, target) {
    const plan = this._runtimePlanFor(node)
    return this._clone(plan.buildOptions[target.name] || plan.buildOptions.default || {})
  }

  _targetFor(node, target) {
    const moduleBuildPath = this._moduleBuildPath(node)
    const moduleProductPath = path.join(moduleBuildPath, this._selectedProductName)
    const moduleOutputPath = path.join(
      this._moduleBuildPath(node, false),
      this._selectedProductName,
      'outputs',
      target.name,
    )
    return {
      getTargetName: () => target.name,
      getCurrentProduct: () => this._selectedProduct(),
      // ModuleTargetImpl._getBuildOption() in hvigor 6.24.3 returns a cloned,
      // deeply frozen snapshot. Mutations must go through the module context.
      getBuildOption: () => this._freezeClone(this._buildOptionFor(node, target)),
      getBuildTargetOutputPath: () => moduleOutputPath,
      getModulePathDetails: () => ({
        getModuleBuildPath: () => moduleBuildPath,
        getIntermediatesRes: () => path.join(moduleProductPath, 'intermediates', 'res', target.name),
        getModuleBuildCachePath: () => path.join(moduleProductPath, 'cache', target.name),
      }),
    }
  }

  _packageValue(packageObject, key) {
    return this._clone(packageObject?.[key])
  }

  _unsupportedContextApi(api, reason) {
    throw this._error('ARKDOWN_UNSUPPORTED_HVIGOR_CONTEXT_API', `${api} is not supported: ${reason}`, { api })
  }

  _pathIdentity(candidate) {
    let resolved = path.resolve(candidate)
    try {
      resolved = fs.realpathSync.native(resolved)
    } catch {}
    return process.platform === 'win32' ? resolved.toLowerCase() : resolved
  }

  _configuredBuildDirectory(supportBuildCacheDir = true) {
    const buildCacheDir = this._parameter.getExtParam('build-cache-dir')
    let configured = buildCacheDir
    if (!configured || supportBuildCacheDir) {
      configured ||= this._parameter.getPropertyInternal('ohos.buildDir')
      if (
        configured &&
        this._parameter.getExtParam('previewMode') !== 'true' &&
        this._parameter.getExtParam('isLocalTest') !== 'true' &&
        this._parameter.getExtParam('isOhosTest') !== 'true'
      ) {
        return String(configured).replace(/^'|'$/g, '').replace(/^"|"$/g, '')
      }
    }
    return undefined
  }

  _buildCacheParentDirectory(localRoot, relativeRoot, supportBuildCacheDir = true) {
    let configured = this._configuredBuildDirectory(supportBuildCacheDir)
    if (!configured || !path.isAbsolute(configured)) return localRoot
    if (fs.existsSync(configured)) {
      try {
        configured = fs.realpathSync.native(configured)
      } catch {}
    }
    return path.join(configured, relativeRoot.replace(':', path.sep))
  }

  _projectBuildPath() {
    const projectName = path.basename(this.projectRoot)
    return path.resolve(
      this._buildCacheParentDirectory(this.projectRoot, projectName, false),
      'build',
    )
  }

  _moduleBuildCacheParent(node) {
    const projectName = path.basename(this.projectRoot)
    const moduleRelativePath = path.relative(this.projectRoot, node.root)
    return this._buildCacheParentDirectory(
      node.root,
      path.join(projectName, moduleRelativePath),
    )
  }

  _moduleBuildPath(node, supportBuildCacheDir = true) {
    const projectName = path.basename(this.projectRoot)
    const moduleRelativePath = path.relative(this.projectRoot, node.root)
    return path.resolve(
      this._buildCacheParentDirectory(
        node.root,
        path.join(projectName, moduleRelativePath),
        supportBuildCacheDir,
      ),
      'build',
    )
  }

  _inspectHvigorContext() {
    if (this._contextInspection) {
      return this._contextInspection
    }
    if (typeof this.inspectHvigorContext !== 'function') {
      return this._unsupportedContextApi(
        'OhosContext dependency inspection',
        'the native Rust context inspector is unavailable in this runtime',
      )
    }
    const runtimeOverrides = this.runtimeContextOverrides()
    const encoded = this.inspectHvigorContext({
      project: this.projectRoot,
      product: this._selectedProductName,
      runtimeContextOverridesJson: runtimeOverrides === undefined ? undefined : JSON.stringify(runtimeOverrides),
    })
    let snapshot
    try {
      snapshot = typeof encoded === 'string' ? JSON.parse(encoded) : encoded
    } catch (cause) {
      throw this._error('INVALID_HVIGOR_CONTEXT_SNAPSHOT', `failed to decode Rust context snapshot: ${cause.message}`, {
        cause,
      })
    }
    if (!snapshot || typeof snapshot !== 'object' || !snapshot.project || !snapshot.modules) {
      throw this._error('INVALID_HVIGOR_CONTEXT_SNAPSHOT', 'Rust context inspector returned an invalid snapshot')
    }
    this._contextInspection = snapshot
    return snapshot
  }

  _dependencyContextFor(node) {
    const snapshot = this._inspectHvigorContext()
    if (node.isProject) {
      return snapshot.project
    }
    const identity = this._pathIdentity(node.root)
    const entry = Object.entries(snapshot.modules).find(([root]) => this._pathIdentity(root) === identity)
    if (!entry) {
      throw this._error(
        'HVIGOR_CONTEXT_MODULE_NOT_FOUND',
        `Rust context snapshot does not contain module ${JSON.stringify(node.name)} at ${node.root}`,
      )
    }
    return entry[1]
  }

  _setProjectBuildProfile(node, profile) {
    const next = this._clone(profile)
    const currentProducts = toArray(this._projectProfile?.app?.products)
    const nextProducts = toArray(next?.app?.products)
    if (currentProducts.length !== nextProducts.length) {
      throw this._error(
        'CAN_NOT_ADD_OR_DELETE_PRODUCTS',
        'Products cannot be added to or removed from build-profile.json5 through OhosProjectContext.',
      )
    }
    for (const current of currentProducts) {
      const candidate = nextProducts.find((product) => product?.name === current?.name)
      if (!candidate || candidate.runtimeOS !== current.runtimeOS) {
        throw this._error(
          'CAN_NOT_CHANGE_NAME_OR_RUNTIME_PRODUCTS',
          `The product name or runtimeOS cannot be changed: ${JSON.stringify(current?.name)}.`,
        )
      }
    }
    this._projectProfile = next
    node.moduleProfile = next
    if (!this.config.product) {
      this._selectedProductName = nextProducts[0]?.name || 'default'
    }
    this._replaceProjectRuntimeValue('buildProfile', next)
  }

  _setPackageValue(owner, key, value) {
    this._validateDependencyVersions(value)
    owner[key] = this._clone(value)
  }

  _validateDependencyVersions(value) {
    for (const key in value) {
      if (typeof value[key] !== 'string') {
        throw this._error(
          'DEPENDENCY_VERSION_IS_NOT_A_STRING',
          `dependency version is not a string: ${String(value[key])}`,
          { dependency: key, version: value[key] },
        )
      }
    }
  }

  _targetArtifactNames(node) {
    return new Map(
      this._targetEntriesFor(node)
        .filter((target) => typeof target.output?.artifactName === 'string')
        .map((target) => [target.name, target.output.artifactName]),
    )
  }

  _sdkDetails() {
    if (this._sdkDetailsValue) return this._sdkDetailsValue
    const isOhos =
      (this._selectedProductDefinition().runtimeOS || this.config.runtimeOS) !== 'HarmonyOS'
    const requestedApi = this._compileApiVersion()
    let inventory
    try {
      inventory = JSON.parse(
        nativeRuntimeBinding.inspectHvigorSdkDetails(
          JSON.stringify({
            projectRoot: this.projectRoot,
            sdkPath: this.config.sdkPath,
            etsLoaderPath: this.config.etsLoaderPath,
            isOhos,
            requestedApi,
          }),
        ),
      )
    } catch (cause) {
      throw this._error('INVALID_SDK_COMPONENT_METADATA', cause.message, { cause })
    }
    const sdkDir = inventory.sdkDir || ''
    const metadata = inventory.metadata
    const product = this._selectedProductDefinition()
    const explicitlyConfiguredSdkVersion =
      this._apiVersion(product.compileSdkVersion) ??
      this._apiVersion(this.config.compileApiVersion) ??
      this._apiVersion(this.config.compileSdkVersion)
    const sdkVersion = explicitlyConfiguredSdkVersion ?? metadata?.apiVersion ?? requestedApi
    const etsComponentVersion = this.config.etsComponentVersion || metadata?.version || ''
    const etsComponentReleaseType = this.config.etsComponentReleaseType || metadata?.releaseType || 'Release'
    this._sdkDetailsValue = {
      isOhos: () => isOhos,
      getSdkDir: () => sdkDir,
      getSdkVersion: () => sdkVersion,
      getEtsComponentVersion: () => etsComponentVersion,
      getEtsComponentReleaseType: () => etsComponentReleaseType,
    }
    return this._sdkDetailsValue
  }

  _ohosContextFor(node) {
    if (node.ohosContext) {
      return node.ohosContext
    }
    if (node.kind === 'app') {
      const buildRoot = this._projectBuildPath()
      node.ohosContext = {
        getProjectName: () => node.name,
        getProjectPath: () => node.root,
        getBuildRootPath: () => buildRoot,
        getBuildProductOutputPath: () => path.join(buildRoot, 'outputs', this._selectedProductName),
        getBuildMode: () => this._effectiveBuildMode(),
        getBuildProfileOpt: () => this._clone(this._projectProfile),
        getCurrentProduct: () => this._selectedProduct(),
        getAppJsonOpt: () => this._clone(this._appJson),
        getDependenciesOpt: () => this._packageValue(this._projectPackage, 'dependencies'),
        getDevDependenciesOpt: () => this._packageValue(this._projectPackage, 'devDependencies'),
        getDynamicDependenciesOpt: () => this._packageValue(this._projectPackage, 'dynamicDependencies'),
        getOverrides: () => this._packageValue(this._projectPackage, 'overrides'),
        getSdkDetails: () => this._sdkDetails(),
        isDebugLineEnable: () => String(this._parameter.getExtParam('ohos-debug-line')) === 'true',
        getFastBuildApp: () => Boolean(this._projectProfile.app?.buildOption?.packOptions?.fastBuildApp),
        getAppWithSignedPkg: () => Boolean(this._projectProfile.app?.buildOption?.packOptions?.appWithSignedPkg),
        getOhpmDependencyInfo: () => this._clone(this._dependencyContextFor(node).ohpmDependencyInfo),
        getOhpmRemoteHspDependencyInfo: (isSigned = false) =>
          this._clone(
            isSigned
              ? this._dependencyContextFor(node).signedRemoteHspDependencyInfo
              : this._dependencyContextFor(node).remoteHspDependencyInfo,
          ),
        getAllModuleNameHash: () => this._inspectHvigorContext().allModuleNameHash,
        setBuildProfileOpt: (profile) => this._setProjectBuildProfile(node, profile),
        setAppJsonOpt: (appJson) => {
          this._appJson = this._clone(appJson)
          this._replaceProjectRuntimeValue('appJson', this._appJson)
        },
        setDependenciesOpt: (dependencies) => {
          this._setPackageValue(this._projectPackage, 'dependencies', dependencies)
          this._replaceProjectRuntimeValue('package', this._projectPackage)
        },
        setDevDependenciesOpt: (dependencies) => {
          this._setPackageValue(this._projectPackage, 'devDependencies', dependencies)
          this._replaceProjectRuntimeValue('package', this._projectPackage)
        },
        setDynamicDependenciesOpt: (dependencies) => {
          this._setPackageValue(this._projectPackage, 'dynamicDependencies', dependencies)
          this._replaceProjectRuntimeValue('package', this._projectPackage)
        },
        setOverrides: (overrides) => {
          this._setPackageValue(this._projectPackage, 'overrides', overrides)
          this._replaceProjectRuntimeValue('package', this._projectPackage)
        },
      }
      return node.ohosContext
    }

    const artifactNames = this._targetArtifactNames(node)
    node.ohosContext = {
      getModuleName: () => node.name,
      getModulePath: () => node.root,
      getModuleType: () => this._moduleTypeForKind(node.kind),
      // OhosModuleContextImpl resolves the configured cache parent against the
      // absolute module path. The latter wins, so this public API returns the
      // module root even when intermediates use a custom build directory.
      getBuildProductRootPath: () => path.resolve(this._moduleBuildCacheParent(node), node.root),
      getBuildMode: () => this._effectiveBuildMode(),
      targets: (callback) => this._targetEntriesFor(node).forEach((target) => callback(this._targetFor(node, target))),
      getTargetArtifactNames: () => new Map(artifactNames),
      getTargetArtifactName: (targetName) => artifactNames.get(targetName),
      getBuildProfileOpt: () => this._clone(node.moduleProfile),
      setBuildProfileOpt: (profile) => {
        node.moduleProfile = this._clone(profile) || {}
        node.taskCatalog.invalidate()
        node.parent?.taskCatalog.invalidate()
        this._replaceModuleRuntimeValue(node, 'buildProfile', node.moduleProfile)
      },
      getModuleJsonOpt: () => this._clone(node.moduleJson),
      setModuleJsonOpt: (moduleJson) => {
        node.moduleJson = this._clone(moduleJson)
        this._replaceModuleRuntimeValue(node, 'moduleJson', node.moduleJson)
      },
      getDependenciesOpt: () => this._packageValue(node.modulePackage, 'dependencies'),
      getDevDependenciesOpt: () => this._packageValue(node.modulePackage, 'devDependencies'),
      getDynamicDependenciesOpt: () => this._packageValue(node.modulePackage, 'dynamicDependencies'),
      getVersion: () => node.modulePackage?.version,
      getOhpmDependencyInfo: () => this._clone(this._dependencyContextFor(node).ohpmDependencyInfo),
      getOhpmRemoteHspDependencyInfo: (isSigned = false) =>
        this._clone(
          isSigned
            ? this._dependencyContextFor(node).signedRemoteHspDependencyInfo
            : this._dependencyContextFor(node).remoteHspDependencyInfo,
        ),
      setDependenciesOpt: (dependencies) => {
        this._setPackageValue(node.modulePackage, 'dependencies', dependencies)
        this._replaceModuleRuntimeValue(node, 'package', node.modulePackage)
      },
      setDevDependenciesOpt: (dependencies) => {
        this._setPackageValue(node.modulePackage, 'devDependencies', dependencies)
        this._replaceModuleRuntimeValue(node, 'package', node.modulePackage)
      },
      setDynamicDependenciesOpt: (dependencies) => {
        this._setPackageValue(node.modulePackage, 'dynamicDependencies', dependencies)
        this._replaceModuleRuntimeValue(node, 'package', node.modulePackage)
      },
      setVersion: (version) => {
        if (typeof version !== 'string') {
          throw this._error('VERSION_IS_NOT_A_STRING', `version is not a string: ${String(version)}`, { version })
        }
        node.modulePackage.version = version
        this._replaceModuleRuntimeValue(node, 'package', node.modulePackage)
      },
      loadCompilePlugin: (pluginPath) => this._loadCompilePlugin(node, pluginPath),
      transformAbc: (callback, targetName) => {
        const key = targetName ?? 'all'
        node.transformAbcActions.set(key, callback)
      },
    }
    return node.ohosContext
  }

  _typeCheckCompilePlugin(pluginFile) {
    let ts
    try {
      ts = require('typescript')
    } catch (cause) {
      throw this._error(
        'HVIGOR_TYPESCRIPT_NOT_FOUND',
        `type-checking compile plugin ${pluginFile} requires TypeScript ${cause.message}`,
        { cause, pluginPath: pluginFile },
      )
    }
    const baseUrl =
      this.hvigorWorkspaceNodeModules || path.resolve(process.env.WORKSPACE_DIR || this.projectRoot, 'node_modules')
    const program = ts.createProgram([pluginFile], {
      allowJs: true,
      alwaysStrict: false,
      baseUrl,
      declaration: false,
      esModuleInterop: false,
      forceConsistentCasingInFileNames: false,
      module: ts.ModuleKind.CommonJS,
      noEmit: true,
      noImplicitAny: false,
      noImplicitReturns: false,
      skipLibCheck: true,
      strict: false,
      target: ts.ScriptTarget.ES2016,
      typeRoots: this._typescriptTypeRoots(),
      types: [],
    })
    const diagnostics = ts.getPreEmitDiagnostics(program)
    if (diagnostics.length === 0) {
      return
    }
    const formatHost = {
      getCanonicalFileName: (fileName) => fileName,
      getCurrentDirectory: () => process.cwd(),
      getNewLine: () => ts.sys.newLine,
    }
    const error = this._error(
      'TYPE_ERRORS',
      `TypeScript errors in compile plugin ${pluginFile}:${ts.sys.newLine}${ts.formatDiagnostics(diagnostics, formatHost)}`,
      { pluginPath: pluginFile },
    )
    error.diagnostics = diagnostics.map((diagnostic) => {
      const location =
        diagnostic.file && diagnostic.start !== undefined
          ? diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start)
          : undefined
      return {
        category: ts.DiagnosticCategory[diagnostic.category].toLowerCase(),
        code: diagnostic.code,
        message: ts.flattenDiagnosticMessageText(diagnostic.messageText, ts.sys.newLine),
        file: diagnostic.file?.fileName,
        line: location ? location.line + 1 : undefined,
        column: location ? location.character + 1 : undefined,
      }
    })
    throw error
  }

  typeCheckBuildScripts() {
    if (!this._parameter.getCoreStartParams().hvigorfileTypeCheck || this._hvigorTypeScriptRoots.size === 0) {
      return
    }
    let ts
    try {
      ts = require('typescript')
    } catch (cause) {
      throw this._error(
        'HVIGOR_TYPESCRIPT_NOT_FOUND',
        `type-checking hvigorfiles requires TypeScript: ${cause.message}`,
        {
          cause,
        },
      )
    }
    const roots = Array.from(this._hvigorTypeScriptRoots).filter((file) => fs.existsSync(file))
    const program = ts.createProgram(roots, {
      allowJs: true,
      alwaysStrict: false,
      baseUrl: this.hvigorWorkspaceNodeModules || path.resolve(this.projectRoot, 'node_modules'),
      declaration: false,
      esModuleInterop: false,
      forceConsistentCasingInFileNames: false,
      module: ts.ModuleKind.CommonJS,
      noEmit: true,
      noImplicitAny: false,
      noImplicitReturns: false,
      skipLibCheck: false,
      strict: false,
      target: ts.ScriptTarget.ES2016,
      typeRoots: this._typescriptTypeRoots(),
      types: [],
    })
    const diagnostics = ts.getPreEmitDiagnostics(program)
    if (diagnostics.length === 0) return
    const formatHost = {
      getCanonicalFileName: (fileName) => fileName,
      getCurrentDirectory: () => process.cwd(),
      getNewLine: () => ts.sys.newLine,
    }
    const error = this._error(
      'TYPE_ERRORS',
      `TypeScript errors in ${roots.join(', ')}:${ts.sys.newLine}${ts.formatDiagnostics(diagnostics, formatHost)}`,
      { files: roots },
    )
    error.diagnostics = diagnostics.map((diagnostic) => {
      const location =
        diagnostic.file && diagnostic.start !== undefined
          ? diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start)
          : undefined
      return {
        category: ts.DiagnosticCategory[diagnostic.category].toLowerCase(),
        code: diagnostic.code,
        message: ts.flattenDiagnosticMessageText(diagnostic.messageText, ts.sys.newLine),
        file: diagnostic.file?.fileName,
        line: location ? location.line + 1 : undefined,
        column: location ? location.character + 1 : undefined,
      }
    })
    throw error
  }

  _typescriptTypeRoots() {
    const candidates = [path.join(this.projectRoot, 'node_modules', '@types')]
    if (this.hvigorWorkspaceNodeModules) {
      try {
        const hvigorPackage = fs.realpathSync.native(path.join(this.hvigorWorkspaceNodeModules, '@ohos', 'hvigor'))
        candidates.push(path.join(hvigorPackage, 'node_modules', '@types'))
      } catch {}
    }
    return candidates.filter((candidate) => fs.existsSync(candidate))
  }

  _loadCompilePlugin(node, pluginPath) {
    if (typeof pluginPath !== 'string' || pluginPath.length === 0) {
      throw this._error('EMPTY_COMPILE_PLUGIN_PATH', 'compile plugin path must not be empty')
    }
    const resolved = path.isAbsolute(pluginPath) ? pluginPath : path.resolve(node.root, pluginPath)
    if (!fs.existsSync(resolved)) {
      throw this._error('PLUGIN_PATH_NOT_FOUND', `compile plugin path does not exist: ${resolved}`, {
        pluginPath: resolved,
      })
    }
    if (!resolved.endsWith('.ts')) {
      throw this._error('PLUGIN_FILE_MUST_END_WITH_TS', `compile plugin file must end with .ts: ${resolved}`, {
        pluginPath: resolved,
      })
    }
    // Latest OhosModuleContextImpl calls doCompilePluginTypeCheck before
    // storing the plugin path. Keep the same TypeScript 4.9.5 program options
    // here; execution itself remains in Arkdown's Rolldown driver.
    this._typeCheckCompilePlugin(resolved)
    node.effects.compilePluginPath.push(resolved)
  }

  _initializeCoreTasks(node) {
    for (const name of node.isProject ? ['tasks', 'taskTree', 'prune'] : ['tasks', 'taskTree']) {
      this._realizeBuiltinTask(node, { name, dependencies: [] })
    }
  }

  _coreTaskFor(task) {
    if (task.taskContext?.__arkdownClassTask) return task.taskContext
    task.coreWrapper ||= new HvigorCoreTaskView(this, task)
    return task.coreWrapper
  }

  _taskCreationFor(node, taskName) {
    const customCreation = node.lazyTaskCreations.get(taskName)
    if (customCreation) return customCreation
    if (node.deletedCatalogTasks.has(taskName)) return undefined
    const definition = node.taskCatalog.get(taskName)
    if (!definition) return undefined
    let creation = node.catalogTaskCreations.get(taskName)
    if (!creation) {
      creation = {
        provider: () => this._coreTaskFor(this._realizeBuiltinTask(node, definition)),
        depends: [...definition.dependencies],
        details: {
          name: definition.name,
          group: definition.group,
          description: definition.description,
          sourcePluginId: definition.sourcePluginId,
        },
        dependsDefinedByUser: [],
      }
      node.catalogTaskCreations.set(taskName, creation)
    }
    return creation
  }

  _allTaskCreations(node) {
    const creations = []
    const names = new Set()
    for (const [taskName, creation] of node.lazyTaskCreations) {
      creations.push(creation)
      names.add(taskName)
    }
    for (const [taskName] of node.taskCatalog.entries()) {
      if (names.has(taskName) || node.deletedCatalogTasks.has(taskName)) continue
      creations.push(this._taskCreationFor(node, taskName))
    }
    return creations
  }

  _allTaskNames(node) {
    const names = new Set(node.tasks.keys())
    for (const taskName of node.lazyTaskCreations.keys()) names.add(taskName)
    for (const [taskName] of node.taskCatalog.entries()) {
      if (!node.deletedCatalogTasks.has(taskName)) names.add(taskName)
    }
    return Array.from(names)
  }

  _hasTask(node, taskName) {
    return (
      node.tasks.has(taskName) ||
      node.lazyTaskCreations.has(taskName) ||
      (!node.deletedCatalogTasks.has(taskName) && node.taskCatalog.has(taskName))
    )
  }

  _registerLazyTaskCreation(node, creation) {
    const taskName = creation?.details?.name
    if (typeof taskName !== 'string' || taskName.length === 0 || typeof creation.provider !== 'function') {
      throw this._error(
        'INVALID_HVIGOR_TASK_CREATION',
        `lazy task creation on ${node.name} must contain details.name and provider()`,
      )
    }
    if (this._hasTask(node, taskName)) throw new Error(`Duplicate task ${taskName}.`)
    node.lazyTaskCreations.set(taskName, creation)
  }

  _realizeLazyTask(node, taskName, creation) {
    const provided = creation.provider()
    if (!provided || typeof provided.getName !== 'function') {
      throw this._error(
        'INVALID_HVIGOR_TASK_CREATION_PROVIDER',
        `lazy task ${taskName} on ${node.name} did not provide a CoreTask`,
      )
    }
    if (!node.tasks.has(provided.getName())) this._registerTask(node, provided)
    const task = node.tasks.get(provided.getName())
    if (!task) {
      throw this._error(
        'INVALID_HVIGOR_TASK_CREATION_PROVIDER',
        `lazy task ${taskName} on ${node.name} was not registered by its provider`,
      )
    }
    const coreTask = this._coreTaskFor(task)
    const dependencies = [
      ...new Set([...toArray(coreTask.getDependsOn?.()), ...toArray(creation.depends)]),
    ]
    this._setCoreTaskDependencies(task, dependencies)
    return task
  }

  _coreDependencyReference(task, dependency, targetNode) {
    const taskName = typeof dependency?.getName === 'function' ? dependency.getName() : dependency
    if (typeof taskName !== 'string' || taskName.length === 0) {
      throw this._error('INVALID_HVIGOR_TASK_DEPENDENCY', `task ${task.path} has an invalid dependency`)
    }
    if (taskName.includes(':')) return taskName
    let owner = task.node
    if (targetNode !== undefined) {
      if (typeof targetNode === 'string') {
        const project = task.node.isProject ? task.node : task.node.parent
        owner =
          targetNode === ':' || targetNode === project?.name
            ? project
            : project?.children.find((candidate) => candidate.name === targetNode)
      } else {
        owner = targetNode?.__arkdownNode
      }
      if (!owner) {
        throw this._error(
          'CANNOT_FIND_NODE_SETTING_TASK',
          `cannot find node ${String(targetNode)} while configuring dependency of ${task.path}`,
        )
      }
    }
    return owner.isProject ? `:${taskName}` : `${owner.name}:${taskName}`
  }

  _setCoreTaskDependencies(task, dependencies) {
    task.userDependencies = dependencies.map((dependency) =>
      this._coreDependencyReference(task, dependency),
    )
    this._updateCoreTaskGraph(task)
  }

  _addCoreTaskDependency(task, dependency, targetNode) {
    task.userDependencies.push(this._coreDependencyReference(task, dependency, targetNode))
    task.userDependencies = [...new Set(task.userDependencies)]
    this._updateCoreTaskGraph(task)
  }

  _updateCoreTaskGraph(task) {
    const taskCore = this._coreTaskFor(task)
    task.node.taskGraph.addNode(taskCore)
    for (const reference of [...task.dependencies, ...task.userDependencies]) {
      const target = this._taskReference(task, reference, true).task
      if (target) task.node.taskGraph.addEdge(taskCore, this._coreTaskFor(target))
    }
  }

  _getTaskByName(node, name) {
    if (node.deletedCatalogTasks.has(name) && !node.tasks.has(name) && !node.lazyTaskCreations.has(name)) {
      return undefined
    }
    const existing = node.tasks.get(name)
    if (existing) {
      const lazyDefinition = node.taskCatalog.get(name)
      if (lazyDefinition) {
        existing.dependencies = [...new Set([...existing.dependencies, ...lazyDefinition.dependencies])]
      }
      return existing
    }
    const lazyCreation = node.lazyTaskCreations.get(name)
    if (lazyCreation) return this._realizeLazyTask(node, name, lazyCreation)
    const definition = node.taskCatalog.get(name)
    return definition ? this._realizeBuiltinTask(node, definition) : undefined
  }

  _realizeBuiltinTask(node, definition) {
    const existing = node.tasks.get(definition.name)
    if (existing) {
      existing.dependencies = [...new Set([...existing.dependencies, ...definition.dependencies])]
      return existing
    }
    const task = {
      key: `${node.name}:${definition.name}`,
      path: node.isProject ? `:${definition.name}` : `${node.name}:${definition.name}`,
      node,
      definition,
      taskContext: undefined,
      runContext: { moduleName: node.name, modulePath: node.root },
      input: { files: [], properties: new Map() },
      output: { files: [] },
      dependencies: [...definition.dependencies],
      userDependencies: [],
      enabled: true,
      builtIn: true,
      beforeRunHooks: [],
      afterRunHooks: [],
    }
    task.wrapper = new HvigorTaskView(task)
    task.coreWrapper = new HvigorCoreTaskView(this, task)
    node.tasks.set(definition.name, task)
    node.deletedCatalogTasks.delete(definition.name)
    return task
  }

  _qualifiedTaskDependency(node, reference) {
    if (reference.startsWith(':') || reference.includes(':')) {
      return reference
    }
    return node.isProject ? `:${reference}` : `${node.name}:${reference}`
  }

  _registerTask(node, registration) {
    const classTask = Boolean(
      registration?.__arkdownClassTask ||
      (typeof registration?.getName === 'function' && typeof registration?.getAction === 'function'),
    )
    const classIncrementalTask = registration instanceof HvigorIncrementalTask
    const definition = classTask
      ? {
          name: registration.getName(),
          group: registration.getGroup?.() || registration.group,
          description: registration.getDescription?.() || registration.description,
          sourcePluginId: registration.getSourcePluginId?.() || registration.sourcePluginId,
          dependencies: registration.getDependsOn?.() || [],
          __arkdownSyncTask: Boolean(registration.__arkdownSyncTask),
          __arkdownIncrementalTask: classIncrementalTask,
          async run(...args) {
            const action = registration.getAction()
            if (typeof action !== 'function') {
              throw new TypeError(`DefaultTask ${registration.getName()} registryAction() must return a function`)
            }
            return action.apply(registration, args)
          },
        }
      : registration
    if (!definition || typeof definition !== 'object' || typeof definition.name !== 'string') {
      throw this._error('INVALID_HVIGOR_TASK', `custom task on ${node.name} must contain a string name`)
    }
    const taskContext = classTask
      ? registration
      : {
          ...definition,
          context: definition.context == null ? undefined : definition.context.apply(),
        }
    taskContext.pendingPromises ||= new HvigorTaskPendingPromises()
    const input = new HvigorTaskInput()
    const output = new HvigorTaskOutput()
    if (classIncrementalTask) {
      const declarations = collectHvigorIncrementalDeclarations(registration)
      input.declarations.properties = declarations.inputs
      input.declarations.files = Array.from(declarations.inputFiles.collect().keys())
      output.declarations.files = Array.from(declarations.outputFiles.collect().keys())
    } else {
      definition.input?.apply(taskContext, [input])
      definition.output?.apply(taskContext, [output])
    }
    const lazyDefinition = node.taskCatalog.get(definition.name)
    const dependencies = [
      ...toArray(definition.dependencies)
        .filter((reference) => typeof reference === 'string')
        .map((reference) => this._qualifiedTaskDependency(node, reference)),
      ...(lazyDefinition?.dependencies || []),
    ]
    const task = {
      key: `${node.name}:${definition.name}`,
      path: node.isProject ? `:${definition.name}` : `${node.name}:${definition.name}`,
      node,
      definition,
      taskContext,
      runContext: { moduleName: node.name, modulePath: node.root },
      input: input.declarations,
      output: output.declarations,
      dependencies: [...new Set(dependencies)],
      userDependencies: [],
      enabled: classTask ? registration.getEnabled?.() !== false : true,
      trackState: !classTask || registration.unTrackStateReason === undefined,
      builtIn: false,
      beforeRunHooks: classTask
        ? toArray(registration.beforeRunHooks)
            .map((callback) => ({ callback, declared: false, classTask: true }))
        : typeof definition.beforeRun === 'function'
          ? [{ callback: definition.beforeRun, declared: true }]
          : [],
      afterRunHooks: classTask
        ? toArray(registration.afterRunHooks).map((callback) => ({ callback, declared: false, classTask: true }))
        : typeof definition.afterRun === 'function'
          ? [{ callback: definition.afterRun, declared: true }]
          : [],
    }
    task.wrapper = new HvigorTaskView(task)
    if (classTask) {
      registration.__arkdownTaskWrapper = task.wrapper
      registration.__arkdownInternalTask = task
    }
    task.coreWrapper = classTask ? registration : new HvigorCoreTaskView(this, task)
    node.tasks.set(definition.name, task)
    node.deletedCatalogTasks.delete(definition.name)
    for (const reference of Array.isArray(definition.postDependencies) ? definition.postDependencies : []) {
      if (typeof reference !== 'string') {
        continue
      }
      const target = this._taskReference(task, reference, true)
      if (!target.task) {
        this._warn(
          'HVIGOR_TASK_POST_DEPENDENCY_NOT_FOUND',
          `postDependency ${JSON.stringify(reference)} of task ${JSON.stringify(task.path)} was not found.`,
        )
        continue
      }
      target.task.userDependencies.push(task.path)
    }
    return registration
  }

  _taskReference(owner, reference, realizeBuiltIn = false) {
    const separator = reference.indexOf(':')
    const nodeName = separator >= 0 ? reference.slice(0, separator) : owner.node.name
    const taskName = separator >= 0 ? reference.slice(separator + 1) : reference
    const node = nodeName
      ? this._nodesByName.get(nodeName)
      : Array.from(this._nodesByFile.values()).find((item) => item.isProject)
    return {
      node,
      taskName,
      task: node ? (realizeBuiltIn ? this._getTaskByName(node, taskName) : node.tasks.get(taskName)) : undefined,
    }
  }

  _taskFromPath(owner, taskPath, realizeBuiltIn = true) {
    return this._taskReference(owner, taskPath, realizeBuiltIn).task
  }

  async _runTaskBeforeHooks(task) {
    for (let index = task.beforeRunHooks.length - 1; index >= 0; index--) {
      const hook = task.beforeRunHooks[index]
      if (hook.declared) {
        await hook.callback.apply(task.taskContext, [task.runContext])
      } else if (hook.classTask) {
        await hook.callback.call(task.taskContext)
      } else {
        await hook.callback.call(task.wrapper)
      }
    }
  }

  async _runTaskAfterHooks(task) {
    for (const hook of task.afterRunHooks) {
      if (hook.declared) {
        await hook.callback.apply(task.taskContext, [task.runContext])
      } else if (hook.classTask) {
        await hook.callback.call(task.taskContext)
      } else {
        await hook.callback.call(task.wrapper)
      }
    }
  }

  async _runTransformAbcEvent(event) {
    if (
      typeof event.moduleName !== 'string' ||
      typeof event.targetName !== 'string' ||
      typeof event.abcPath !== 'string' ||
      !event.config ||
      typeof event.config.isArkGuardEnabled !== 'boolean'
    ) {
      throw this._error('INVALID_ARKDOWN_TRANSFORM_ABC_EVENT', `invalid transformAbc event: ${JSON.stringify(event)}`)
    }
    const node = this._nodesByName.get(event.moduleName)
    if (!node) {
      throw this._error(
        'ARKDOWN_TRANSFORM_ABC_NODE_NOT_FOUND',
        `transformAbc references unknown node ${JSON.stringify(event.moduleName)}`,
      )
    }
    const callback = node.transformAbcActions.get(event.targetName) ?? node.transformAbcActions.get('all')
    if (callback === undefined) {
      return true
    }
    await Reflect.apply(callback, undefined, [event.abcPath, this._clone(event.config)])
    return true
  }

  _runNativeCommandModifier(event) {
    const modifier = this._commandBuilderModifiers.get(event?.builderType)
    if (!modifier) return event.command
    const command = modifier.modify([...event.command])
    if (!Array.isArray(command) || command.some((argument) => typeof argument !== 'string')) {
      throw this._error(
        'ARKDOWN_INVALID_COMMAND_MODIFIER_RESULT',
        `CommandBuilderModifier.modify() must return string[] for builder type ${String(event.builderType)}`,
        { builderType: event.builderType },
      )
    }
    return command
  }

  _captureTaskEffect(opts, file, kind) {
    const node = file ? this._nodeForFile(file, true) : null
    const effects = node?.effects || this._effects
    const options = opts || {}
    const ark = options.buildOption?.arkOptions || {}
    for (const candidate of toArray(options.compilePluginPath).concat(toArray(ark.compilePluginPath))) {
      effects.compilePluginPath.push(candidate)
    }
    for (const candidate of toArray(options.hvigorPluginFile).concat(toArray(ark.hvigorPluginFile))) {
      effects.hvigorPluginFile.push(candidate)
    }
    if (options.target) effects.target = options.target
    if (options.buildOption) effects.buildOption = mergeHvigorConfig(effects.buildOption, options.buildOption)
    const plugin = { name: 'arkdown-system-plugin' }
    Object.defineProperty(plugin, '__arkdownHvigorSystemKind', { value: kind || 'hap' })
    return plugin
  }

  _systemFactory(kind, file) {
    const factory = (opts) => this._captureTaskEffect(opts, file, kind)
    Object.defineProperty(factory, '__arkdownHvigorSystemKind', { value: kind })
    return factory
  }

  _legacySystemFactory(kind, file) {
    const factory = () => {
      throw this._error(
        'ARKDOWN_UNSUPPORTED_LEGACY_FA_MODEL',
        `legacy${kind[0].toUpperCase()}${kind.slice(1)}Tasks requires the legacy FA/JSBUNDLE pipeline, which arkdown does not support`,
        { kind, file },
      )
    }
    Object.defineProperty(factory, '__arkdownHvigorSystemKind', { value: `legacy-${kind}` })
    return factory
  }

  _publicNodePath(node) {
    if (typeof node?.getNodePath === 'function') return path.resolve(node.getNodePath())
    const nodeDir = node?.getNodeDir?.()
    if (typeof nodeDir === 'string') return path.resolve(nodeDir)
    if (typeof nodeDir?.getPath === 'function') return path.resolve(nodeDir.getPath())
    throw this._error('FAILED_TO_GET_REMOTE_HSP_NODE_PATH', 'getRemoteHspObjList expects an Hvigor node')
  }

  _getRemoteHspObjList(node) {
    const moduleRoot = this._publicNodePath(node)
    const project = node?.getProject?.()
    const projectRoot = project ? this._publicNodePath(project) : this.projectRoot
    const response = JSON.parse(
      nativeRuntimeBinding.inspectRemoteHspObjects(JSON.stringify({ moduleRoot, projectRoot })),
    )
    if (response.error) {
      const { code, message, ...details } = response.error
      throw this._error(code, message, details)
    }
    return response.objects
  }

  _registryCommandModifier(builderType, modifier) {
    this._commandBuilderModifiers.set(builderType, modifier)
  }

  _submitWorker(publicNode, taskName, workPath, options) {
    const nodeName = publicNode?.getNodeNameInternal?.() ?? publicNode?.getNodeName?.()
    const node = this._nodesByName.get(nodeName)
    if (!node) {
      throw this._error(
        'FAILED_TO_SUBMIT_WORKER_MODULE_NOT_FOUND',
        `worker node ${String(nodeName)} was not found`,
      )
    }
    const task = this._getTaskByName(node, taskName)
    if (!task) {
      throw this._error(
        'FAILED_TO_SUBMIT_WORKER_TASK_NOT_FOUND',
        `worker task ${String(taskName)} was not found on node ${node.name}`,
      )
    }
    if (typeof workPath !== 'string') {
      throw new TypeError('submitWorker workPath must be a string')
    }
    return this._workerPool.submit(task, workPath, options)
  }

  _makeOhosPluginMock(file = this._activeFile) {
    const OhosPluginId = Object.freeze({
      OHOS_APP_PLUGIN: 'com.ohos.app',
      OHOS_HAP_PLUGIN: 'com.ohos.hap',
      OHOS_HAR_PLUGIN: 'com.ohos.har',
      OHOS_HSP_PLUGIN: 'com.ohos.hsp',
    })
    return {
      OhosPluginId,
      appTasks: this._systemFactory('app', file),
      hapTasks: this._systemFactory('hap', file),
      hspTasks: this._systemFactory('hsp', file),
      harTasks: this._systemFactory('har', file),
      legacyAppTasks: this._legacySystemFactory('app', file),
      legacyHapTasks: this._legacySystemFactory('hap', file),
      legacyHarTasks: this._legacySystemFactory('har', file),
      getRemoteHspObjList: (node) => this._getRemoteHspObjList(node),
      CommandBuilderType: OHOS_COMMAND_BUILDER_TYPE,
      registryCommandModifier: (builderType, modifier) =>
        this._registryCommandModifier(builderType, modifier),
    }
  }

  _makeHvigorMock(file = this._activeFile) {
    const host = this
    const callerFile = file || host._activeFile
    const cacheKey =
      typeof callerFile === 'string' && callerFile.length > 0
        ? host._normalizeFile(callerFile)
        : '<unbound>'
    const cached = host._hvigorExports.get(cacheKey)
    if (cached) return cached
    const publicUtilities = (host._publicUtilities ||= createHvigorPublicUtilities(host))
    const registerLifecycle = (name, callbacks, callback) => {
      if (typeof callback !== 'function') {
        throw host._error('INVALID_HVIGOR_LIFECYCLE_CALLBACK', `hvigor.${name} expects a function`)
      }
      callbacks.push(callback)
    }
    const rootNode = () => {
      const node = Array.from(host._nodesByFile.values()).find((candidate) => candidate.isProject)?.wrapper
      if (!node) {
        throw host._error(
          'HVIGOR_CONFIG_FILE_ERROR',
          'The root node is not yet available for build. Register pre-node behavior with hvigor.configEvaluated().',
        )
      }
      return node
    }
    const allNodes = () => {
      const nodes = Array.from(host._nodesByFile.values())
      const root = nodes.find((node) => node.isProject)
      return [...nodes.filter((node) => node !== root), ...(root ? [root] : [])].map((node) => node.wrapper)
    }
    const hvigor = (host._hvigorApi ||= {
      getRootNode: rootNode,
      getAllNodes: allNodes,
      getNodeByName: (name, classKind) =>
        allNodes().find(
          (node) => node.getNodeName() === name && (classKind === undefined || node.classKind === classKind),
        ),
      getHvigorConfig: () => host._hvigorConfig,
      getParameter: () => host._parameter,
      beforeNodeEvaluate: (callback) =>
        registerLifecycle('beforeNodeEvaluate', host._beforeNodeEvaluateCallbacks, callback),
      afterNodeEvaluate: (callback) =>
        registerLifecycle('afterNodeEvaluate', host._afterNodeEvaluateCallbacks, callback),
      configEvaluated: (callback) => registerLifecycle('configEvaluated', host._configEvaluatedCallbacks, callback),
      nodesInitialized: (callback) => registerLifecycle('nodesInitialized', host._nodesInitializedCallbacks, callback),
      nodesEvaluated: (callback) => registerLifecycle('nodesEvaluated', host._nodesEvaluatedCallbacks, callback),
      taskGraphResolved: (callback) =>
        registerLifecycle('taskGraphResolved', host._taskGraphResolvedCallbacks, callback),
      buildFinished: (callback) => registerLifecycle('buildFinished', host._buildFinishedCallbacks, callback),
      onWatchWorkerMessage: (callback) =>
        registerLifecycle('onWatchWorkerMessage', host._watchWorkerMessageCallbacks, callback),
      getCommandEntryTask: () => (host._commandEntryTasks.length > 0 ? [...host._commandEntryTasks] : undefined),
      isCommandEntryTask: (taskName) => host._commandEntryTasks.includes(taskName),
      getExtraConfig: () => new Map(Object.entries(host._parameter.getExtParams())),
      getProject: rootNode,
      getHvigorUserHome,
      getHvigorVersion: () => HVIGOR_PARITY_VERSION,
    })
    const exports = {
      getNode: (scriptPath) => host._nodeForFile(scriptPath || callerFile || host._activeFile, false)?.wrapper,
      getHvigorNode: (scriptPath) =>
        host._nodeForFile(scriptPath || callerFile || host._activeFile, false)?.coreWrapper,
      hvigor,
      hvigorConfig: host._hvigorConfig,
      BuildResult: HvigorBuildResult,
      AnalyzeModeMap: HVIGOR_ANALYZE_MODE_MAP,
      CacheStore: publicUtilities.CacheStore,
      CacheStoreManager: publicUtilities.CacheStoreManager,
      ContinualEvent: publicUtilities.ContinualEvent,
      GlobalParam: publicUtilities.GlobalParam,
      HvigorConfigLoader: publicUtilities.HvigorConfigLoader,
      HvigorLogger: publicUtilities.HvigorLogger,
      LocalFileWriter: publicUtilities.LocalFileWriter,
      LogLevelMap: publicUtilities.LogLevelMap,
      CUSTOM_WORKER_REQUEST: HVIGOR_CUSTOM_WORKER_REQUEST,
      CoreTask: HvigorCoreTask,
      DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME: HVIGOR_DEFAULT_CONFIG_FILE_NAME,
      DefaultTask: HvigorDefaultTask,
      DefaultSyncTask: HvigorDefaultSyncTask,
      DurationEvent: publicUtilities.DurationEvent,
      ExtendedErrorInfo: publicUtilities.ExtendedErrorInfo,
      FileUtil: publicUtilities.FileUtil,
      HvigorSystemPlugin: publicUtilities.HvigorSystemPlugin,
      IncrementalExecTask: HvigorIncrementalExecTask,
      INCREMENTAL_OPTIMIZATION: HVIGOR_INCREMENTAL_OPTIMIZATION,
      Json5Reader: publicUtilities.Json5Reader,
      MAIN_THREAD: HVIGOR_MAIN_THREAD,
      MarkEvent: publicUtilities.MarkEvent,
      MarkEventType: publicUtilities.MarkEventType,
      MetricEventType: publicUtilities.MetricEventType,
      MetricFactory: publicUtilities.MetricFactory,
      MetricLogType: publicUtilities.MetricLogType,
      MetricService: publicUtilities.MetricService,
      OHOS_ARK_COMPILE_SOURCE_MAP_DIR,
      ParseJsonFile: publicUtilities.ParseJsonFile,
      PathUtil: publicUtilities.PathUtil,
      PoolConstant: publicUtilities.PoolConstant,
      ProjectCacheService: publicUtilities.ProjectCacheService,
      ReplacePlaceHolderUtil: HvigorReplacePlaceHolderUtil,
      ReportServiceImpl: publicUtilities.ReportServiceImpl,
      TrackAPI: publicUtilities.TrackAPI,
      TaskState: HVIGOR_TASK_STATE,
      Priority: HVIGOR_WORKER_PRIORITY,
      TaskDirectedAcyclicGraph: publicUtilities.TaskDirectedAcyclicGraph,
      TaskProxy: publicUtilities.TaskProxy,
      Tasks: publicUtilities.Tasks,
      TcbStore: publicUtilities.TcbStore,
      TraceManager: publicUtilities.TraceManager,
      WatchEvent: publicUtilities.WatchEvent,
      WorkerEvent: publicUtilities.WorkerEvent,
      WorkerPoolDelegator: publicUtilities.WorkerPoolDelegator,
      normalWorker: publicUtilities.normalWorker,
      projectTaskDag: publicUtilities.projectTaskDag,
      readJavaDaemonInfoFromCacheFile: publicUtilities.readJavaDaemonInfoFromCacheFile,
      submitWorker: (node, taskName, workPath, options) =>
        host._submitWorker(node, taskName, workPath, options),
      watchWorker: publicUtilities.watchWorker,
      WithParamReplacement: publicUtilities.WithParamReplacement,
      HVIGOR_PROJECT_WRAPPER_HOME: path.resolve(host.projectRoot, 'hvigor'),
      HvigorBuildConst,
      HvigorCommonConst,
      FileSet: HvigorFileSet,
      Input: HvigorInput,
      Inputs: HvigorInputs,
      InputFile: HvigorInputFile,
      InputFiles: HvigorInputFiles,
      OutputFile: HvigorOutputFile,
      OutputFiles: HvigorOutputFiles,
      IgnoreSuperIncremental: HvigorIgnoreSuperIncremental,
      formatTime: hvigorFormatTime,
      formatTimeToNumber: hvigorFormatTimeToNumber,
      addExtensionHandler: publicUtilities.addExtensionHandler,
      addLayout: publicUtilities.addLayout,
      configure: publicUtilities.configure,
      connectLogger: publicUtilities.connectLogger,
      coreParameter: publicUtilities.coreParameter,
      doCompilePluginTypeCheck: publicUtilities.doCompilePluginTypeCheck,
      getAlignTarget: () => host._parameter.getPropertyInternal('ohos.align.target'),
      getConfiguration: publicUtilities.getConfiguration,
      getHvigorConfigValue: publicUtilities.getHvigorConfigValue,
      getLogger: publicUtilities.getLogger,
      globalData: publicUtilities.globalData,
      hash: hvigorHash,
      hashFile: hvigorHashFile,
      isLinux: hvigorIsLinux,
      isMac: hvigorIsMac,
      isWindows: hvigorIsWindows,
      isConfigured: publicUtilities.isConfigured,
      iconv: publicUtilities.iconv,
      levelMap: publicUtilities.levelMap,
      levels: publicUtilities.levels,
      maxPathLength: hvigorMaxPathLength,
      noop: hvigorNoop,
      parseJsonFile: publicUtilities.parseJsonFile,
      replaceBundleName: hvigorReplaceBundleName,
      replacer: hvigorReplacer,
      recording: publicUtilities.recording,
      setCategoriesLevel: publicUtilities.setCategoriesLevel,
      shutdown: publicUtilities.shutdown,
      splitTime: hvigorSplitTime,
      startEnvironment: publicUtilities.startEnvironment,
      defaultModelRegistry: host._toolingModelRegistry,
      HvigorTaskGroupType: HVIGOR_TASK_GROUP_TYPE,
      hvigorCore: host._hvigorCore,
      TaskInput: HvigorTaskInput,
      TaskOutput: HvigorTaskOutput,
    }
    host._hvigorExports.set(cacheKey, exports)
    return exports
  }

  loadWithModuleMocks(file, callback) {
    const originalLoad = Module._load
    const host = this
    Module._load = function arkdownHvigorModuleLoad(request, parent, isMain) {
      if (request === '@ohos/hvigor') {
        return host._makeHvigorMock(parent?.filename || file)
      }
      if (request === '@ohos/hvigor-ohos-plugin') {
        return host._makeOhosPluginMock(parent?.filename || file)
      }
      return originalLoad.call(this, request, parent, isMain)
    }
    try {
      return callback()
    } finally {
      Module._load = originalLoad
    }
  }
}

// Plugin discovery and hook wrapping remain JavaScript glue. Function-valued
// ordering is resolved by the Rust Rolldown planner before TSFNs are created.
function normalizePluginList(value, baseDir) {
  const plugins = []
  appendNormalizedPlugins(plugins, unwrapDefault(value), baseDir)
  return applyFunctionOrder(plugins)
}

// normalizeSortedPluginList is the strict hvigor compile-plugin loader used by
// compilePluginPath / hvigorPluginFile. Unlike Arkdown's explicit plugin API,
// hvigor accepts only an object or a flat array of objects and requires a
// non-empty string name even when the plugin declares no hooks.
function normalizeSortedPluginList(value, _baseDir, pluginFile = '<compile-plugin>', moduleRoot, targetName) {
  const exported = unwrapDefault(value)
  const items = Array.isArray(exported) ? exported : [exported]
  const validation = JSON.parse(nativeRuntimeBinding.validateHvigorCompilePlugins(items, pluginFile))
  emitPluginPlanDiagnostics(validation)
  return items.map((plugin) => normalizeHvigorCompilePlugin(plugin, pluginFile, moduleRoot, targetName))
}

/**
 * Thin object-identity glue around the Rust hvigor/Rolldown plugin planner.
 * Rust owns scope grouping, the latest IDE built-in inventory, the sequential
 * order state machine, duplicate validation, and native-slot reduction. The
 * N-API planner invokes function-valued `order` callbacks on this V8 thread.
 */
class HvigorCompilePluginPlanner {
  constructor(config = {}) {
    this.config = config
  }

  apply(plugins) {
    const compilePlugins = plugins.filter((plugin) => plugin.__arkdownHvigorCompilePlugin)
    if (compilePlugins.length === 0) {
      return plugins
    }
    const response = JSON.parse(
      nativeRuntimeBinding.planHvigorCompilePlugins(
        JSON.stringify({
          compileMode: this.config.compileMode,
          preview: Boolean(this.config.preview),
          target: this.config.target,
          mode: this.config.mode || this.config.buildMode,
        }),
        compilePlugins,
      ),
    )
    if (response.error) {
      emitDiagnostic('error', response.error)
    }
    const retained = new Set()
    for (const placement of response.placements) {
      const plugin = compilePlugins[placement.pluginIndex]
      Object.defineProperty(plugin, '__arkdownOrderIndex', {
        configurable: true,
        enumerable: true,
        value: placement.orderIndex,
      })
      Object.defineProperties(plugin, {
        __arkdownOrderAnchorRank: {
          configurable: true,
          enumerable: true,
          value: placement.anchorRank,
        },
        __arkdownOrderAnchorSequence: {
          configurable: true,
          enumerable: true,
          value: placement.anchorSequence,
        },
      })
      retained.add(plugin)
    }
    return plugins.filter((plugin) => !plugin.__arkdownHvigorCompilePlugin || retained.has(plugin))
  }
}

// Object identity/shape glue for the Rust explicit-package-plugin planner.
// String orders remain on their plugin for PluginFactory; a callback order is
// removed after Rust resolves it into deterministic list order.
function applyFunctionOrder(plugins) {
  const response = JSON.parse(nativeRuntimeBinding.planRolldownPackagePlugins(plugins))
  emitPluginPlanDiagnostics(response)
  return response.placements.map((placement) => {
    const plugin = plugins[placement.pluginIndex]
    if (!placement.callbackResolved) return plugin
    const { order: _resolvedOrder, ...resolvedPlugin } = plugin
    return resolvedPlugin
  })
}

function appendNormalizedPlugins(plugins, item, baseDir) {
  const value = unwrapDefault(item)
  if (!value) {
    return
  }
  if (typeof value === 'string') {
    return
  }
  if (typeof value === 'function') {
    appendNormalizedPlugins(plugins, value(), baseDir)
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      appendNormalizedPlugins(plugins, item, baseDir)
    }
    return
  }
  if (typeof value === 'object' && hasRollupHook(value)) {
    plugins.push(normalizeHookArguments(value))
    return
  }
  if (typeof value === 'object' && value.path) {
    return
  }
}

function normalizeHvigorCompilePlugin(plugin, pluginFile, moduleRoot, targetName) {
  const normalized = normalizeHvigorCompileHookArguments(plugin, pluginFile)
  const internalProperties = {
    __arkdownHvigorCompilePlugin: {
      enumerable: false,
      value: true,
    },
    __arkdownHvigorOrder: {
      enumerable: false,
      value: plugin.order,
    },
  }
  if (moduleRoot) {
    internalProperties.__arkdownModuleRoot = {
      enumerable: false,
      value: path.resolve(moduleRoot),
    }
  }
  if (targetName) {
    internalProperties.__arkdownTargetName = {
      enumerable: false,
      value: targetName,
    }
  }
  Object.defineProperties(normalized, internalProperties)
  return normalized
}

function normalizeHvigorCompileHookArguments(plugin, pluginFile) {
  const normalized = { name: plugin.name }
  let pluginShareContext = {}
  const invoke = (hookName, context, args) => {
    try {
      return plugin[hookName].apply(context, args)
    } catch (cause) {
      cause.message = `${cause.message} ${pluginFile}`
      throw cause
    }
  }

  // Internal data bridge only: standard hooks remain native Rolldown hooks.
  // The latest hvigor fork adds `share` to PluginContext; seed the equivalent
  // data before Rolldown starts without exposing another lifecycle hook.
  Object.defineProperty(normalized, '__arkdownSeedHvigorContext', {
    enumerable: false,
    value(contextJson) {
      const hookContext = parsePluginContext(contextJson)
      if (hookContext.share) {
        pluginShareContext = hookContext.share
      }
    },
  })

  if (plugin.buildStart) {
    normalized.buildStart = function buildStart(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const contextJson = args[0]
      // Installed 6.24.3 arkts-pack.js::wrapperBuildStart invokes the custom
      // hook but deliberately does not return its value to Rollup. Preserve
      // that strict compile-plugin behavior, including an unawaited Promise.
      invoke('buildStart', buildPluginContext(parsePluginContext(contextJson), pluginShareContext, rolldownContext), [])
    }
  }
  for (const hookName of ['resolveId', 'load', 'transform']) {
    if (plugin[hookName]) {
      normalized[hookName] = function (...transportArgs) {
        const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
        return invoke(hookName, buildPluginContext({}, pluginShareContext, rolldownContext), args)
      }
    }
  }
  if (plugin.shouldInvalidCache) {
    normalized.shouldInvalidCache = function shouldInvalidCache(request) {
      return invoke('shouldInvalidCache', buildPluginContext({}, pluginShareContext), [
        parseShouldInvalidCacheRequest(request),
      ])
    }
  }
  if (plugin.moduleParsed) {
    normalized.moduleParsed = function moduleParsed(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const moduleInfo = args[0]
      const parsed = parseModuleInfo(moduleInfo)
      const moduleInfoMap = parsed?.id ? { [parsed.id]: parsed } : {}
      // Matches arkts-pack.js::wrapperModuleParsed: synchronous failures are
      // forwarded, while the user's return value is not returned to Rollup.
      invoke('moduleParsed', buildPluginContext({ moduleInfoMap }, pluginShareContext, rolldownContext), [parsed])
    }
  }
  if (plugin.beforeBuildEnd) {
    normalized.beforeBuildEnd = function beforeBuildEnd(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const contextJson = args[0]
      const hookContext = parsePluginContext(contextJson)
      return invoke('beforeBuildEnd', buildPluginContext(hookContext, pluginShareContext, rolldownContext), [
        hookContext.input,
      ])
    }
  }
  if (plugin.buildEnd) {
    normalized.buildEnd = function buildEnd(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const contextJson = args[0]
      const hookContext = parsePluginContext(contextJson)
      const count = Number(hookContext.errorCount || 0)
      const error = count > 0 ? new Error(`Rolldown build ended with ${count} error(s)`) : undefined
      // Matches arkts-pack.js::wrapperBuildEnd. `beforeBuildEnd` above is the
      // only lifecycle gap wrapper that returns and is therefore awaited.
      invoke('buildEnd', buildPluginContext(hookContext, pluginShareContext, rolldownContext), [error])
    }
  }
  return normalized
}

function parseModuleInfo(moduleInfo) {
  if (typeof moduleInfo !== 'string') return moduleInfo
  try {
    return JSON.parse(moduleInfo)
  } catch {
    return {
      id: moduleInfo,
      code: null,
      ast: null,
      originalCode: null,
      importedIds: [],
      importers: [],
      isEntry: false,
      isExternal: false,
      isIncluded: null,
      hasDefaultExport: null,
      exports: null,
      dynamicallyImportedIds: [],
      exportedBindings: null,
      dynamicImporters: [],
    }
  }
}

function emitPluginPlanDiagnostics(response) {
  for (const warning of response.warnings || []) emitDiagnostic('warn', warning)
  if (response.error) emitDiagnostic('error', response.error)
}

// emitDiagnostic is the central place to emit structured diagnostics.
// Errors throw; warnings go through the host warning channel.
function emitDiagnostic(level, diagnostic) {
  if (level === 'error') {
    const err = new Error(diagnostic.message)
    err.code = diagnostic.code
    if (diagnostic.pluginName) err.pluginName = diagnostic.pluginName
    throw err
  }
  // warning
  emitWarningDiagnostic(diagnostic)
}

function emitWarningDiagnostic(diagnostic) {
  const message = `[arkdown] ${diagnostic.code}: ${diagnostic.message}`
  if (typeof process !== 'undefined' && typeof process.emitWarning === 'function') {
    process.emitWarning(message, {
      code: diagnostic.code,
      type: 'ArkdownWarning',
    })
    return
  }
  if (typeof globalThis.dispatchEvent === 'function' && typeof globalThis.CustomEvent === 'function') {
    globalThis.dispatchEvent(new CustomEvent('arkdown:warning', { detail: diagnostic }))
  }
}

// ---------------------------------------------------------------------------
// Plugin context binding — hvigor's doWrapper / getCompilePluginContext
// ---------------------------------------------------------------------------

/**
 * Parse the JSON context string passed from Rust to a lifecycle hook.
 * Returns an empty object if the string is absent/invalid/not-JSON.
 */
function parsePluginContext(ctxJson) {
  if (!ctxJson || typeof ctxJson !== 'string' || ctxJson === '') return {}
  try {
    const parsed = JSON.parse(ctxJson)
    return typeof parsed === 'object' && parsed !== null ? parsed : {}
  } catch {
    return {}
  }
}

function reviveOxcPluginAst(envelope) {
  if (!envelope || typeof envelope !== 'object' || !envelope.node) return undefined
  const program = envelope.node
  for (const fixPath of envelope.fixes || []) {
    let node = program
    for (const key of fixPath) node = node[key]
    if (node.bigint !== undefined) {
      node.value = BigInt(node.bigint)
    } else if (node.regex) {
      try {
        node.value = new RegExp(node.regex.pattern, node.regex.flags)
      } catch {
        // Match OXC/Rollup: retain null when the current Node runtime cannot
        // construct a regular expression accepted by the parser.
      }
    }
  }
  return program
}

function normalizePluginParseError(cause) {
  const message = cause instanceof Error ? cause.message : String(cause)
  const parsed = /PLUGIN_PARSE_ERROR: (.*) \((\d+):(\d+)\) \[pos=(\d+)\]/.exec(message)
  if (!parsed) return cause
  const error = new SyntaxError(`${parsed[1]} (${parsed[2]}:${parsed[3]})`)
  error.code = 'PLUGIN_PARSE_ERROR'
  error.pos = Number(parsed[4])
  error.loc = { line: Number(parsed[2]), column: Number(parsed[3]) }
  return error
}

function parsePluginAst(source, options = {}, rolldownContext) {
  const nativeOptions = {
    sourceType: options.sourceType || 'module',
    ranges: options.ranges === true,
    locations: options.locations === true,
    sourceFile: options.sourceFile,
    directSourceFile: options.directSourceFile,
    allowReturnOutsideFunction: options.allowReturnOutsideFunction === true,
    preserveParens: options.preserveParens === true,
    captureComments: options.onComment !== undefined && options.onComment !== null,
    ecmaVersion: options.ecmaVersion ?? 'latest',
    allowAwaitOutsideFunction: options.allowAwaitOutsideFunction === true,
    allowSuperOutsideMethod: options.allowSuperOutsideMethod === true,
    allowHashBang: options.allowHashBang,
    hasOnToken: options.onToken !== undefined && options.onToken !== null,
    hasProgram: options.program !== undefined && options.program !== null,
  }
  let encoded
  try {
    const optionsJson = JSON.stringify(nativeOptions)
    encoded =
      rolldownContext && typeof rolldownContext.parse === 'function'
        ? rolldownContext.parse(source, optionsJson)
        : nativeRuntimeBinding.parsePluginAst(source, optionsJson)
  } catch (cause) {
    throw normalizePluginParseError(cause)
  }
  const result = JSON.parse(encoded)
  const comments = result.comments || []
  if (Array.isArray(options.onComment)) {
    options.onComment.push(...comments)
  } else if (typeof options.onComment === 'function') {
    for (const comment of comments) {
      options.onComment.call(
        options,
        comment.type === 'Block',
        comment.value,
        comment.start,
        comment.end,
        comment.loc?.start,
        comment.loc?.end,
      )
    }
  } else if (options.onComment !== undefined && options.onComment !== null) {
    throw new TypeError('onComment must be a function or an array')
  }
  return reviveOxcPluginAst(result.ast)
}

/**
 * Build a full hvigor-compatible compile plugin context object.
 *
 * Merges the per-plugin share data seeded before Rolldown starts with any
 * fields from the current hook's context JSON.
 *
 * Shape: `{ share: { bundleName, buildMode, … }, getModuleIds(), getModuleInfo(), … }`
 *
 * # Upstream correspondence
 *
 * Corresponds to hvigor's `getCompilePluginContext(this)` result from
 * `arkts-pack.ts`.  User plugins access `this.share.projectConfig.bundleName`,
 * `this.getModuleIds()`, etc., through this object.
 */
function buildPluginContext(hookCtx, fallbackShare = {}, rolldownContext) {
  const share = hookCtx.share || fallbackShare
  const moduleIds = hookCtx.moduleIds || []
  const moduleInfoMap = hookCtx.moduleInfoMap || {}
  const fileNames = hookCtx.fileNames || {}
  return {
    // hvigor share context
    share,
    // Hvigor forwards Rollup's IterableIterator-returning getModuleIds().
    getModuleIds() {
      if (rolldownContext) {
        return rolldownContext.getModuleIds().values()
      }
      return moduleIds.values()
    },
    // getModuleInfo(id) — returns { importedIds, dynamicallyImportedIds }
    getModuleInfo(id) {
      if (rolldownContext) {
        return rolldownContext.getModuleInfo(id)
      }
      return moduleInfoMap[id] || null
    },
    // Hvigor forwards this to Rollup's parser. Arkdown keeps the developer API
    // in JavaScript while OXC owns parsing and ESTree construction in Rust.
    parse(source, options = {}) {
      return parsePluginAst(source, options, rolldownContext)
    },
    getFileName(referenceId) {
      if (rolldownContext) {
        return rolldownContext.getFileName(referenceId)
      }
      if (Object.prototype.hasOwnProperty.call(fileNames, referenceId)) {
        return fileNames[referenceId]
      }
      const error = new Error(`Unable to get file name for unknown reference ID "${referenceId}"`)
      error.code = 'FILE_NAME_NOT_FOUND'
      throw error
    },
  }
}

function splitRolldownHookArguments(transportArgs) {
  const [candidate, ...args] = transportArgs
  if (
    candidate &&
    typeof candidate === 'object' &&
    typeof candidate.getModuleIds === 'function' &&
    typeof candidate.getModuleInfo === 'function' &&
    typeof candidate.getFileName === 'function'
  ) {
    return { rolldownContext: candidate, args }
  }
  return { rolldownContext: undefined, args: transportArgs }
}

function normalizeHookArguments(plugin) {
  const patches = {}
  let pluginShareContext = {}

  // The explicit API follows Rolldown/Rollup arguments. JSON is only the NAPI
  // transport; callers never observe that representation.
  if (typeof plugin.buildStart === 'function') {
    const original = plugin.buildStart
    patches.buildStart = function buildStart(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const optionsJson = args[0]
      const inputOptions = parsePluginContext(optionsJson)
      return original.call(buildPluginContext({}, pluginShareContext, rolldownContext), inputOptions)
    }
  }

  if (typeof plugin.beforeBuildEnd === 'function') {
    const original = plugin.beforeBuildEnd
    patches.beforeBuildEnd = function beforeBuildEnd(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const contextJson = args[0]
      const hookCtx = parsePluginContext(contextJson)
      return original.call(buildPluginContext(hookCtx, pluginShareContext, rolldownContext), hookCtx.input)
    }
  }

  if (typeof plugin.buildEnd === 'function') {
    const original = plugin.buildEnd
    patches.buildEnd = function buildEnd(...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const contextJson = args[0]
      const hookCtx = parsePluginContext(contextJson)
      const errors = Array.isArray(hookCtx.errors) ? hookCtx.errors : []
      let error
      if (errors.length > 0) {
        error = new Error(errors.map((item) => item.message || String(item)).join('\n'))
        error.name = 'RolldownError'
        error.errors = errors
      }
      return original.call(buildPluginContext(hookCtx, pluginShareContext, rolldownContext), error)
    }
  }

  // NAPI does not supply a JavaScript receiver for Rolldown hook calls.
  // Hvigor wraps every compile hook with getCompilePluginContext, so bind the
  // same context here while retaining the original hook arguments.
  for (const hookName of ['resolveId', 'resolveDynamicImport', 'load', 'transform']) {
    if (typeof plugin[hookName] === 'function') {
      const original = plugin[hookName]
      patches[hookName] = function (...transportArgs) {
        const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
        return original.apply(buildPluginContext({}, pluginShareContext, rolldownContext), args)
      }
    }
  }

  if (typeof plugin.moduleParsed === 'function') {
    const original = plugin.moduleParsed
    patches.moduleParsed = function (...transportArgs) {
      const { rolldownContext, args } = splitRolldownHookArguments(transportArgs)
      const moduleInfo = args[0]
      let parsedModuleInfo = moduleInfo
      if (typeof moduleInfo === 'string') {
        try {
          parsedModuleInfo = JSON.parse(moduleInfo)
        } catch {
          parsedModuleInfo = {
            id: moduleInfo,
            code: null,
            ast: null,
            originalCode: null,
            importedIds: [],
            importers: [],
            isEntry: false,
            isExternal: false,
            isIncluded: null,
            hasDefaultExport: null,
            exports: null,
            dynamicallyImportedIds: [],
            exportedBindings: null,
            dynamicImporters: [],
          }
        }
      }
      const moduleInfoMap = parsedModuleInfo && parsedModuleInfo.id ? { [parsedModuleInfo.id]: parsedModuleInfo } : {}
      return original.call(buildPluginContext({ moduleInfoMap }, pluginShareContext, rolldownContext), parsedModuleInfo)
    }
  }

  if (typeof plugin.shouldInvalidCache === 'function') {
    const original = plugin.shouldInvalidCache
    patches.shouldInvalidCache = function shouldInvalidCache(request) {
      return original.call(buildPluginContext({}, pluginShareContext), parseShouldInvalidCacheRequest(request))
    }
  }

  const normalized = Object.keys(patches).length === 0 ? { ...plugin } : { ...plugin, ...patches }
  Object.defineProperty(normalized, '__arkdownSeedHvigorContext', {
    enumerable: false,
    value(contextJson) {
      const hookContext = parsePluginContext(contextJson)
      if (hookContext.share) pluginShareContext = hookContext.share
    },
  })
  return normalized
}

function parseShouldInvalidCacheRequest(request) {
  let parsed = request
  try {
    if (typeof request === 'string') parsed = JSON.parse(request)
  } catch {
    return { cacheHit: Boolean(request) }
  }
  if (parsed && typeof parsed === 'object' && parsed.__arkdownOxcAst) {
    if (parsed.ast === undefined) parsed.ast = reviveOxcPluginAst(parsed.__arkdownOxcAst)
    delete parsed.__arkdownOxcAst
  }
  return parsed
}

const ALL_PLUGIN_HOOKS = new Set([...ROLLUP_HOOKS, 'beforeBuildEnd', 'shouldInvalidCache'])

function hasRollupHook(plugin) {
  return Array.from(ALL_PLUGIN_HOOKS).some((hook) => typeof plugin[hook] === 'function')
}

function hasHvigorfile(root) {
  return JSON.parse(
    nativeRuntimeBinding.discoverBuildScriptFiles(JSON.stringify({ roots: [path.resolve(root)] })),
  ).length > 0
}

function configureHvigorNodePath(projectRoot, projectHvigorConfig, plannedWorkspace, plannedNodeModules) {
  let config = projectHvigorConfig
  if (config === undefined) {
    const snapshot = JSON.parse(
      nativeRuntimeBinding.loadBuildScriptRuntimeModel(
        JSON.stringify({ projectRoot: path.resolve(projectRoot), hvigorUserHome: getHvigorUserHome() }),
      ),
    )
    config = snapshot.projectHvigorConfigPath ? snapshot.projectHvigorConfig : null
  }
  if (config === null) {
    return null
  }
  const dependencies =
    config && config.dependencies && typeof config.dependencies === 'object' ? { ...config.dependencies } : {}
  delete dependencies['@ohos/hvigor']
  delete dependencies['@ohos/hvigor-ohos-plugin']
  const workspace =
    plannedWorkspace ||
    path.join(getHvigorUserHome(), 'project_caches', hvigorProjectHash(projectRoot), 'workspace')
  const nodeModules = plannedNodeModules || path.join(workspace, 'node_modules')
  process.env.WORKSPACE_DIR = workspace
  const nodePathEntries = (process.env.NODE_PATH || '').split(path.delimiter).filter(Boolean)
  process.env.NODE_PATH = [nodeModules, ...nodePathEntries.filter((entry) => entry !== nodeModules)].join(
    path.delimiter,
  )
  Module._initPaths()
  installHvigorTypeScriptExtension()
  if (process.versions.node.split('.')[0] === '14') {
    installHvigorResolvePatch()
  }
  const priorContext = hvigorDependencyContexts.findIndex((context) => context.nodeModules === nodeModules)
  if (priorContext >= 0) {
    hvigorDependencyContexts.splice(priorContext, 1)
  }
  hvigorDependencyContexts.unshift({ nodeModules, dependencies })
  return nodeModules
}

function getHvigorUserHome() {
  if (process.env.HVIGOR_USER_HOME && path.isAbsolute(process.env.HVIGOR_USER_HOME)) {
    return process.env.HVIGOR_USER_HOME
  }
  return path.resolve(os.homedir(), '.hvigor')
}

function hvigorProjectHash(projectRoot) {
  return nativeRuntimeBinding.hvigorProjectCacheHash(path.resolve(projectRoot))
}

function readProjectHvigorConfig(file) {
  const projectRoot = path.dirname(path.dirname(path.resolve(file)))
  const snapshot = JSON.parse(
    nativeRuntimeBinding.loadBuildScriptRuntimeModel(
      JSON.stringify({ projectRoot, hvigorUserHome: getHvigorUserHome() }),
    ),
  )
  const config = snapshot.projectHvigorConfig || {}
  const dependencies =
    config && config.dependencies && typeof config.dependencies === 'object' ? { ...config.dependencies } : {}
  delete dependencies['@ohos/hvigor']
  delete dependencies['@ohos/hvigor-ohos-plugin']
  return { ...config, dependencies }
}

function installHvigorTypeScriptExtension() {
  if (hvigorTypeScriptExtensionInstalled) {
    return
  }
  const extension = function hvigorTypeScriptExtension(mod, filename) {
    mod._compile(compileHvigorScriptSync(filename), filename)
  }
  // This is the exact extension pair installed by latest hvigor's
  // base/boot/hooks/require-hook.js.
  Module._extensions['.ts'] = extension
  Module._extensions['.mjs'] = extension
  hvigorTypeScriptExtensionInstalled = true
}

function markHvigorScriptCompileError(error) {
  if (error && typeof error === 'object') {
    error.code = 'HVIGOR_TYPESCRIPT_DIAGNOSTIC'
  }
  return error
}

function compileHvigorScriptSync(filename, sourceMap = false) {
  try {
    return nativeRuntimeBinding.compileHvigorScriptSync(filename, sourceMap)
  } catch (error) {
    throw markHvigorScriptCompileError(error)
  }
}

async function compileHvigorScript(filename, sourceMap = false) {
  try {
    return await nativeRuntimeBinding.compileHvigorScript(filename, sourceMap)
  } catch (error) {
    throw markHvigorScriptCompileError(error)
  }
}

function installHvigorResolvePatch() {
  if (hvigorResolvePatchInstalled) {
    return
  }
  const originalResolveFilename = Module._resolveFilename
  Module._resolveFilename = function hvigorResolveFilename(request, parent, isMain, options) {
    try {
      return originalResolveFilename.call(this, request, parent, isMain, options)
    } catch (err) {
      const resolved = resolveHvigorDependencyIndex(request)
      if (resolved) {
        return resolved
      }
      throw err
    }
  }
  hvigorResolvePatchInstalled = true
}

function resolveHvigorDependencyIndex(request) {
  for (const context of hvigorDependencyContexts) {
    if (!Object.prototype.hasOwnProperty.call(context.dependencies, request)) {
      continue
    }
    const packageRoot = path.resolve(context.nodeModules, request)
    for (const ext of ['.js', '.ts']) {
      const indexFile = path.join(packageRoot, `index${ext}`)
      if (fs.existsSync(indexFile)) {
        return indexFile
      }
    }
  }
  return null
}

// host is an optional HvigorRuntimeHost — when provided, @ohos/hvigor-ohos-plugin
// returns callable task factories that capture config effects.
async function loadModule(file, host) {
  const ext = path.extname(file)
  // Latest @ohos/hvigor 6.24.3 installs the same CommonJS TypeScript
  // extension handler for both .ts and .mjs. Do not use native ESM import:
  // it would bypass the hvigor dependency mocks and change module semantics.
  if (ext === '.ts' || ext === '.mjs') {
    return loadHvigorScriptModule(file, host)
  }
  // Clear the require cache so that reloads (watch mode, test isolation)
  // always get a fresh copy — matching hvigor's behaviour.
  const resolved = require.resolve(file)
  if (host) {
    host._aliasBuildScriptFile(resolved, file)
  }
  delete require.cache[resolved]
  return host ? host.loadWithModuleMocks(file, () => require(resolved)) : require(resolved)
}

// host: optional HvigorRuntimeHost for capturing task-factory config effects.
async function loadHvigorScriptModule(file, host) {
  installHvigorTypeScriptExtension()
  const output = await compileHvigorScript(file)

  const mod = new Module(file, module.parent || module)
  mod.filename = file
  mod.paths = Module._nodeModulePaths(path.dirname(file)).concat(Module.globalPaths)
  if (host) {
    host.loadWithModuleMocks(file, () => mod._compile(output, file))
  } else {
    mod._compile(output, file)
  }
  return mod.exports
}

function unwrapDefault(value) {
  if (value && typeof value === 'object' && 'default' in value) {
    return value.default
  }
  return value
}

function requireDefaultExport(value, file) {
  if (value && typeof value === 'object' && 'default' in value && value.default) {
    return value.default
  }
  const err = new Error(`PLUGIN_FILE_NO_DEFAULT_EXPORT: ${file}`)
  err.code = 'PLUGIN_FILE_NO_DEFAULT_EXPORT'
  throw err
}

function toArray(value) {
  if (value == null) {
    return []
  }
  return Array.isArray(value) ? value : [value]
}

function mergeHvigorConfig(base, overlay) {
  return nativeRuntimeBinding.mergeHvigorConfig(base, overlay)
}

module.exports = {
  HVIGOR_PARITY_VERSION,
  HvigorRuntimeHost,
  HvigorCompilePluginPlanner,
  RuntimePluginLoader,
  createBuild,
  createHvigorCommand,
  toNativeBuildConfig,
  hasHvigorfile,
  normalizePluginList,
  normalizeSortedPluginList,
  applyFunctionOrder,
  parseShouldInvalidCacheRequest,
  parsePluginContext,
  buildPluginContext,
  requireDefaultExport,
  configureHvigorNodePath,
  hvigorProjectHash,
  readProjectHvigorConfig,
}
