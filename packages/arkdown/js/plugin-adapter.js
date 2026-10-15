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
const PLUGIN_HOOKS = Object.freeze([
  'beforeBuild',
  'buildStart',
  'afterBuildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'resolveDynamicImport',
  'watchChange',
  'beforeBuildEnd',
  'buildEnd',
  'afterBuildEnd',
  'bundleEnd',
  'cleanUp',
  'shouldInvalidCache',
  ...ESMODULE_DORMANT_HOOKS,
])

// hvigor-arkts-compose/types.d.ts and arkts-pack.js::doWrapper compile hooks.
// Explicit plugins use the wider Rolldown surface above.
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
    throw arkdownError(
      diagnostic.code,
      diagnostic.message,
      diagnostic.pluginName ? { pluginName: diagnostic.pluginName } : {},
    )
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

const isObject = (value) => value && typeof value === 'object'

function unwrapDefault(value) {
  return isObject(value) && 'default' in value ? value.default : value
}

function requireDefaultExport(value, file) {
  if (isObject(value) && 'default' in value && value.default) return value.default
  throw arkdownError('PLUGIN_FILE_NO_DEFAULT_EXPORT', `Plugin module must have a default export: ${file}`, {
    pluginFile: file,
  })
}

// Bound app ASTs/indexes for interleaved batches; keep declarations.
const SDK_PLUGIN_AST_LRU_CAPACITY = 64

class SdkPluginHost {
  classInstances = new Map()
  typeScripts = new Map()
  sessions = new Map()
  heapAfterGc = process.memoryUsage().heapUsed

  session(id) {
    if (!this.sessions.has(id))
      this.sessions.set(id, {
        snapshots: new Map(),
        asts: new WeakMap(),
        lru: new Map(),
        configs: new Map(),
        plugins: new Map(),
        stats: { parses: 0, reparses: 0, batches: 0, peakCommitted: 0 },
      })
    return this.sessions.get(id)
  }

  loadTypeScript(loader) {
    if (!loader) throw new Error('SDK checker project config has no etsLoaderPath')
    if (!this.typeScripts.has(loader)) {
      this.typeScripts.set(loader, require(path.join(loader, 'node_modules', 'typescript')))
    }
    return this.typeScripts.get(loader)
  }

  register(req) {
    if (req.path !== undefined)
      this.session(req.sessionId).plugins.set(req.pluginId, {
        path: req.path,
        functionName: req.functionName,
        className: req.className,
      })
  }

  snapshot(id, span) {
    const { snapshots } = this.session(id)
    if (!snapshots)
      throw new Error(`SDK checker source ${id}:${span.sourceId}: sources were released before strict diagnostics`)
    let source = snapshots.get(span.sourceId)
    if (span.source !== undefined && !source) {
      source = { fileName: span.fileName, source: span.source, parses: 0 }
      snapshots.set(span.sourceId, source)
    }
    if (!source || source.fileName !== span.fileName)
      throw new Error(`SDK checker source ${id}:${span.sourceId} is not registered`)
    return source
  }

  encodeText(text) {
    // Rust str via JSON has no lone surrogates: UTF-8 preserves UTF-16 spans.
    if (typeof text.source === 'string') text.source = Buffer.from(text.source)
  }

  projectConfig(id, ref) {
    const { configs } = this.session(id)
    let config = configs.get(ref.projectConfigId)
    if (!config && ref.projectConfig !== undefined) {
      config = { ...ref.projectConfig }
      for (const field of ['syscapIntersection', 'syscapUnion'])
        config[field + 'Set'] = new Set(config[field + 'Set'] || config[field] || [])
      configs.set(ref.projectConfigId, config)
    }
    if (!config) throw new Error(`SDK checker project config ${id}:${ref.projectConfigId} is not registered`)
    return config
  }

  locateNode(ts, id, span) {
    const text = this.snapshot(id, span)
    const session = this.session(id)
    let files = session.asts.get(ts)
    if (!files) session.asts.set(ts, (files = new Map()))
    let source = files.get(span.sourceId)
    if (!source) {
      text.source = text.source.toString()
      const kind = span.fileName.endsWith('.ets') ? (ts.ScriptKind.ETS ?? ts.ScriptKind.TS) : ts.ScriptKind.TS
      source = {
        sourceFile: ts.createSourceFile(span.fileName, text.source, ts.ScriptTarget.Latest, true, kind),
        nodes: new Map(),
        children: new WeakMap(),
      }
      session.stats.parses++
      if (text.parses++) session.stats.reparses++
      files.set(span.sourceId, source)
    }
    const lru = session.lru
    if (!/\.d\.(ts|ets)$/.test(span.fileName)) {
      lru.delete(source)
      lru.set(source, { files, id: span.sourceId })
      if (lru.size > SDK_PLUGIN_AST_LRU_CAPACITY) {
        const [oldest, entry] = lru.entries().next().value
        entry.files.delete(entry.id)
        lru.delete(oldest)
        this.encodeText(session.snapshots.get(entry.id))
      }
    }
    const sf = source.sourceFile
    const key = `${span.pos}:${span.end}:${span.start}`
    const matches = (node, start) => (span.text === undefined ? start === span.start : node.getText(sf) === span.text)
    const cached = source.nodes.get(key)
    if (cached && matches(cached, span.text === undefined ? cached.getStart(sf) : span.start)) return cached
    const childrenOf = (node) => {
      let children = source.children.get(node)
      if (!children) {
        children = []
        ts.forEachChild(node, (child) => {
          children.push(child)
        })
        children.sort((a, b) => a.pos - b.pos || a.end - b.end)
        source.children.set(node, children)
      }
      return children
    }
    const locate = (node) => {
      if (node.pos > span.pos || node.end < span.end) return undefined
      let exact
      // Prune by raw spans; scan only end-matching candidates.
      if (node.end === span.end) {
        const start = span.text === undefined || node.pos !== span.pos ? node.getStart(sf) : span.start
        if ((node.pos === span.pos || start === span.pos) && matches(node, start)) exact = node
      }
      const children = childrenOf(node)
      let low = 0
      let high = children.length
      while (low < high) {
        const middle = (low + high) >>> 1
        if (children[middle].pos <= span.pos) low = middle + 1
        else high = middle
      }
      for (let index = low - 1; index >= 0; index--) {
        if (children[index].end < span.end) continue
        const nested = locate(children[index])
        if (nested) return nested
      }
      return exact
    }
    const exact = locate(sf)
    if (exact) source.nodes.set(key, exact)
    return exact
  }

  prepareNodes(requests) {
    // Allocate the batch once; every slot is assigned below before callbacks run.
    // oxlint-disable-next-line unicorn/no-new-array
    const items = new Array(requests.length)
    const groups = new Map()
    // Register even short-circuited callbacks.
    for (let i = 0; i < requests.length; i++) {
      const req = requests[i],
        payload = req.args[0]
      this.register(req)
      const item = {}
      items[i] = item
      try {
        item.config = this.projectConfig(req.sessionId, payload)
        for (const field of ['node', 'declaration']) this.snapshot(req.sessionId, payload[field])
        const ts = this.loadTypeScript(item.config.etsLoaderPath)
        let bySource = groups.get(ts)
        if (!bySource) groups.set(ts, (bySource = new Map()))
        for (const field of ['node', 'declaration']) {
          const key = `${req.sessionId}:${payload[field].sourceId}`
          if (!bySource.has(key)) bySource.set(key, [])
          bySource.get(key).push(i, field)
        }
      } catch (error) {
        item.error = error
      }
    }
    // Group node lookup; keep callbacks in wire order.
    for (const [ts, bySource] of groups)
      for (const entries of bySource.values()) {
        for (let j = 0; j < entries.length; j += 2) {
          const i = entries[j],
            field = entries[j + 1],
            req = requests[i]
          const item = items[i]
          try {
            item[field] = this.locateNode(ts, req.sessionId, req.args[0][field])
          } catch (error) {
            item[field + 'Error'] = error
          }
        }
      }
    return items
  }

  collectHeap({ reduceMemory = false } = {}) {
    if (typeof global.gc === 'function') {
      // ets_checker.ts::serviceChecker collects after releasing compiler state.
      // Node 18/20 interpret unknown options as minor GC; use full GC there.
      if (reduceMemory && Number.parseInt(process.versions.node, 10) >= 22) {
        global.gc({ type: 'major', execution: 'sync', flavor: 'last-resort' })
      } else global.gc()
      this.heapAfterGc = process.memoryUsage().heapUsed
    }
  }

  dispatchBatch(requests) {
    const responses = this.runBatch(requests)
    if (process.memoryUsage().heapUsed > Math.max(128 << 20, 2 * this.heapAfterGc)) this.collectHeap()
    return responses
  }

  runBatch(requests) {
    if (requests.some((r) => r.operation !== 'syscap')) {
      return requests.map((r) => (r.operation === 'syscap' ? this.runBatch([r])[0] : this.dispatch(r)))
    }
    const prepared = this.prepareNodes(requests)
    const committed = v8.getHeapStatistics().total_physical_size
    for (const id of new Set(requests.map((r) => r.sessionId))) {
      const stats = this.session(id).stats
      stats.batches++
      stats.peakCommitted = Math.max(stats.peakCommitted, committed)
    }
    const responses = []
    for (let i = 0; i < requests.length; i++) {
      const previous = responses[requests[i].after]
      const skip =
        previous &&
        (previous.skipped || (previous.error ? previous.phase !== 'load' : previous.found && !previous.checkResult))
      responses.push(skip ? { skipped: true } : this.dispatch(requests[i], prepared[i]))
    }
    return responses
  }

  dispatch(req, prepared) {
    const { operation, sessionId, pluginId, args } = req
    if (['compactSession', 'releaseSources', 'releaseSession'].includes(operation)) {
      let session = this.sessions.get(sessionId)
      if (!session) return { found: true }
      const stats = session.stats
      const committedBefore = v8.getHeapStatistics().total_physical_size
      if (operation === 'releaseSession') this.sessions.delete(sessionId)
      else {
        // tsrs sends text once; retain snapshots for reparsing.
        session.asts = new WeakMap()
        session.lru.clear()
        if (operation === 'releaseSources') session.snapshots = null
        else for (const text of session.snapshots?.values() || []) this.encodeText(text)
      }
      session = null
      this.collectHeap({ reduceMemory: true })
      return {
        found: true,
        stats: { ...stats, committedBefore, retainedCommitted: v8.getHeapStatistics().total_physical_size },
      }
    }
    let callback
    const isClass = ['prepareClass', 'syscap'].includes(operation)
    try {
      this.register(req)
      const descriptor = this.session(sessionId).plugins.get(pluginId)
      if (!descriptor) throw new Error(`SDK checker plugin ${sessionId}:${pluginId} is not registered`)
      if (isClass) {
        const key = `${descriptor.path}\0${descriptor.className}`
        callback = this.classInstances.get(key)
        if (!callback) {
          const Checker = require(descriptor.path)[descriptor.className]
          if (typeof Checker !== 'function') return { found: false }
          this.classInstances.set(key, (callback = new Checker()))
        }
      } else callback = require(descriptor.path)[descriptor.functionName]
    } catch (error) {
      return { found: false, phase: 'load', error: String(error?.stack || error) }
    }
    if (isClass ? !callback : typeof callback !== 'function') return { found: false }
    if (operation === 'prepareClass') return { found: true }
    try {
      if (operation === 'syscap') {
        if (typeof callback.check !== 'function') return { found: true, checkResult: false, checkMessage: '' }
        const { node, declaration, config, error, nodeError, declarationError } = prepared
        if (error || nodeError || declarationError) throw error || nodeError || declarationError
        if (!node || !declaration) {
          const missing = args[0][!node ? 'node' : 'declaration']
          throw new Error(
            `Unable to recreate SDK checker source node ${missing.fileName}:${missing.pos}-${missing.end} (start ${missing.start})`,
          )
        }
        const value = callback.check(node, declaration, config)
        return {
          found: true,
          checkResult: Boolean(value?.checkResult),
          checkMessage: String(value?.checkMessage ?? ''),
        }
      }
      if (operation === 'regex') {
        const regex = callback()
        if (!(regex instanceof RegExp)) return { found: true, matched: false }
        regex.lastIndex = 0
        const match = regex.exec(args[0])
        return {
          found: true,
          matched: match !== null,
          groups: match === null ? [] : Array.from(match, (value) => (value === undefined ? '' : String(value))),
        }
      }
      const value = callback(...args)
      if (operation === 'distribution')
        return {
          found: true,
          valid: Boolean(value?.valid),
          version: String(value?.version ?? ''),
          message: String(value?.message ?? ''),
        }
      return { found: true, result: Boolean(value?.result), message: String(value?.message ?? '') }
    } catch (error) {
      return { found: true, phase: 'invoke', error: String(error?.stack || error) }
    }
  }
}

function createSdkPluginHost() {
  const host = new SdkPluginHost()
  return (request) => host.dispatchBatch([request])[0]
}

function createSdkPluginDispatch() {
  const host = new SdkPluginHost()
  return (request) => JSON.stringify(host.dispatchBatch(JSON.parse(request)))
}

const asArray = (value) => (value == null ? [] : Array.isArray(value) ? value : [value])

function validateHookSurface(plugin, hookNames, pluginFile, compilePlugin = false) {
  const invalid = (kind, hookName, message) =>
    arkdownError(`ARKDOWN_PLUGIN_HOOK_${kind}`, `Plugin "${String(plugin.name || '<anonymous>')}" ${message}`, {
      hookName,
      pluginFile,
    })
  if (!compilePlugin)
    for (const hookName of UNSUPPORTED_ROLLUP_HOOKS) {
      if (plugin[hookName] !== undefined && plugin[hookName] !== null) {
        throw invalid('UNSUPPORTED', hookName, `uses unsupported hook "${hookName}"`)
      }
    }
  for (const hookName of hookNames) {
    const hook = plugin[hookName]
    if (hook == null) continue
    if (
      typeof hook !== 'function' &&
      !(compilePlugin === false && typeof hook === 'object' && typeof hook.handler === 'function')
    ) {
      const expected = compilePlugin ? 'a function' : 'a function or an object with a handler function'
      throw invalid('INVALID', hookName, `hook "${hookName}" must be ${expected}`)
    }
    if (
      !compilePlugin &&
      typeof hook === 'object' &&
      hook.order !== undefined &&
      hook.order !== 'pre' &&
      hook.order !== 'post'
    ) {
      throw invalid('ORDER_INVALID', hookName, `hook "${hookName}" order must be "pre" or "post"`)
    }
  }
}

const hookHandler = (hook) => (typeof hook === 'function' ? hook : hook.handler)
const hasHook = (hook) => typeof hook === 'function' || (isObject(hook) && typeof hook.handler === 'function')

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
  if (isObject(value) && typeof value.message === 'string') return value.message
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

const HVIGOR_CACHE_FIELDS = ['cache', 'commonCache', 'cacheStores', 'pluginCaches', 'moduleInfos']

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
  for (const key of HVIGOR_CACHE_FIELDS) {
    if (restored?.[key] instanceof Map) state[key] = restored[key]
  }
}

function snapshotHvigorCacheState(share) {
  const state = share?.[HVIGOR_SHARE_STATE]
  if (!state) return undefined
  const snapshot = v8.serialize(Object.fromEntries(HVIGOR_CACHE_FIELDS.map((key) => [key, state[key]])))
  const buildInstanceId = share.__arkdownBuildInstanceId
  if (buildInstanceId != null) {
    hvigorShareByBuildInstance.delete(String(buildInstanceId))
  }
  return snapshot
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
    if (isObject(value) && !(value instanceof Map)) {
      projectConfig[key] = new Map(Object.entries(value))
    }
  }
  const sourceToModule = projectConfig.mockParams?.source2ModuleIdMap
  if (isObject(sourceToModule) && !(sourceToModule instanceof Map)) {
    projectConfig.mockParams.source2ModuleIdMap = new Map(Object.entries(sourceToModule))
  }
}

function hydrateHvigorShare(rawShare, binding) {
  const share = isObject(rawShare) ? rawShare : {}
  const projectConfig = share.projectConfig && typeof share.projectConfig === 'object' ? share.projectConfig : {}
  restoreProjectConfigCollections(projectConfig)
  if (typeof projectConfig.watchMode === 'boolean') projectConfig.watchMode = String(projectConfig.watchMode)
  const scope = hvigorShareScope(projectConfig)
  let state = hvigorShareStateByScope.get(scope)
  if (!state) {
    state = {
      ...Object.fromEntries(
        [...HVIGOR_CACHE_FIELDS, 'hashOverrides', 'pendingModuleInfos'].map((key) => [key, new Map()]),
      ),
      errorFileIds: new Set(),
      arkProjectConfig: {},
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
  if (buildInstanceId != null) {
    const existing = hvigorShareByBuildInstance.get(String(buildInstanceId))
    if (existing) return existing
  }
  const handle = share.__arkdownNativeShare
  const shareMethod = (name, fallback) => {
    if (typeof share[name] === 'function') return share[name].bind(share)
    const nativeName = `hvigorShare${name[0].toUpperCase()}${name.slice(1)}`
    return handle ? (...args) => binding[nativeName](handle, ...args) : fallback
  }
  const nativeGetHash = shareMethod('getHashByFilePath')
  const nativeGetFileInfo = shareMethod('getCachedFileInfo')
  const nativeGetSourceFiles = shareMethod('getSourceFiles', () => [])
  const nativeImportResolver = shareMethod('importResolver', () => undefined)
  const nativeRecordHookEvent = shareMethod('recordHookEvent', () => {})
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
  if (buildInstanceId != null) {
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
  const result = {
    ...info,
    meta: options?.meta || info.meta || {},
    moduleSideEffects:
      options && Object.hasOwn(options, 'moduleSideEffects')
        ? options.moduleSideEffects
        : (info.moduleSideEffects ?? null),
  }
  for (const key of ['description', 'packageJsonPath']) {
    if (options && Object.hasOwn(options, key)) result[key] = options[key]
  }
  return result
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
      if (isObject(value) && !(value instanceof Error)) Object.assign(error, value)
      error.code ||= 'PLUGIN_ERROR'
      error.plugin ||= pluginName
      throw error
    },
  }
}

function cacheRequest(request) {
  const value = isObject(request) ? request : { cacheHit: Boolean(request) }
  if (value.__arkdownOxcAst) {
    if (value.ast === undefined) value.ast = reviveOxcAst(value.__arkdownOxcAst)
    delete value.__arkdownOxcAst
  }
  return value
}

const HOOK_ARGUMENT_FIELDS = new Map(
  [
    ['beforeBuild afterBuildStart afterBuildEnd bundleEnd cleanUp closeBundle closeWatcher', []],
    ['buildStart', ['buildStartOptions']],
    ['resolveId resolveDynamicImport', ['id', 'importer', 'resolveOptions']],
    ['load', ['id']],
    ['transform', ['code', 'id', 'transformOptions']],
    ['moduleParsed', ['moduleInfo']],
    ['renderStart', ['outputOptions']],
    ['banner footer intro outro augmentChunkHash', ['chunk']],
    ['renderDynamicImport resolveFileUrl resolveImportMeta', ['outputHookOptions']],
    ['renderChunk', ['code', 'chunk', 'outputOptions']],
    ['renderError', ['error']],
    ['writeBundle', ['outputOptions', 'bundle']],
  ].flatMap(([names, fields]) => names.split(' ').map((name) => [name, fields])),
)

function hookArguments(hookName, request, compilePlugin) {
  if (compilePlugin && hookName === 'buildStart') return []
  if (compilePlugin && hookName === 'transform') return [request.code, request.id]
  const fields = HOOK_ARGUMENT_FIELDS.get(hookName)
  if (fields) return fields.map((field) => request[field])
  switch (hookName) {
    case 'watchChange':
      return [request.id, { event: request.watchEvent }]
    case 'beforeBuildEnd':
      return [undefined]
    case 'shouldInvalidCache':
      return [cacheRequest(request.cacheRequest)]
    case 'generateBundle':
      return [request.outputOptions, request.bundle, request.isWrite === true]
    case 'buildEnd': {
      const errors = request.buildEndContext?.errors || []
      if (errors.length === 0) return [undefined]
      const error = new Error(errors.map((item) => item.message || String(item)).join('\n'))
      error.name = 'RolldownError'
      error.errors = errors
      return [error]
    }
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
  const hookNames = allowedHooks.filter((name) => hasHook(plugin[name]))
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
      if (compilePlugin && pluginFile && isObject(error)) {
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
        if (
          hookName === 'cleanUp' ||
          (compilePlugin && hookName === 'buildEnd' && (request.buildEndContext?.errors?.length ?? 0) > 0)
        ) {
          const buildInstanceId = share.__arkdownBuildInstanceId
          if (buildInstanceId != null) hvigorShareByBuildInstance.delete(String(buildInstanceId))
        }
        return {}
      }
      return result && typeof result.then === 'function' ? Promise.resolve(result).then(finish) : finish()
    }
    const wrap = (value) => {
      if (value == null) return {}
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
    if (!PLUGIN_HOOKS.some((name) => hasHook(plugin[name]))) {
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
          if (isObject(cause)) cause.code ||= 'ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR'
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
        if (isObject(cause)) cause.code ||= 'ARKDOWN_PLUGIN_MODULE_COMPILE_ERROR'
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
