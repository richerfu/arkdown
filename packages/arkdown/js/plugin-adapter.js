'use strict'

const Module = require('node:module')
const path = require('node:path')
const { pathToFileURL } = require('node:url')
const v8 = require('node:v8')

const nativeBinding = require('../index.js')

const ESMODULE_DORMANT_HOOKS = Object.freeze(
  'renderStart banner footer intro outro renderDynamicImport resolveFileUrl resolveImportMeta renderChunk augmentChunkHash renderError generateBundle writeBundle closeBundle closeWatcher'.split(
    ' ',
  ),
)
const PLUGIN_HOOKS = Object.freeze(
  'beforeBuild buildStart afterBuildStart resolveId load transform moduleParsed resolveDynamicImport watchChange beforeBuildEnd buildEnd afterBuildEnd bundleEnd cleanUp shouldInvalidCache'
    .split(' ')
    .concat(ESMODULE_DORMANT_HOOKS),
)

// @ohos/hvigor-arkts-compose/dist/src/hvigor-plugins/types.d.ts and
// arkts-pack.js::doWrapper expose exactly these hooks to compilePluginFile,
// hvigorPluginFile, and compilePluginPath modules. Arkdown's explicit plugin
// argument has a wider Rolldown surface and is deliberately kept separate.
const COMPILE_PLUGIN_HOOKS = Object.freeze(
  'buildStart resolveId load transform shouldInvalidCache moduleParsed beforeBuildEnd buildEnd'.split(' '),
)
const RESULT_FIELDS = Object.freeze({
  resolveId: 'resolveId',
  resolveDynamicImport: 'resolveId',
  load: 'load',
  transform: 'transform',
  shouldInvalidCache: 'shouldInvalidCache',
})
const TYPESCRIPT_EXTENSIONS = new Set(['.ts', '.mts', '.cts'])
const JAVASCRIPT_EXTENSIONS = new Set(['.js', '.mjs', '.cjs'])
const UNSUPPORTED_ROLLUP_HOOKS = new Set('options outputOptions shouldTransformCachedModule'.split(' '))
const REMOVED_HVIGOR_CONFIG_KEYS = new Set(
  'tasks task executionMode prop configOverrides hvigorProperties extParams sync typeCheck enableBuildScriptTypeCheck parallel daemon'.split(
    ' ',
  ),
)

let esmLoadSequence = 0
const hvigorShareStateByScope = new Map()
const hvigorShareByBuildInstance = new Map()
const HVIGOR_SHARE_STATE = Symbol('arkdown.hvigorShareState')
const HVIGOR_MODULE_OPTIONS = Symbol('arkdown.hvigorModuleOptions')
const PROJECT_CONFIG_MAP_FIELDS = Object.freeze([
  'depName2OhExports',
  'depName2DepInfo',
  'depName2RootPath',
  'dependencyAliasMap',
  'dependentModuleMap',
  'sdkAliasMap',
])

function arkdownError(code, message, details = {}) {
  const error = new Error(message)
  error.code = code
  Object.assign(error, details)
  return error
}

function emitDiagnostic(level, diagnostic) {
  if (level === 'error') {
    throw arkdownError(diagnostic.code, diagnostic.message, {
      ...(diagnostic.pluginName ? { pluginName: diagnostic.pluginName } : {}),
    })
  }
  process.emitWarning(`[arkdown] ${diagnostic.code}: ${diagnostic.message}`, {
    code: diagnostic.code,
    type: 'ArkdownWarning',
  })
}

function emitPlanDiagnostics(response) {
  for (const warning of response.warnings || []) emitDiagnostic('warning', warning)
  if (response.error) emitDiagnostic('error', response.error)
}

function unwrapDefault(value) {
  return value && typeof value === 'object' && 'default' in value ? value.default : value
}

function requireDefaultExport(value, file) {
  if (value && typeof value === 'object' && 'default' in value && value.default) return value.default
  throw arkdownError('PLUGIN_FILE_NO_DEFAULT_EXPORT', `Plugin module must have a default export: ${file}`, {
    pluginFile: file,
  })
}

// main.js::collectExternalApiCheckPlugin/collectExternalApiChecker: tsrs owns
// diagnostic decisions; this build-scoped host preserves only the CommonJS ABI.
class SdkPluginHost {
  constructor() {
    this.classInstances = new Map()
    this.typeScriptModules = new Map()
    this.sessions = new Map()
  }

  session(sessionId) {
    if (!this.sessions.has(sessionId))
      this.sessions.set(sessionId, {
        snapshots: new Map(),
        sourceFiles: new WeakMap(),
        configs: new Map(),
      })
    return this.sessions.get(sessionId)
  }

  loadTypeScript(etsLoaderPath) {
    if (!etsLoaderPath) throw new Error('SDK checker project config has no etsLoaderPath')
    const key = etsLoaderPath
    if (this.typeScriptModules.has(key)) return this.typeScriptModules.get(key)
    const typescript = require(path.join(etsLoaderPath, 'node_modules', 'typescript'))
    this.typeScriptModules.set(key, typescript)
    return typescript
  }

  materializeSnapshot(sessionId, snapshot) {
    const key = `${sessionId}:${snapshot.sourceId}`
    if (snapshot.source !== undefined) {
      this.session(sessionId).snapshots.set(key, { fileName: snapshot.fileName, source: snapshot.source })
      return snapshot
    }
    const cached = this.session(sessionId).snapshots.get(key)
    if (!cached || cached.fileName !== snapshot.fileName) {
      throw new Error(`SDK checker source ${key} is not registered`)
    }
    return { ...snapshot, source: cached.source }
  }

  materializeProjectConfig(sessionId, payload) {
    const key = `${sessionId}:${payload.projectConfigId}`
    if (payload.projectConfig !== undefined) {
      const projectConfig = { ...payload.projectConfig }
      projectConfig.syscapIntersectionSet = new Set(
        projectConfig.syscapIntersectionSet || projectConfig.syscapIntersection || [],
      )
      projectConfig.syscapUnionSet = new Set(projectConfig.syscapUnionSet || projectConfig.syscapUnion || [])
      this.session(sessionId).configs.set(key, projectConfig)
      return projectConfig
    }
    const projectConfig = this.session(sessionId).configs.get(key)
    if (!projectConfig) throw new Error(`SDK checker project config ${key} is not registered`)
    return projectConfig
  }

  locateNode(typescript, sessionId, snapshot) {
    snapshot = this.materializeSnapshot(sessionId, snapshot)
    const scriptKind =
      snapshot.fileName.endsWith('.ets') && typescript.ScriptKind.ETS !== undefined
        ? typescript.ScriptKind.ETS
        : typescript.ScriptKind.TS
    const sourceFiles = this.session(sessionId).sourceFiles
    let files = sourceFiles.get(typescript)
    if (!files) {
      files = new Map()
      sourceFiles.set(typescript, files)
    }
    let source = files.get(snapshot.fileName)
    if (!source || source.text !== snapshot.source || source.scriptKind !== scriptKind) {
      source = {
        text: snapshot.source,
        scriptKind,
        sourceFile: typescript.createSourceFile(
          snapshot.fileName,
          snapshot.source,
          typescript.ScriptTarget.Latest,
          true,
          scriptKind,
        ),
        nodes: new Map(),
        children: new WeakMap(),
      }
      files.set(snapshot.fileName, source)
    }
    const nodeKey = `${snapshot.pos}:${snapshot.end}`
    const cached = source.nodes.get(nodeKey)
    if (cached?.text === snapshot.text) return cached.node

    const sourceFile = source.sourceFile
    const childNodes = (node) => {
      let children = source.children.get(node)
      if (children) return children
      children = []
      typescript.forEachChild(node, (child) => {
        children.push(child)
      })
      children.sort((left, right) => left.pos - right.pos || left.end - right.end)
      source.children.set(node, children)
      return children
    }
    const locate = (node) => {
      if (node.pos > snapshot.pos || node.end < snapshot.end) return undefined
      let exact
      if (
        (node.pos === snapshot.pos || node.getStart(sourceFile) === snapshot.pos) &&
        node.end === snapshot.end &&
        node.getText(sourceFile) === snapshot.text
      ) {
        exact = node
      }

      // SDK SyscapChecker receives the original TypeScript nodes. The N-API
      // boundary recreates those nodes from the same source and span, so walk
      // only the containing AST branch. A full forEachChild scan per API use
      // turns large declaration files into quadratic work.
      const children = childNodes(node)
      let low = 0
      let high = children.length
      while (low < high) {
        const middle = (low + high) >>> 1
        if (children[middle].pos <= snapshot.pos) low = middle + 1
        else high = middle
      }
      for (let index = low - 1; index >= 0; index -= 1) {
        const child = children[index]
        if (child.end < snapshot.end) continue
        const nested = locate(child)
        if (nested) return nested
      }
      return exact
    }
    const exact = locate(sourceFile)
    if (exact) source.nodes.set(nodeKey, { text: snapshot.text, node: exact })
    return exact
  }

  classChecker(request) {
    const key = `${request.path}\0${request.className}`
    if (this.classInstances.has(key)) return this.classInstances.get(key)
    const Checker = require(request.path)[request.className]
    if (typeof Checker !== 'function') return undefined
    const checker = new Checker()
    this.classInstances.set(key, checker)
    return checker
  }

  dispatch(request) {
    if (request.operation === 'releaseSession') {
      const released = this.sessions.delete(request.sessionId)
      // serviceChecker releases state before exposed GC.
      if (released && typeof global.gc === 'function') global.gc()
      return { found: true }
    }
    if (request.operation === 'prepareClass' || request.operation === 'syscap') {
      let checker
      try {
        checker = this.classChecker(request)
      } catch (error) {
        return { found: false, phase: 'load', error: String(error?.stack || error) }
      }
      if (!checker) return { found: false }
      if (request.operation === 'prepareClass') return { found: true }
      try {
        if (typeof checker.check !== 'function') return { found: true, checkResult: false, checkMessage: '' }
        const payload = request.args[0]
        const projectConfig = this.materializeProjectConfig(request.sessionId, payload)
        const typescript = this.loadTypeScript(projectConfig.etsLoaderPath)
        const node = this.locateNode(typescript, request.sessionId, payload.node)
        const declaration = this.locateNode(typescript, request.sessionId, payload.declaration)
        if (!node || !declaration) {
          const missing = !node ? payload.node : payload.declaration
          throw new Error(
            `Unable to recreate SDK checker source node ${missing.fileName}:${missing.pos}-${missing.end} ${JSON.stringify(missing.text)}`,
          )
        }
        const value = checker.check(node, declaration, projectConfig)
        return {
          found: true,
          checkResult: Boolean(value?.checkResult),
          checkMessage: value?.checkMessage == null ? '' : String(value.checkMessage),
        }
      } catch (error) {
        return { found: true, phase: 'invoke', error: String(error?.stack || error) }
      }
    }

    let callback
    try {
      callback = require(request.path)[request.functionName]
    } catch (error) {
      return { found: false, phase: 'load', error: String(error?.stack || error) }
    }
    if (typeof callback !== 'function') return { found: false }
    try {
      if (request.operation === 'regex') {
        const regex = callback()
        if (!(regex instanceof RegExp)) return { found: true, matched: false }
        regex.lastIndex = 0
        const match = regex.exec(request.args[0])
        return {
          found: true,
          matched: match !== null,
          groups: match === null ? [] : Array.from(match, (value) => (value === undefined ? '' : String(value))),
        }
      }
      const value = callback(...request.args)
      if (request.operation === 'distribution') {
        return {
          found: true,
          valid: Boolean(value?.valid),
          version: value?.version == null ? '' : String(value.version),
          message: value?.message == null ? '' : String(value.message),
        }
      }
      return {
        found: true,
        result: Boolean(value?.result),
        message: value?.message == null ? '' : String(value.message),
      }
    } catch (error) {
      return { found: true, phase: 'invoke', error: String(error?.stack || error) }
    }
  }
}

function createSdkPluginHost() {
  const host = new SdkPluginHost()
  return (request) => host.dispatch(request)
}

function createSdkPluginDispatch() {
  const invoke = createSdkPluginHost()
  return (request) => JSON.stringify(JSON.parse(request).map(invoke))
}

const asArray = (value) => (value === undefined || value === null ? [] : Array.isArray(value) ? value : [value])

function validateHookSurface(plugin, hookNames, pluginFile, compilePlugin = false) {
  if (!compilePlugin) {
    for (const hookName of UNSUPPORTED_ROLLUP_HOOKS) {
      if (plugin[hookName] === undefined || plugin[hookName] === null) continue
      throw arkdownError(
        'ARKDOWN_PLUGIN_HOOK_UNSUPPORTED',
        `Plugin "${String(plugin.name || '<anonymous>')}" uses unsupported hook "${hookName}"`,
        { hookName, pluginFile },
      )
    }
  }
  for (const hookName of hookNames) {
    const hook = plugin[hookName]
    if (hook === undefined || hook === null) continue
    if (
      typeof hook !== 'function' &&
      !(compilePlugin === false && typeof hook === 'object' && typeof hook.handler === 'function')
    ) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_HOOK_INVALID',
        compilePlugin
          ? `Plugin "${String(plugin.name || '<anonymous>')}" hook "${hookName}" must be a function`
          : `Plugin "${String(plugin.name || '<anonymous>')}" hook "${hookName}" must be a function or an object with a handler function`,
        { hookName, pluginFile },
      )
    }
    if (
      !compilePlugin &&
      typeof hook === 'object' &&
      hook.order !== undefined &&
      hook.order !== 'pre' &&
      hook.order !== 'post'
    ) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_HOOK_ORDER_INVALID',
        `Plugin "${String(plugin.name || '<anonymous>')}" hook "${hookName}" order must be "pre" or "post"`,
        { hookName, pluginFile },
      )
    }
  }
}

const hookHandler = (hook) => (typeof hook === 'function' ? hook : hook.handler)

function reviveOxcAst(envelope) {
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
      } catch {}
    }
  }
  return program
}

function normalizeParseError(cause) {
  const message = cause instanceof Error ? cause.message : String(cause)
  const parsed = /PLUGIN_PARSE_ERROR: (.*) \((\d+):(\d+)\) \[pos=(\d+)\]/.exec(message)
  if (!parsed) return cause
  const error = new Error(parsed[1])
  error.name = 'RollupError'
  error.code = 'PARSE_ERROR'
  error.pos = Number(parsed[4])
  return error
}

function parsePluginAst(source, options, context, binding) {
  const parseOptions = options || {}
  const nativeOptions = {
    allowReturnOutsideFunction: parseOptions.allowReturnOutsideFunction === true,
  }
  let result
  try {
    result = context ? context.parse(source, nativeOptions) : binding.parsePluginAst(source, nativeOptions)
  } catch (cause) {
    throw normalizeParseError(cause)
  }
  return reviveOxcAst(result.ast)
}

function pluginLogMessage(value) {
  if (typeof value === 'string') return value
  if (value instanceof Error) return value.stack || value.message
  if (value && typeof value === 'object' && typeof value.message === 'string') return value.message
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

function createAsyncTracker() {
  let pending = 0
  let waiters = []
  return {
    run(operation, ...args) {
      if (typeof operation !== 'function') throw new TypeError('this.async expects a function')
      pending += 1
      try {
        return operation(...args)
      } catch (error) {
        this.signal()
        throw error
      }
    },
    signal() {
      if (pending === 0) return
      pending -= 1
      if (pending !== 0) return
      const completed = waiters
      waiters = []
      for (const resolve of completed) resolve()
    },
    block() {
      return pending === 0 ? Promise.resolve() : new Promise((resolve) => waiters.push(resolve))
    },
  }
}

const hvigorShareScope = (config) =>
  [config.projectTopDir, config.modulePath, config.moduleName, config.target, config.buildMode]
    .map((value) => value || '')
    .join('\0')

function createHvigorLogger(prefix, errorFileIds) {
  const write = (method, values) => {
    const message = values.map(pluginLogMessage).join(' ')
    if (method === 'warn') process.emitWarning(`[${prefix}] ${message}`, { type: 'ArkdownWarning' })
    else if (method === 'error') process.stderr.write(`[${prefix}] ${message}\n`)
    else if (method !== 'debug' || process.env.ARKDOWN_LOG === 'debug' || process.env.ARKDOWN_LOG === 'trace') {
      process.stdout.write(`[${prefix}] ${message}\n`)
    }
  }
  const logger = {
    printErrorAndExit(value) {
      const error = value instanceof Error ? value : new Error(pluginLogMessage(value))
      error.code ||= 'ARKTS_COMPILER_ERROR'
      throw error
    },
    recordErrorFileId(id) {
      if (id) errorFileIds.add(String(id))
    },
  }
  for (const level of ['debug', 'info', 'warn', 'error']) {
    logger[level] = (...values) => write(level, values)
    logger[`print${level[0].toUpperCase()}${level.slice(1)}`] = (value) => write(level, [value])
  }
  return logger
}

function createCacheStoreManager(stores) {
  return {
    keys: () => stores.keys(),
    unmount: (key) => stores.delete(key),
    mount(key) {
      if (!stores.has(key)) stores.set(key, new Map())
      const cache = stores.get(key)
      return { getCache: (key) => cache.get(key), setCache: (key, value) => cache.set(key, value) }
    },
  }
}

function restoreHvigorCacheState(state, snapshot) {
  if (state.snapshotLoaded) return
  state.snapshotLoaded = true
  if (!snapshot || snapshot.byteLength === 0) return
  let restored
  try {
    restored = v8.deserialize(Buffer.from(snapshot))
  } catch (cause) {
    process.emitWarning(`[arkdown] ignored unreadable plugin context cache: ${String(cause?.message || cause)}`, {
      code: 'ARKDOWN_PLUGIN_CACHE_DECODE',
      type: 'ArkdownWarning',
    })
    return
  }
  for (const key of ['cache', 'commonCache', 'cacheStores', 'pluginCaches', 'moduleInfos']) {
    if (restored?.[key] instanceof Map) state[key] = restored[key]
  }
}

function snapshotHvigorCacheState(share) {
  const state = share?.[HVIGOR_SHARE_STATE]
  if (!state) return undefined
  const snapshot = v8.serialize({
    cache: state.cache,
    commonCache: state.commonCache,
    cacheStores: state.cacheStores,
    pluginCaches: state.pluginCaches,
    moduleInfos: state.moduleInfos,
  })
  const buildInstanceId = share.__arkdownBuildInstanceId
  if (buildInstanceId !== undefined && buildInstanceId !== null) {
    hvigorShareByBuildInstance.delete(String(buildInstanceId))
  }
  return snapshot
}

function nativeShareMethod(share, binding, handle, localName, nativeName, fallback) {
  if (typeof share[localName] === 'function') return share[localName].bind(share)
  if (handle) return (...args) => binding[nativeName](handle, ...args)
  return fallback
}

function createHookEventFactory(record, state, buildInstanceId, pluginName, hookName) {
  const createEvent = (eventName, parentEventId) => {
    const eventId = `${String(buildInstanceId ?? 'plugin')}:${++state.hookEventSequence}`
    const base = {
      eventId,
      pluginName: String(pluginName),
      hookName: String(hookName),
      eventName: String(eventName),
    }
    if (parentEventId) base.parentEventId = parentEventId
    const emit = (operation, time, eventState, totalTime) => {
      const value = {
        ...base,
        operation,
      }
      if (operation !== 'setTotalTime') {
        value.timestampMs = Number(time ?? performance.timeOrigin + performance.now())
      }
      if (eventState !== undefined) value.state = String(eventState)
      if (totalTime !== undefined) value.totalTimeMs = Number(totalTime)
      record(value)
    }
    const event = {
      createEvent: (name) => createEvent(name),
      createSubEvent: (name) => createEvent(name, eventId),
      start(time) {
        emit('start', time)
        return event
      },
      stop: (eventState, time) => emit('stop', time, eventState),
      startAsyncEvent(time) {
        emit('startAsyncEvent', time)
        return event
      },
      stopAsyncEvent: (eventState, time) => emit('stopAsyncEvent', time, eventState),
      setTotalTime: (time) => emit('setTotalTime', undefined, undefined, time),
    }
    return event
  }
  return { createEvent: (name) => createEvent(name) }
}

function restoreProjectConfigCollections(projectConfig) {
  for (const key of PROJECT_CONFIG_MAP_FIELDS) {
    const value = projectConfig[key]
    if (value && typeof value === 'object' && !(value instanceof Map)) {
      projectConfig[key] = new Map(Object.entries(value))
    }
  }
  const sourceToModule = projectConfig.mockParams?.source2ModuleIdMap
  if (sourceToModule && typeof sourceToModule === 'object' && !(sourceToModule instanceof Map)) {
    projectConfig.mockParams.source2ModuleIdMap = new Map(Object.entries(sourceToModule))
  }
}

function hydrateHvigorShare(rawShare, binding) {
  const share = rawShare && typeof rawShare === 'object' ? rawShare : {}
  const projectConfig = share.projectConfig && typeof share.projectConfig === 'object' ? share.projectConfig : {}
  restoreProjectConfigCollections(projectConfig)
  if (typeof projectConfig.watchMode === 'boolean') projectConfig.watchMode = String(projectConfig.watchMode)
  const scope = hvigorShareScope(projectConfig)
  let state = hvigorShareStateByScope.get(scope)
  if (!state) {
    state = {
      cache: new Map(),
      commonCache: new Map(),
      cacheStores: new Map(),
      errorFileIds: new Set(),
      hashOverrides: new Map(),
      arkProjectConfig: {},
      pluginCaches: new Map(),
      moduleInfos: new Map(),
      pendingModuleInfos: new Map(),
      pendingBuildInstanceId: undefined,
      hookEventSequence: 0,
      snapshotLoaded: false,
    }
    hvigorShareStateByScope.set(scope, state)
  }
  restoreHvigorCacheState(state, share.__arkdownPluginCacheSnapshot)
  const buildInstanceId = share.__arkdownBuildInstanceId
  if (state.pendingBuildInstanceId !== buildInstanceId) {
    state.pendingBuildInstanceId = buildInstanceId
    state.pendingModuleInfos = new Map()
  }
  if (buildInstanceId !== undefined && buildInstanceId !== null) {
    const existing = hvigorShareByBuildInstance.get(String(buildInstanceId))
    if (existing) return existing
  }
  const handle = share.__arkdownNativeShare
  const nativeGetHash = nativeShareMethod(share, binding, handle, 'getHashByFilePath', 'hvigorShareGetHashByFilePath')
  const nativeGetFileInfo = nativeShareMethod(
    share,
    binding,
    handle,
    'getCachedFileInfo',
    'hvigorShareGetCachedFileInfo',
  )
  const nativeGetSourceFiles = nativeShareMethod(
    share,
    binding,
    handle,
    'getSourceFiles',
    'hvigorShareGetSourceFiles',
    () => [],
  )
  const nativeImportResolver = nativeShareMethod(
    share,
    binding,
    handle,
    'importResolver',
    'hvigorShareImportResolver',
    () => undefined,
  )
  const nativeRecordHookEvent = nativeShareMethod(
    share,
    binding,
    handle,
    'recordHookEvent',
    'hvigorShareRecordHookEvent',
    () => {},
  )
  const pathKey = (value) => path.resolve(String(value)).split(path.sep).join('/')
  Object.defineProperty(share, 'projectConfig', { value: projectConfig, enumerable: true, configurable: true })
  Object.defineProperty(share, HVIGOR_SHARE_STATE, { value: state })
  Object.defineProperty(share, HVIGOR_MODULE_OPTIONS, { value: new Map(), configurable: true })
  Object.assign(share, {
    arkProjectConfig: state.arkProjectConfig,
    cache: state.cache,
    commonCache: state.commonCache,
    cacheStoreManager: createCacheStoreManager(state.cacheStores),
    allFiles: new Set(nativeGetSourceFiles()),
    getCachedFileInfo: nativeGetFileInfo,
    getSourceFiles: nativeGetSourceFiles,
    importResolver: nativeImportResolver,
    getHashByFilePath(file) {
      const key = pathKey(file)
      return state.hashOverrides.get(key) ?? nativeGetHash?.(file) ?? ''
    },
    setHashValueByFilePath: (file, hash) => state.hashOverrides.set(pathKey(file), String(hash)),
    getLogger: (prefix) => createHvigorLogger(String(prefix), state.errorFileIds),
    getHvigorConsoleLogger: (prefix) => createHvigorLogger(String(prefix), state.errorFileIds),
    getHookEventFactory(pluginName, hookName) {
      if (projectConfig.perf !== 1 && projectConfig.perf !== 3) return undefined
      return createHookEventFactory(nativeRecordHookEvent, state, buildInstanceId, pluginName, hookName)
    },
    throwArkTsCompilerError(value) {
      const error = value instanceof Error ? value : new Error(pluginLogMessage(value))
      error.code ||= 'ARKTS_COMPILER_ERROR'
      throw error
    },
    recordErrorFileId(id) {
      if (id) state.errorFileIds.add(String(id))
    },
    depInfo:
      share.depInfo && typeof share.depInfo === 'object' ? share.depInfo : { enableIncre: false, changedDeps: [] },
  })
  if (buildInstanceId !== undefined && buildInstanceId !== null) {
    hvigorShareByBuildInstance.set(String(buildInstanceId), share)
  }
  return share
}

function updateModuleOptions(share, id, value) {
  if (!id || !value || typeof value !== 'object') return
  const options = share?.[HVIGOR_MODULE_OPTIONS]
  if (!options) return
  const current = options.get(id) || { meta: {} }
  if (value.meta && typeof value.meta === 'object') current.meta = { ...current.meta, ...value.meta }
  for (const key of ['moduleSideEffects', 'description', 'packageJsonPath']) {
    if (Object.hasOwn(value, key)) current[key] = value[key]
  }
  options.set(id, current)
}

function moduleInfoWithOptions(share, info) {
  if (!info) return info
  const options = share?.[HVIGOR_MODULE_OPTIONS]?.get(info.id)
  return {
    ...info,
    meta: options?.meta || info.meta || {},
    moduleSideEffects:
      options && Object.hasOwn(options, 'moduleSideEffects')
        ? options.moduleSideEffects
        : (info.moduleSideEffects ?? null),
    ...(options && Object.hasOwn(options, 'description') ? { description: options.description } : {}),
    ...(options && Object.hasOwn(options, 'packageJsonPath') ? { packageJsonPath: options.packageJsonPath } : {}),
  }
}

function pluginContext(request, share, binding, cache, asyncTracker, pluginName, compilePlugin) {
  const context = request.context
  const moduleInfo = moduleInfoWithOptions(share, request.moduleInfo)
  const state = share?.[HVIGOR_SHARE_STATE]
  const unavailable = (operation) => {
    throw arkdownError('PLUGIN_CONTEXT_UNAVAILABLE', `${operation} requires a live Rolldown plugin context`)
  }
  const log = (level, value) => {
    const message = pluginLogMessage(value)
    if (context) context[level](message)
    else if (level === 'warn') process.emitWarning(message, { code: 'PLUGIN_WARNING', type: 'ArkdownWarning' })
    else process[level === 'debug' ? 'stderr' : 'stdout'].write(`${message}\n`)
  }
  const getModuleIds = () =>
    context?.getModuleIds().values() ?? state?.moduleInfos.keys() ?? (moduleInfo?.id ? [moduleInfo.id] : []).values()
  const getModuleInfo = (id) =>
    moduleInfoWithOptions(
      share,
      context?.getModuleInfo(id) ?? state?.moduleInfos.get(id) ?? (moduleInfo?.id === id ? moduleInfo : null),
    )
  const parse = (source, options = {}) => parsePluginAst(source, options, context, binding)
  const getFileName = (referenceId) => {
    if (context) return context.getFileName(referenceId)
    throw arkdownError('FILE_NAME_NOT_FOUND', `Unable to get file name for unknown reference ID "${referenceId}"`)
  }
  if (compilePlugin) {
    // arkts-pack.js::getCompilePluginContext forwards only these five members.
    return { getFileName, getModuleIds, getModuleInfo, parse, share }
  }
  return {
    share,
    cache,
    meta: {
      rollupVersion: '4.23.0',
      rolldownVersion: 'arkdown-native',
      watchMode: share?.projectConfig?.watchMode === true || share?.projectConfig?.watchMode === 'true',
    },
    async: (operation, ...args) => asyncTracker.run(operation, ...args),
    signal: () => asyncTracker.signal(),
    block: () => asyncTracker.block(),
    resolve: async (source, importer, options) => {
      const result = context ? await context.resolve(source, importer, options) : null
      return moduleInfoWithOptions(share, result)
    },
    load: async (options) => {
      updateModuleOptions(share, options?.id, options)
      const result = context ? await context.load(options) : moduleInfo?.id === options?.id ? moduleInfo : null
      return moduleInfoWithOptions(share, result)
    },
    emitFile: (file) => (context ? context.emitFile(file) : unavailable('emitFile')),
    addWatchFile: (id) => context?.addWatchFile(id),
    getModuleIds,
    getModuleInfo,
    ...(request.hookName === 'transform' && context
      ? { getCombinedSourcemap: () => context.getCombinedSourcemap() }
      : {}),
    parse,
    getFileName,
    info: (value) => log('info', value),
    warn: (value) => log('warn', value),
    debug(value) {
      if (process.env.ARKDOWN_LOG === 'debug' || process.env.ARKDOWN_LOG === 'trace') log('debug', value)
    },
    error(value) {
      const error = value instanceof Error ? value : new Error(pluginLogMessage(value))
      if (value && typeof value === 'object' && !(value instanceof Error)) Object.assign(error, value)
      error.code ||= 'PLUGIN_ERROR'
      error.plugin ||= pluginName
      throw error
    },
  }
}

function cacheRequest(request) {
  const value = request && typeof request === 'object' ? request : { cacheHit: Boolean(request) }
  if (value.__arkdownOxcAst) {
    if (value.ast === undefined) value.ast = reviveOxcAst(value.__arkdownOxcAst)
    delete value.__arkdownOxcAst
  }
  return value
}

function hookArguments(hookName, request, compilePlugin) {
  switch (hookName) {
    case 'buildStart':
      return compilePlugin ? [] : [request.buildStartOptions]
    case 'beforeBuild':
    case 'afterBuildStart':
    case 'afterBuildEnd':
    case 'bundleEnd':
    case 'cleanUp':
      return []
    case 'resolveId':
    case 'resolveDynamicImport':
      return [request.id, request.importer, request.resolveOptions]
    case 'load':
      return [request.id]
    case 'transform':
      return compilePlugin ? [request.code, request.id] : [request.code, request.id, request.transformOptions]
    case 'moduleParsed':
      return [request.moduleInfo]
    case 'watchChange':
      return [request.id, { event: request.watchEvent }]
    case 'beforeBuildEnd':
      return [undefined]
    case 'shouldInvalidCache':
      return [cacheRequest(request.cacheRequest)]
    case 'buildEnd': {
      const errors = request.buildEndContext?.errors || []
      if (errors.length === 0) return [undefined]
      const error = new Error(errors.map((item) => item.message || String(item)).join('\n'))
      error.name = 'RolldownError'
      error.errors = errors
      return [error]
    }
    case 'renderStart':
      return [request.outputOptions]
    case 'banner':
    case 'footer':
    case 'intro':
    case 'outro':
    case 'augmentChunkHash':
      return [request.chunk]
    case 'renderDynamicImport':
    case 'resolveFileUrl':
    case 'resolveImportMeta':
      return [request.outputHookOptions]
    case 'renderChunk':
      return [request.code, request.chunk, request.outputOptions]
    case 'renderError':
      return [request.error]
    case 'generateBundle':
      return [request.outputOptions, request.bundle, request.isWrite === true]
    case 'writeBundle':
      return [request.outputOptions, request.bundle]
    case 'closeBundle':
    case 'closeWatcher':
      return []
    default:
      throw arkdownError('ARKDOWN_PLUGIN_HOOK_UNKNOWN', `Unknown native plugin hook "${String(hookName)}"`)
  }
}

function normalizePlugin(plugin, binding, options = {}) {
  const compilePlugin = options.compilePlugin === true
  const pluginFile = options.pluginFile
  const allowedHooks = compilePlugin ? COMPILE_PLUGIN_HOOKS : PLUGIN_HOOKS
  validateHookSurface(plugin, allowedHooks, pluginFile, compilePlugin)

  const normalized = compilePlugin ? { name: plugin.name } : { ...plugin }
  const hookNames = allowedHooks.filter((name) => {
    const hook = plugin[name]
    return typeof hook === 'function' || (hook && typeof hook === 'object' && typeof hook.handler === 'function')
  })
  const hookMeta = hookNames.flatMap((name) => {
    const hook = plugin[name]
    return typeof hook === 'object' && hook.order ? [{ name, order: hook.order }] : []
  })
  for (const hookName of PLUGIN_HOOKS) delete normalized[hookName]
  let share = {}
  let cache = new Map()
  const asyncTracker = createAsyncTracker()
  const cacheIdentity = `${options.pluginCacheIdentity || (pluginFile ? path.resolve(pluginFile) : '<explicit>')}\0${String(plugin.name || '<anonymous>')}`

  const dispatch = (request = {}) => {
    if (request.hookName === 'seedContext') {
      share = hydrateHvigorShare(request.share, binding)
      const state = share[HVIGOR_SHARE_STATE]
      if (state) {
        if (!state.pluginCaches.has(cacheIdentity)) state.pluginCaches.set(cacheIdentity, new Map())
        cache = state.pluginCaches.get(cacheIdentity)
      }
      return {}
    }
    if (request.hookName === 'snapshotContext') {
      const cacheSnapshot = snapshotHvigorCacheState(share)
      return cacheSnapshot ? { cacheSnapshot } : {}
    }
    if (request.moduleInfo) {
      request.moduleInfo = moduleInfoWithOptions(share, request.moduleInfo)
      const state = share?.[HVIGOR_SHARE_STATE]
      if (state && request.moduleInfo.id) {
        state.pendingModuleInfos.set(request.moduleInfo.id, request.moduleInfo)
      }
    }
    const hookName = request.hookName
    const context = pluginContext(
      request,
      share,
      binding,
      cache,
      asyncTracker,
      plugin.name || '<anonymous>',
      compilePlugin,
    )
    const handler = plugin[hookName] && hookHandler(plugin[hookName])
    const invoke = () => handler?.apply(context, hookArguments(hookName, request, compilePlugin))
    let result
    try {
      result = hookName === 'bundleEnd' ? asyncTracker.block().then(invoke) : invoke()
    } catch (error) {
      if (compilePlugin && pluginFile && error && typeof error === 'object') {
        error.message = `${String(error.message || error)} ${pluginFile}`
      }
      throw error
    }
    // arkts-pack.js wrappers intentionally do not return buildStart,
    // moduleParsed, or buildEnd results from Hvigor compile plugins.
    if (compilePlugin && ['buildStart', 'moduleParsed', 'buildEnd'].includes(hookName)) result = undefined
    const field = RESULT_FIELDS[hookName]
    if (!field) {
      const finish = () => {
        if (compilePlugin && hookName === 'buildEnd') {
          const state = share?.[HVIGOR_SHARE_STATE]
          if ((request.buildEndContext?.errors?.length ?? 0) === 0 && state) {
            state.moduleInfos = state.pendingModuleInfos
          }
        }
        if (hookName === 'cleanUp') {
          const buildInstanceId = share.__arkdownBuildInstanceId
          if (buildInstanceId !== undefined && buildInstanceId !== null) {
            hvigorShareByBuildInstance.delete(String(buildInstanceId))
          }
        }
        if (compilePlugin && hookName === 'buildEnd' && (request.buildEndContext?.errors?.length ?? 0) > 0) {
          const buildInstanceId = share.__arkdownBuildInstanceId
          if (buildInstanceId !== undefined && buildInstanceId !== null) {
            hvigorShareByBuildInstance.delete(String(buildInstanceId))
          }
        }
        return {}
      }
      return result && typeof result.then === 'function' ? Promise.resolve(result).then(finish) : finish()
    }
    const wrap = (value) => {
      if (value === undefined || value === null) return {}
      if (compilePlugin && hookName === 'shouldInvalidCache' && typeof value !== 'boolean') {
        return { shouldInvalidCache: false }
      }
      const id =
        (hookName === 'resolveId' || hookName === 'resolveDynamicImport') && typeof value === 'object'
          ? value.id
          : request.id
      updateModuleOptions(share, id, value)
      if (typeof value !== 'object' || !Object.hasOwn(value, 'meta')) return { [field]: value }
      const nativeValue = { ...value }
      delete nativeValue.meta
      return { [field]: nativeValue }
    }
    return result && typeof result.then === 'function' ? Promise.resolve(result).then(wrap) : wrap(result)
  }

  Object.defineProperties(normalized, {
    __arkdownPluginHooks: { value: hookNames },
    __arkdownPluginHookMeta: { value: hookMeta },
    __arkdownDispatchPluginHook: { value: dispatch },
    __arkdownPluginCacheIdentity: { value: cacheIdentity },
  })
  if (compilePlugin) {
    Object.defineProperties(normalized, {
      __arkdownHvigorCompilePlugin: { value: true },
      __arkdownHvigorOrder: { value: plugin.order },
      ...(options.moduleRoot ? { __arkdownModuleRoot: { value: path.resolve(options.moduleRoot) } } : {}),
      ...(options.targetName ? { __arkdownTargetName: { value: options.targetName } } : {}),
    })
  }
  return normalized
}

function normalizePluginList(value, binding = nativeBinding) {
  const plugins = []
  const append = (candidate) => {
    const plugin = unwrapDefault(candidate)
    if (plugin === undefined || plugin === null || plugin === false) return
    if (Array.isArray(plugin)) {
      for (const item of plugin) append(item)
      return
    }
    if (typeof plugin === 'function') {
      append(plugin())
      return
    }
    if (typeof plugin !== 'object') {
      throw arkdownError('ARKDOWN_PLUGIN_EXPORT_INVALID', `Expected a plugin object, got ${typeof plugin}`)
    }
    validateHookSurface(plugin, PLUGIN_HOOKS)
    if (
      !PLUGIN_HOOKS.some((name) => {
        const hook = plugin[name]
        return typeof hook === 'function' || (hook && typeof hook === 'object' && typeof hook.handler === 'function')
      })
    ) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_EXPORT_INVALID',
        `Plugin "${String(plugin.name || '<anonymous>')}" does not declare a supported hook`,
      )
    }
    plugins.push(normalizePlugin(plugin, binding))
  }
  append(value)
  if (plugins.length === 0) return plugins
  const response = binding.planRolldownPackagePlugins(plugins)
  emitPlanDiagnostics(response)
  return response.placements.map(({ pluginIndex }) => plugins[pluginIndex])
}

class PluginModuleLoader {
  constructor(binding = nativeBinding) {
    this.binding = binding
    this.installTypeScriptExtensions()
  }

  installTypeScriptExtensions() {
    for (const extension of TYPESCRIPT_EXTENSIONS) {
      Module._extensions[extension] = (module, filename) => {
        let output
        try {
          output = this.binding.compilePluginModuleSync(filename, true)
        } catch (cause) {
          if (cause && typeof cause === 'object') cause.code ||= 'ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR'
          throw cause
        }
        module._compile(output, filename)
      }
    }
  }

  resolve(candidate, baseDir) {
    if (typeof candidate !== 'string' || candidate.length === 0) {
      throw arkdownError('ARKDOWN_PLUGIN_PATH_INVALID', 'Plugin path must be a non-empty string')
    }
    try {
      const pluginPath = path.isAbsolute(candidate) ? candidate : path.resolve(baseDir, candidate)
      return require.resolve(pluginPath)
    } catch (cause) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_MODULE_NOT_FOUND',
        `Cannot resolve plugin module "${candidate}" from ${baseDir}`,
        {
          cause,
          pluginPath: candidate,
        },
      )
    }
  }

  async load(candidate, baseDir) {
    const file = this.resolve(candidate, baseDir)
    const extension = path.extname(file).toLowerCase()
    if (TYPESCRIPT_EXTENSIONS.has(extension)) {
      let output
      try {
        output = await this.binding.compilePluginModule(file, true)
      } catch (cause) {
        if (cause && typeof cause === 'object') cause.code ||= 'ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR'
        throw cause
      }
      const loadedModule = new Module(file, module.parent || module)
      loadedModule.filename = file
      loadedModule.paths = Module._nodeModulePaths(path.dirname(file)).concat(Module.globalPaths)
      loadedModule._compile(output, file)
      return { file, exports: loadedModule.exports }
    }
    if (!JAVASCRIPT_EXTENSIONS.has(extension)) {
      throw arkdownError(
        'ARKDOWN_PLUGIN_MODULE_EXTENSION_UNSUPPORTED',
        `Unsupported plugin module extension "${extension || '<none>'}": ${file}`,
        { pluginFile: file },
      )
    }
    if (extension === '.mjs') return { file, exports: await this.importFresh(file) }
    try {
      delete require.cache[file]
      return { file, exports: require(file) }
    } catch (cause) {
      if (cause?.code !== 'ERR_REQUIRE_ESM') throw cause
      return { file, exports: await this.importFresh(file) }
    }
  }

  importFresh(file) {
    const url = pathToFileURL(file)
    url.searchParams.set('arkdown-plugin-load', String(++esmLoadSequence))
    return import(url.href)
  }
}

class PluginAdapter {
  constructor(binding = nativeBinding) {
    this.binding = binding
    this.loader = new PluginModuleLoader(binding)
  }

  async prepare(config = {}, explicitPlugins) {
    if (!config || typeof config !== 'object' || Array.isArray(config)) {
      throw arkdownError('ARKDOWN_BUILD_CONFIG_INVALID', 'Build config must be an object')
    }
    for (const key of REMOVED_HVIGOR_CONFIG_KEYS) {
      if (Object.hasOwn(config, key)) {
        throw arkdownError(
          'ARKDOWN_HVIGOR_RUNTIME_UNSUPPORTED',
          `Build option "${key}" belonged to the removed Hvigor task runtime; use the native build config instead`,
          { option: key },
        )
      }
    }

    const projectRoot = path.resolve(config.project || process.cwd())
    const compilePlugins = []
    for (const key of ['hvigorPluginFile', 'compilePluginPath']) {
      for (const candidate of asArray(config[key])) {
        const loaded = await this.loader.load(candidate, projectRoot)
        if (!loaded) continue
        const value = requireDefaultExport(loaded.exports, loaded.file)
        const items = Array.isArray(value) ? value : [value]
        const validation = this.binding.validateHvigorCompilePlugins(items, loaded.file)
        emitPlanDiagnostics(validation)
        const pluginCacheIdentity =
          typeof this.binding.pluginModuleCacheIdentity === 'function'
            ? this.binding.pluginModuleCacheIdentity(loaded.file)
            : path.resolve(loaded.file)
        for (const plugin of items) {
          compilePlugins.push(
            normalizePlugin(plugin, this.binding, {
              compilePlugin: true,
              pluginFile: loaded.file,
              pluginCacheIdentity,
            }),
          )
        }
      }
    }
    if (compilePlugins.length > 0) {
      const response = this.binding.planHvigorCompilePlugins(
        {
          compileMode: 'esmodule',
          preview: Boolean(config.preview),
          target: config.target,
          mode: config.mode,
        },
        compilePlugins,
      )
      emitPlanDiagnostics(response)
      const retained = new Set(response.placements.map(({ pluginIndex }) => pluginIndex))
      for (let index = compilePlugins.length - 1; index >= 0; index -= 1) {
        if (!retained.has(index)) compilePlugins.splice(index, 1)
      }
    }

    const nativeConfig = { ...config }
    delete nativeConfig.hvigorPluginFile
    delete nativeConfig.compilePluginPath
    nativeConfig.nodeExecutable = process.execPath
    return {
      config: nativeConfig,
      plugins: [...compilePlugins, ...normalizePluginList(explicitPlugins, this.binding)],
    }
  }
}

function createBuild(nativeBuild = nativeBinding.build, binding = nativeBinding) {
  return async (config = {}, plugins) => {
    const prepared = await new PluginAdapter(binding).prepare(config, plugins)
    return nativeBuild(prepared.config, prepared.plugins, createSdkPluginDispatch())
  }
}

function createCheck(nativeCheck = nativeBinding.check) {
  return (config = {}) => nativeCheck(config, createSdkPluginDispatch())
}

module.exports = {
  COMPILE_PLUGIN_HOOKS,
  ESMODULE_DORMANT_HOOKS,
  PLUGIN_HOOKS,
  PluginAdapter,
  PluginModuleLoader,
  createBuild,
  createCheck,
  createSdkPluginDispatch,
  createSdkPluginHost,
  normalizePlugin,
  normalizePluginList,
  parsePluginAst,
  requireDefaultExport,
}
